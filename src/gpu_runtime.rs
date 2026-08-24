//! Validated WebGL 2 / WebGPU command runtime.
//!
//! Nexus owns API validation, resources, command encoding and frame pacing.
//! `GpuBackend::Software` is executable in this release; Vulkan, Metal and
//! WGPU are explicit adapter targets and are not reported as active hardware
//! until a platform adapter supplies one.

use std::collections::HashMap;
use std::time::Duration;

pub type GpuResourceId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsStandard { WebGl2, WebGpu }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackend { Vulkan, Metal, Wgpu, OpenGlEs, Software }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderStage { Vertex, Fragment, Compute }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderLanguage { GlslEs300, Wgsl }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuLimits {
    pub max_buffer_size: usize,
    pub max_texture_dimension_2d: u32,
    pub max_compute_workgroups_per_dimension: u32,
    pub max_bind_groups: u32,
}

impl Default for GpuLimits {
    fn default() -> Self {
        Self {
            max_buffer_size: 256 * 1024 * 1024,
            max_texture_dimension_2d: 8192,
            max_compute_workgroups_per_dimension: 65_535,
            max_bind_groups: 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuAdapter {
    pub name: String,
    pub backend: GpuBackend,
    pub hardware_accelerated: bool,
    pub supports_compute: bool,
    pub limits: GpuLimits,
}

impl GpuAdapter {
    #[must_use]
    pub fn software() -> Self {
        Self {
            name: "Nexus validated software adapter".to_owned(),
            backend: GpuBackend::Software,
            hardware_accelerated: false,
            supports_compute: true,
            limits: GpuLimits::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaderModule {
    pub stage: ShaderStage,
    pub language: ShaderLanguage,
    pub entry_point: String,
    pub source: String,
}

impl ShaderModule {
    pub fn validate(&self, standard: GraphicsStandard, adapter: &GpuAdapter) -> Result<(), GpuError> {
        if self.source.trim().is_empty() || self.source.len() > 1024 * 1024 {
            return Err(GpuError::InvalidShader("shader source is empty or too large".to_owned()));
        }
        if self.entry_point.trim().is_empty() || !self.source.contains(&self.entry_point) {
            return Err(GpuError::InvalidShader("entry point is not present in shader source".to_owned()));
        }
        match (standard, self.language) {
            (GraphicsStandard::WebGl2, ShaderLanguage::GlslEs300)
            | (GraphicsStandard::WebGpu, ShaderLanguage::Wgsl) => {}
            _ => return Err(GpuError::InvalidShader("shader language does not match graphics API".to_owned())),
        }
        if self.stage == ShaderStage::Compute {
            if standard != GraphicsStandard::WebGpu {
                return Err(GpuError::Unsupported("compute shaders require WebGPU".to_owned()));
            }
            if !adapter.supports_compute {
                return Err(GpuError::Unsupported("adapter does not support compute shaders".to_owned()));
            }
        }
        if self.source.contains("unsafe_external_texture") {
            return Err(GpuError::InvalidShader("unsupported external texture declaration".to_owned()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferUsage(u32);

impl BufferUsage {
    pub const VERTEX: Self = Self(1 << 0);
    pub const INDEX: Self = Self(1 << 1);
    pub const UNIFORM: Self = Self(1 << 2);
    pub const STORAGE: Self = Self(1 << 3);
    pub const COPY_SRC: Self = Self(1 << 4);
    pub const COPY_DST: Self = Self(1 << 5);

    #[must_use]
    pub const fn union(self, other: Self) -> Self { Self(self.0 | other.0) }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool { self.0 & other.0 == other.0 }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferDescriptor { pub size: usize, pub usage: BufferUsage }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat { Rgba8Unorm, Bgra8Unorm, Depth24Stencil8, Rgba16Float }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureDescriptor {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpuError {
    InvalidDescriptor(String),
    InvalidShader(String),
    InvalidCommand(String),
    MissingResource(GpuResourceId),
    OutOfMemory,
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodedCommand {
    BeginRenderPass,
    EndRenderPass,
    SetPipeline(GpuResourceId),
    SetVertexBuffer(GpuResourceId),
    Draw { vertices: u32, instances: u32 },
    Dispatch { x: u32, y: u32, z: u32 },
}

#[derive(Debug, Default)]
pub struct CommandEncoder {
    commands: Vec<EncodedCommand>,
    render_pass_open: bool,
}

impl CommandEncoder {
    pub fn begin_render_pass(&mut self) -> Result<(), GpuError> {
        if self.render_pass_open { return Err(GpuError::InvalidCommand("render pass already open".to_owned())); }
        self.render_pass_open = true;
        self.commands.push(EncodedCommand::BeginRenderPass);
        Ok(())
    }

    pub fn end_render_pass(&mut self) -> Result<(), GpuError> {
        if !self.render_pass_open { return Err(GpuError::InvalidCommand("no render pass is open".to_owned())); }
        self.render_pass_open = false;
        self.commands.push(EncodedCommand::EndRenderPass);
        Ok(())
    }

    pub fn set_pipeline(&mut self, pipeline: GpuResourceId) {
        self.commands.push(EncodedCommand::SetPipeline(pipeline));
    }

    pub fn set_vertex_buffer(&mut self, buffer: GpuResourceId) {
        self.commands.push(EncodedCommand::SetVertexBuffer(buffer));
    }

    pub fn draw(&mut self, vertices: u32, instances: u32) -> Result<(), GpuError> {
        if !self.render_pass_open || vertices == 0 || instances == 0 {
            return Err(GpuError::InvalidCommand("draw requires an open pass and non-zero counts".to_owned()));
        }
        self.commands.push(EncodedCommand::Draw { vertices, instances });
        Ok(())
    }

    pub fn dispatch(&mut self, x: u32, y: u32, z: u32) -> Result<(), GpuError> {
        if self.render_pass_open || x == 0 || y == 0 || z == 0 {
            return Err(GpuError::InvalidCommand("compute dispatch requires positive dimensions outside a render pass".to_owned()));
        }
        self.commands.push(EncodedCommand::Dispatch { x, y, z });
        Ok(())
    }

    pub fn finish(self) -> Result<Vec<EncodedCommand>, GpuError> {
        if self.render_pass_open { return Err(GpuError::InvalidCommand("render pass was not ended".to_owned())); }
        Ok(self.commands)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BufferResource { descriptor: BufferDescriptor, bytes: Vec<u8> }

#[derive(Debug, Clone, PartialEq, Eq)]
struct PipelineResource { shaders: Vec<ShaderModule> }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionReport {
    pub command_count: usize,
    pub draw_calls: usize,
    pub compute_dispatches: usize,
}

pub struct GpuDevice {
    standard: GraphicsStandard,
    adapter: GpuAdapter,
    next_resource: GpuResourceId,
    buffers: HashMap<GpuResourceId, BufferResource>,
    textures: HashMap<GpuResourceId, TextureDescriptor>,
    pipelines: HashMap<GpuResourceId, PipelineResource>,
    allocated_bytes: usize,
}

impl GpuDevice {
    #[must_use]
    pub fn new(standard: GraphicsStandard, adapter: GpuAdapter) -> Self {
        Self { standard, adapter, next_resource: 1, buffers: HashMap::new(), textures: HashMap::new(), pipelines: HashMap::new(), allocated_bytes: 0 }
    }

    pub fn create_buffer(&mut self, descriptor: BufferDescriptor) -> Result<GpuResourceId, GpuError> {
        if descriptor.size == 0 || descriptor.size > self.adapter.limits.max_buffer_size {
            return Err(GpuError::InvalidDescriptor("buffer size exceeds device limits".to_owned()));
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(descriptor.size).map_err(|_| GpuError::OutOfMemory)?;
        bytes.resize(descriptor.size, 0);
        let id = self.allocate_id();
        self.allocated_bytes = self.allocated_bytes.checked_add(descriptor.size).ok_or(GpuError::OutOfMemory)?;
        self.buffers.insert(id, BufferResource { descriptor, bytes });
        Ok(id)
    }

    pub fn write_buffer(&mut self, id: GpuResourceId, offset: usize, data: &[u8]) -> Result<(), GpuError> {
        let buffer = self.buffers.get_mut(&id).ok_or(GpuError::MissingResource(id))?;
        if !buffer.descriptor.usage.contains(BufferUsage::COPY_DST) {
            return Err(GpuError::InvalidCommand("buffer lacks COPY_DST usage".to_owned()));
        }
        let end = offset.checked_add(data.len()).ok_or_else(|| GpuError::InvalidCommand("buffer write overflow".to_owned()))?;
        let destination = buffer.bytes.get_mut(offset..end).ok_or_else(|| GpuError::InvalidCommand("buffer write is out of bounds".to_owned()))?;
        destination.copy_from_slice(data);
        Ok(())
    }

    pub fn create_texture(&mut self, descriptor: TextureDescriptor) -> Result<GpuResourceId, GpuError> {
        let max = self.adapter.limits.max_texture_dimension_2d;
        if descriptor.width == 0 || descriptor.height == 0 || descriptor.width > max || descriptor.height > max {
            return Err(GpuError::InvalidDescriptor("texture dimensions exceed device limits".to_owned()));
        }
        let bytes = (descriptor.width as usize).checked_mul(descriptor.height as usize)
            .and_then(|pixels| pixels.checked_mul(4)).ok_or(GpuError::OutOfMemory)?;
        self.allocated_bytes = self.allocated_bytes.checked_add(bytes).ok_or(GpuError::OutOfMemory)?;
        let id = self.allocate_id();
        self.textures.insert(id, descriptor);
        Ok(id)
    }

    pub fn create_pipeline(&mut self, shaders: Vec<ShaderModule>) -> Result<GpuResourceId, GpuError> {
        if shaders.is_empty() { return Err(GpuError::InvalidDescriptor("pipeline has no shaders".to_owned())); }
        for shader in &shaders { shader.validate(self.standard, &self.adapter)?; }
        let compute = shaders.iter().filter(|shader| shader.stage == ShaderStage::Compute).count();
        if compute > 0 && (compute != 1 || shaders.len() != 1) {
            return Err(GpuError::InvalidDescriptor("compute pipeline must contain exactly one compute shader".to_owned()));
        }
        let id = self.allocate_id();
        self.pipelines.insert(id, PipelineResource { shaders });
        Ok(id)
    }

    pub fn submit(&self, commands: &[EncodedCommand]) -> Result<SubmissionReport, GpuError> {
        let mut pipeline = None;
        let mut render_pass_open = false;
        let mut draw_calls = 0;
        let mut compute_dispatches = 0;
        for command in commands {
            match *command {
                EncodedCommand::BeginRenderPass => {
                    if render_pass_open { return Err(GpuError::InvalidCommand("nested render pass".to_owned())); }
                    render_pass_open = true;
                }
                EncodedCommand::EndRenderPass => {
                    if !render_pass_open { return Err(GpuError::InvalidCommand("render pass end without begin".to_owned())); }
                    render_pass_open = false;
                }
                EncodedCommand::SetPipeline(id) => {
                    if !self.pipelines.contains_key(&id) { return Err(GpuError::MissingResource(id)); }
                    pipeline = Some(id);
                }
                EncodedCommand::SetVertexBuffer(id) => {
                    let buffer = self.buffers.get(&id).ok_or(GpuError::MissingResource(id))?;
                    if !buffer.descriptor.usage.contains(BufferUsage::VERTEX) {
                        return Err(GpuError::InvalidCommand("buffer lacks VERTEX usage".to_owned()));
                    }
                }
                EncodedCommand::Draw { .. } => {
                    if !render_pass_open { return Err(GpuError::InvalidCommand("draw is outside render pass".to_owned())); }
                    let id = pipeline.ok_or_else(|| GpuError::InvalidCommand("draw has no pipeline".to_owned()))?;
                    if self.pipelines[&id].shaders.iter().any(|shader| shader.stage == ShaderStage::Compute) {
                        return Err(GpuError::InvalidCommand("draw cannot use a compute pipeline".to_owned()));
                    }
                    draw_calls += 1;
                }
                EncodedCommand::Dispatch { x, y, z } => {
                    if render_pass_open { return Err(GpuError::InvalidCommand("dispatch is inside render pass".to_owned())); }
                    let limit = self.adapter.limits.max_compute_workgroups_per_dimension;
                    if x > limit || y > limit || z > limit { return Err(GpuError::InvalidCommand("dispatch exceeds workgroup limits".to_owned())); }
                    let id = pipeline.ok_or_else(|| GpuError::InvalidCommand("dispatch has no pipeline".to_owned()))?;
                    if !self.pipelines[&id].shaders.iter().all(|shader| shader.stage == ShaderStage::Compute) {
                        return Err(GpuError::InvalidCommand("dispatch requires a compute pipeline".to_owned()));
                    }
                    compute_dispatches += 1;
                }
            }
        }
        if render_pass_open { return Err(GpuError::InvalidCommand("submission ends inside render pass".to_owned())); }
        Ok(SubmissionReport { command_count: commands.len(), draw_calls, compute_dispatches })
    }

    #[must_use]
    pub fn allocated_bytes(&self) -> usize { self.allocated_bytes }

    #[must_use]
    pub fn adapter(&self) -> &GpuAdapter { &self.adapter }

    fn allocate_id(&mut self) -> GpuResourceId {
        let id = self.next_resource;
        self.next_resource = self.next_resource.saturating_add(1).max(1);
        id
    }
}

#[derive(Debug, Clone)]
pub struct FrameScheduler {
    refresh_hz: u16,
    interval: Duration,
    next_frame: Duration,
    presented_frames: u64,
    missed_frames: u64,
}

impl FrameScheduler {
    #[must_use]
    pub fn new(refresh_hz: u16) -> Self {
        let refresh_hz = refresh_hz.clamp(30, 120);
        Self { refresh_hz, interval: Duration::from_secs_f64(1.0 / f64::from(refresh_hz)), next_frame: Duration::ZERO, presented_frames: 0, missed_frames: 0 }
    }

    pub fn present(&mut self, elapsed: Duration) -> bool {
        if elapsed < self.next_frame { return false; }
        if self.next_frame != Duration::ZERO && elapsed >= self.next_frame + self.interval {
            let late = elapsed.saturating_sub(self.next_frame).as_nanos() / self.interval.as_nanos().max(1);
            self.missed_frames = self.missed_frames.saturating_add(late as u64);
        }
        self.presented_frames = self.presented_frames.saturating_add(1);
        self.next_frame = elapsed + self.interval;
        true
    }

    #[must_use]
    pub fn refresh_hz(&self) -> u16 { self.refresh_hz }
    #[must_use]
    pub fn presented_frames(&self) -> u64 { self.presented_frames }
    #[must_use]
    pub fn missed_frames(&self) -> u64 { self.missed_frames }
}
