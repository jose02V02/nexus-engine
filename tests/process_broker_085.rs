use std::thread;
use std::time::{Duration, Instant};

use nexus_engine::{
    ProcessBroker, RendererCommand, RendererEvent, SandboxCapability, SandboxPolicy,
};

fn next_event(broker: &ProcessBroker, process: u64) -> RendererEvent {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if let Some(event) = broker.try_event(process) { return event; }
        assert!(Instant::now() < deadline, "renderer event timed out");
        thread::yield_now();
    }
}

#[test]
fn renderer_process_commits_and_paints_routed_frames() {
    let mut broker = ProcessBroker::default();
    let process = broker.spawn_renderer("https://example.test", SandboxPolicy::renderer(4096)).unwrap();
    broker.attach_frame(process, 7).unwrap();
    assert!(matches!(next_event(&broker, process), RendererEvent::Ready { .. }));
    broker.send_to_frame(7, RendererCommand::CommitDocument {
        frame_id: 7, url: "https://example.test/".into(), html: b"<p>Nexus</p>".to_vec(),
    }).unwrap();
    assert!(matches!(next_event(&broker, process), RendererEvent::DocumentCommitted { frame_id: 7, .. }));
    broker.send_to_frame(7, RendererCommand::Paint { frame_id: 7 }).unwrap();
    assert!(matches!(next_event(&broker, process), RendererEvent::FramePainted { frame_id: 7, .. }));
}

#[test]
fn renderer_sandbox_denies_direct_network_and_filesystem_access() {
    let mut broker = ProcessBroker::default();
    let process = broker.spawn_renderer("https://safe.test", SandboxPolicy::renderer(4096)).unwrap();
    let _ = next_event(&broker, process);
    broker.send(process, RendererCommand::RequestCapability(SandboxCapability::Network)).unwrap();
    broker.send(process, RendererCommand::RequestCapability(SandboxCapability::Filesystem)).unwrap();
    assert!(matches!(next_event(&broker, process), RendererEvent::CapabilityDenied { capability: SandboxCapability::Network, .. }));
    assert!(matches!(next_event(&broker, process), RendererEvent::CapabilityDenied { capability: SandboxCapability::Filesystem, .. }));
}

#[test]
fn memory_budget_rejects_oversized_document_commit() {
    let mut broker = ProcessBroker::default();
    let process = broker.spawn_renderer("https://large.test", SandboxPolicy::renderer(8)).unwrap();
    broker.attach_frame(process, 1).unwrap();
    let _ = next_event(&broker, process);
    broker.send_to_frame(1, RendererCommand::CommitDocument {
        frame_id: 1, url: "https://large.test/".into(), html: vec![0; 9],
    }).unwrap();
    assert!(matches!(next_event(&broker, process), RendererEvent::MemoryLimitExceeded { limit_bytes: 8, .. }));
}

#[test]
fn simulated_renderer_crash_is_reported_without_crashing_browser() {
    let mut broker = ProcessBroker::default();
    let process = broker.spawn_renderer("https://crash.test", SandboxPolicy::renderer(4096)).unwrap();
    let _ = next_event(&broker, process);
    broker.send(process, RendererCommand::SimulateCrash).unwrap();
    assert_eq!(next_event(&broker, process), RendererEvent::Crashed { process_id: process });
    assert_eq!(next_event(&broker, process), RendererEvent::Exited { process_id: process });
}

#[test]
fn terminating_process_removes_all_frame_routes() {
    let mut broker = ProcessBroker::default();
    let process = broker.spawn_renderer("https://gone.test", SandboxPolicy::renderer(4096)).unwrap();
    broker.attach_frame(process, 42).unwrap();
    assert!(broker.terminate(process));
    assert!(broker.send_to_frame(42, RendererCommand::Paint { frame_id: 42 }).is_err());
}
