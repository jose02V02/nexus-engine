//! Process ownership, bounded IPC and renderer sandbox enforcement.
//!
//! The portable backend uses one native thread per renderer so the protocol,
//! routing, crash handling and policy can be tested on every target. Android
//! isolated-service transport can implement the same message boundary without
//! changing BrowserCore.

use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread::{self, JoinHandle};

use crate::web_platform::{ProcessRole, SandboxPolicy};

pub type ProcessId = u64;
pub type FrameId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxCapability { Network, Filesystem, Camera, Microphone, Gpu }

#[derive(Debug, Clone, PartialEq)]
pub enum RendererCommand {
    CommitDocument { frame_id: FrameId, url: String, html: Vec<u8> },
    Paint { frame_id: FrameId },
    Input { frame_id: FrameId, x: f32, y: f32 },
    RequestCapability(SandboxCapability),
    ReleaseFrame { frame_id: FrameId },
    Shutdown,
    SimulateCrash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererEvent {
    Ready { process_id: ProcessId },
    DocumentCommitted { process_id: ProcessId, frame_id: FrameId, url: String },
    FramePainted { process_id: ProcessId, frame_id: FrameId },
    InputAccepted { process_id: ProcessId, frame_id: FrameId },
    CapabilityDenied { process_id: ProcessId, capability: SandboxCapability },
    MemoryLimitExceeded { process_id: ProcessId, requested_bytes: usize, limit_bytes: usize },
    FrameReleased { process_id: ProcessId, frame_id: FrameId },
    Crashed { process_id: ProcessId },
    Exited { process_id: ProcessId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererProcessSnapshot {
    pub id: ProcessId,
    pub site_key: String,
    pub sandbox: SandboxPolicy,
    pub attached_frames: Vec<FrameId>,
}

struct RendererProcess {
    site_key: String,
    sandbox: SandboxPolicy,
    commands: SyncSender<RendererCommand>,
    events: Receiver<RendererEvent>,
    thread: Option<JoinHandle<()>>,
}

/// Owns renderer execution units and is the only component allowed to route
/// browser commands across the renderer boundary.
pub struct ProcessBroker {
    next_process_id: ProcessId,
    processes: HashMap<ProcessId, RendererProcess>,
    frame_routes: HashMap<FrameId, ProcessId>,
    ipc_capacity: usize,
}

impl Default for ProcessBroker {
    fn default() -> Self { Self::new(64) }
}

impl ProcessBroker {
    #[must_use]
    pub fn new(ipc_capacity: usize) -> Self {
        Self {
            next_process_id: 1,
            processes: HashMap::new(),
            frame_routes: HashMap::new(),
            ipc_capacity: ipc_capacity.max(1),
        }
    }

    pub fn spawn_renderer(&mut self, site_key: impl Into<String>, sandbox: SandboxPolicy) -> Result<ProcessId, String> {
        if sandbox.role != ProcessRole::Renderer {
            return Err("renderer broker requires a renderer sandbox policy".to_owned());
        }
        let id = self.next_process_id;
        self.next_process_id = self.next_process_id.saturating_add(1).max(1);
        let (command_tx, command_rx) = mpsc::sync_channel(self.ipc_capacity);
        let (event_tx, event_rx) = mpsc::channel();
        let worker_policy = sandbox.clone();
        let thread = thread::Builder::new().name(format!("nexus-renderer-{id}"))
            .spawn(move || renderer_main(id, worker_policy, command_rx, event_tx))
            .map_err(|error| error.to_string())?;
        self.processes.insert(id, RendererProcess {
            site_key: site_key.into(), sandbox, commands: command_tx, events: event_rx, thread: Some(thread),
        });
        Ok(id)
    }

    pub fn attach_frame(&mut self, process_id: ProcessId, frame_id: FrameId) -> Result<(), String> {
        if !self.processes.contains_key(&process_id) { return Err("unknown renderer process".to_owned()); }
        if self.frame_routes.contains_key(&frame_id) { return Err("frame is already attached".to_owned()); }
        self.frame_routes.insert(frame_id, process_id);
        Ok(())
    }

    pub fn send_to_frame(&self, frame_id: FrameId, command: RendererCommand) -> Result<(), String> {
        if command_frame(&command).is_some_and(|target| target != frame_id) {
            return Err("command frame does not match route".to_owned());
        }
        let process_id = self.frame_routes.get(&frame_id).ok_or_else(|| "unrouted frame".to_owned())?;
        self.send(*process_id, command)
    }

    pub fn send(&self, process_id: ProcessId, command: RendererCommand) -> Result<(), String> {
        if let Some(frame_id) = command_frame(&command) {
            if self.frame_routes.get(&frame_id) != Some(&process_id) {
                return Err("frame is not owned by renderer process".to_owned());
            }
        }
        let process = self.processes.get(&process_id).ok_or_else(|| "unknown renderer process".to_owned())?;
        process.commands.try_send(command).map_err(|error| match error {
            TrySendError::Full(_) => "renderer IPC backpressure: queue full".to_owned(),
            TrySendError::Disconnected(_) => "renderer IPC channel closed".to_owned(),
        })
    }

    pub fn release_frame(&mut self, frame_id: FrameId) -> Result<(), String> {
        let process_id = *self.frame_routes.get(&frame_id).ok_or_else(|| "unrouted frame".to_owned())?;
        self.send(process_id, RendererCommand::ReleaseFrame { frame_id })?;
        self.frame_routes.remove(&frame_id);
        Ok(())
    }

    pub fn try_event(&self, process_id: ProcessId) -> Option<RendererEvent> {
        let process = self.processes.get(&process_id)?;
        match process.events.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }

    #[must_use]
    pub fn snapshot(&self, process_id: ProcessId) -> Option<RendererProcessSnapshot> {
        let process = self.processes.get(&process_id)?;
        let mut attached_frames = self.frame_routes.iter()
            .filter_map(|(frame, owner)| (*owner == process_id).then_some(*frame)).collect::<Vec<_>>();
        attached_frames.sort_unstable();
        Some(RendererProcessSnapshot {
            id: process_id, site_key: process.site_key.clone(), sandbox: process.sandbox.clone(), attached_frames,
        })
    }

    pub fn terminate(&mut self, process_id: ProcessId) -> bool {
        let Some(mut process) = self.processes.remove(&process_id) else { return false };
        let _ = process.commands.send(RendererCommand::Shutdown);
        if let Some(thread) = process.thread.take() { let _ = thread.join(); }
        self.frame_routes.retain(|_, owner| *owner != process_id);
        true
    }
}

impl Drop for ProcessBroker {
    fn drop(&mut self) {
        let ids = self.processes.keys().copied().collect::<Vec<_>>();
        for id in ids { self.terminate(id); }
    }
}

fn renderer_main(
    process_id: ProcessId,
    sandbox: SandboxPolicy,
    commands: Receiver<RendererCommand>,
    events: mpsc::Sender<RendererEvent>,
) {
    let _ = events.send(RendererEvent::Ready { process_id });
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let mut frame_bytes = HashMap::<FrameId, usize>::new();
        while let Ok(command) = commands.recv() {
            match command {
                RendererCommand::CommitDocument { frame_id, url, html } => {
                    let retained = frame_bytes.values().copied().sum::<usize>()
                        .saturating_sub(frame_bytes.get(&frame_id).copied().unwrap_or(0));
                    let requested = retained.saturating_add(html.len());
                    if requested > sandbox.memory_limit_bytes {
                        let _ = events.send(RendererEvent::MemoryLimitExceeded {
                            process_id, requested_bytes: requested, limit_bytes: sandbox.memory_limit_bytes,
                        });
                    } else {
                        frame_bytes.insert(frame_id, html.len());
                        let _ = events.send(RendererEvent::DocumentCommitted { process_id, frame_id, url });
                    }
                }
                RendererCommand::Paint { frame_id } => {
                    if frame_bytes.contains_key(&frame_id) {
                        let _ = events.send(RendererEvent::FramePainted { process_id, frame_id });
                    }
                }
                RendererCommand::Input { frame_id, .. } => {
                    if frame_bytes.contains_key(&frame_id) {
                        let _ = events.send(RendererEvent::InputAccepted { process_id, frame_id });
                    }
                }
                RendererCommand::RequestCapability(capability) => {
                    if !capability_allowed(&sandbox, capability) {
                        let _ = events.send(RendererEvent::CapabilityDenied { process_id, capability });
                    }
                }
                RendererCommand::ReleaseFrame { frame_id } => {
                    frame_bytes.remove(&frame_id);
                    let _ = events.send(RendererEvent::FrameReleased { process_id, frame_id });
                }
                RendererCommand::Shutdown => break,
                RendererCommand::SimulateCrash => panic!("simulated renderer crash"),
            }
        }
    }));
    if result.is_err() { let _ = events.send(RendererEvent::Crashed { process_id }); }
    let _ = events.send(RendererEvent::Exited { process_id });
}

fn capability_allowed(policy: &SandboxPolicy, capability: SandboxCapability) -> bool {
    match capability {
        SandboxCapability::Network => policy.network_allowed,
        SandboxCapability::Filesystem => policy.filesystem_allowed,
        SandboxCapability::Camera | SandboxCapability::Microphone | SandboxCapability::Gpu => false,
    }
}

fn command_frame(command: &RendererCommand) -> Option<FrameId> {
    match command {
        RendererCommand::CommitDocument { frame_id, .. }
        | RendererCommand::Paint { frame_id }
        | RendererCommand::Input { frame_id, .. }
        | RendererCommand::ReleaseFrame { frame_id } => Some(*frame_id),
        _ => None,
    }
}
