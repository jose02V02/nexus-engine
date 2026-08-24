use std::time::Duration;

use nexus_engine::{
    BufferDescriptor, BufferUsage, CommandEncoder, FrameScheduler, GpuAdapter, GpuDevice,
    GpuError, GraphicsStandard, ShaderLanguage, ShaderModule, ShaderStage,
};

fn compute_shader() -> ShaderModule {
    ShaderModule {
        stage: ShaderStage::Compute,
        language: ShaderLanguage::Wgsl,
        entry_point: "main".to_owned(),
        source: "@compute @workgroup_size(1) fn main() {}".to_owned(),
    }
}

#[test]
fn webgpu_validates_and_submits_compute_dispatch() {
    let mut device = GpuDevice::new(GraphicsStandard::WebGpu, GpuAdapter::software());
    let pipeline = device.create_pipeline(vec![compute_shader()]).unwrap();
    let mut encoder = CommandEncoder::default();
    encoder.set_pipeline(pipeline);
    encoder.dispatch(8, 4, 1).unwrap();
    let report = device.submit(&encoder.finish().unwrap()).unwrap();
    assert_eq!(report.compute_dispatches, 1);
}

#[test]
fn webgl2_rejects_compute_shaders() {
    let mut device = GpuDevice::new(GraphicsStandard::WebGl2, GpuAdapter::software());
    assert!(matches!(device.create_pipeline(vec![compute_shader()]), Err(GpuError::InvalidShader(_) | GpuError::Unsupported(_))));
}

#[test]
fn buffer_writes_require_usage_and_bounds() {
    let mut device = GpuDevice::new(GraphicsStandard::WebGpu, GpuAdapter::software());
    let writable = device.create_buffer(BufferDescriptor { size: 8, usage: BufferUsage::STORAGE.union(BufferUsage::COPY_DST) }).unwrap();
    device.write_buffer(writable, 2, &[1, 2, 3]).unwrap();
    assert!(device.write_buffer(writable, 7, &[1, 2]).is_err());
    let vertex = device.create_buffer(BufferDescriptor { size: 8, usage: BufferUsage::VERTEX }).unwrap();
    assert!(device.write_buffer(vertex, 0, &[1]).is_err());
}

#[test]
fn render_encoder_enforces_pass_lifecycle() {
    let mut encoder = CommandEncoder::default();
    assert!(encoder.draw(3, 1).is_err());
    encoder.begin_render_pass().unwrap();
    encoder.draw(3, 1).unwrap();
    assert!(encoder.finish().is_err(), "open render passes cannot be submitted");
}

#[test]
fn frame_scheduler_supports_120_hz_without_over_presenting() {
    let mut scheduler = FrameScheduler::new(120);
    assert!(scheduler.present(Duration::ZERO));
    assert!(!scheduler.present(Duration::from_millis(4)));
    assert!(scheduler.present(Duration::from_millis(9)));
    assert_eq!(scheduler.refresh_hz(), 120);
    assert_eq!(scheduler.presented_frames(), 2);
}

#[test]
fn resource_limits_reject_oversized_buffers() {
    let adapter = GpuAdapter::software();
    let too_large = adapter.limits.max_buffer_size + 1;
    let mut device = GpuDevice::new(GraphicsStandard::WebGpu, adapter);
    assert!(device.create_buffer(BufferDescriptor { size: too_large, usage: BufferUsage::STORAGE }).is_err());
}
