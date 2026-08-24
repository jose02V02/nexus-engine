//! Nexus 0.80 web-platform foundations.
//!
//! This module deliberately separates capability claims from architecture.
//! A feature is `Available` only when a functional backend exists; the other
//! levels expose stable integration boundaries without pretending that a web
//! standard has already been implemented completely.

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};

use crate::dom::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmModuleState { Compiled, Instantiated, Trapped }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmModuleDescriptor {
    pub byte_length: usize,
    pub imports: Vec<String>,
    pub exports: Vec<String>,
    pub state: WasmModuleState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WebCapability {
    WebAssembly,
    DedicatedWorkers,
    ServiceWorkers,
    WebComponents,
    ObserverApis,
    StreamingFetch,
    MediaPlayback,
    WebRtc,
    WebGl,
    WebGpu,
    ProcessSandbox,
    TabDiscarding,
    BrowserShell,
    WebExtensions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SupportLevel { Planned, Foundation, Partial, Available }

#[derive(Debug, Clone)]
pub struct CapabilityRegistry {
    levels: HashMap<WebCapability, SupportLevel>,
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        use SupportLevel::{Available, Foundation, Partial};
        use WebCapability::*;
        Self { levels: HashMap::from([
            (WebAssembly, Partial),
            (DedicatedWorkers, Partial),
            (ServiceWorkers, Partial),
            (WebComponents, Partial),
            (ObserverApis, Partial),
            (StreamingFetch, Partial),
            (MediaPlayback, Partial),
            (WebRtc, Partial),
            (WebGl, Partial),
            (WebGpu, Partial),
            (ProcessSandbox, Partial),
            (TabDiscarding, Available),
            (BrowserShell, Partial),
            (WebExtensions, Foundation),
        ]) }
    }
}

impl CapabilityRegistry {
    #[must_use]
    pub fn level(&self, capability: WebCapability) -> SupportLevel {
        self.levels.get(&capability).copied().unwrap_or(SupportLevel::Planned)
    }

    #[must_use]
    pub fn is_available(&self, capability: WebCapability) -> bool {
        self.level(capability) == SupportLevel::Available
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerMessage { Text(String), Bytes(Vec<u8>), Shutdown }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerEvent { Ready, Message(WorkerMessage), Closed }

/// A real native background thread and bidirectional message channel. Script
/// realm creation is intentionally left to the JavaScript backend adapter.
pub struct DedicatedWorker {
    commands: Sender<WorkerMessage>,
    events: Receiver<WorkerEvent>,
    thread: Option<JoinHandle<()>>,
}

impl DedicatedWorker {
    pub fn spawn<F>(mut handler: F) -> Self
    where
        F: FnMut(WorkerMessage) -> Option<WorkerMessage> + Send + 'static,
    {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let thread = thread::spawn(move || {
            let _ = event_tx.send(WorkerEvent::Ready);
            while let Ok(message) = command_rx.recv() {
                if message == WorkerMessage::Shutdown { break; }
                if let Some(response) = handler(message) {
                    let _ = event_tx.send(WorkerEvent::Message(response));
                }
            }
            let _ = event_tx.send(WorkerEvent::Closed);
        });
        Self { commands: command_tx, events: event_rx, thread: Some(thread) }
    }

    pub fn post_message(&self, message: WorkerMessage) -> Result<(), String> {
        self.commands.send(message).map_err(|_| "worker channel closed".to_owned())
    }

    pub fn try_event(&self) -> Option<WorkerEvent> {
        match self.events.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }

    pub fn shutdown(&mut self) {
        let _ = self.commands.send(WorkerMessage::Shutdown);
        if let Some(thread) = self.thread.take() { let _ = thread.join(); }
    }
}

impl Drop for DedicatedWorker {
    fn drop(&mut self) { self.shutdown(); }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceWorkerState { Parsed, Installing, Activated, Redundant }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceWorkerRegistration {
    pub scope: String,
    pub script_url: String,
    pub state: ServiceWorkerState,
}

impl ServiceWorkerRegistration {
    pub fn activate(&mut self) {
        if matches!(self.state, ServiceWorkerState::Parsed | ServiceWorkerState::Installing) {
            self.state = ServiceWorkerState::Activated;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowRootMode { Open, Closed }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowRootDescriptor {
    pub host: NodeId,
    pub mode: ShadowRootMode,
    pub delegates_focus: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomElementDefinition {
    pub name: String,
    pub observed_attributes: Vec<String>,
}

#[derive(Debug, Default)]
pub struct CustomElementRegistry { definitions: HashMap<String, CustomElementDefinition> }

impl CustomElementRegistry {
    pub fn define(&mut self, definition: CustomElementDefinition) -> Result<(), String> {
        let name = definition.name.to_ascii_lowercase();
        if !name.contains('-') || self.definitions.contains_key(&name) {
            return Err("invalid or duplicate custom element name".to_owned());
        }
        self.definitions.insert(name, definition);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&CustomElementDefinition> {
        self.definitions.get(&name.to_ascii_lowercase())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlTemplateContent { pub fragment_nodes: Vec<NodeId> }

#[derive(Debug, Clone, PartialEq)]
pub enum ObserverRecord {
    Mutation { target: NodeId, kind: MutationKind },
    Intersection { target: NodeId, ratio: f32, is_intersecting: bool },
    Resize { target: NodeId, width: f32, height: f32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationKind { Attributes(String), ChildList, CharacterData }

#[derive(Debug, Default)]
pub struct ObserverHub { records: VecDeque<ObserverRecord> }

impl ObserverHub {
    pub fn enqueue(&mut self, record: ObserverRecord) { self.records.push_back(record); }
    pub fn drain(&mut self) -> Vec<ObserverRecord> { self.records.drain(..).collect() }
    #[must_use]
    pub fn pending(&self) -> usize { self.records.len() }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchChunk { pub bytes: Vec<u8>, pub final_chunk: bool }

#[derive(Debug, Default)]
pub struct StreamingBody {
    chunks: VecDeque<FetchChunk>,
    consumed_bytes: usize,
}

impl StreamingBody {
    pub fn push(&mut self, chunk: FetchChunk) { self.chunks.push_back(chunk); }
    pub fn read(&mut self) -> Option<FetchChunk> {
        let chunk = self.chunks.pop_front()?;
        self.consumed_bytes = self.consumed_bytes.saturating_add(chunk.bytes.len());
        Some(chunk)
    }
    #[must_use]
    pub fn consumed_bytes(&self) -> usize { self.consumed_bytes }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaCodec { Av1, Vp9, H264, Opus, Aac }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaElementKind { Audio, Video }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaState { Empty, Loading, Paused, Playing, Ended, Failed }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaPipelineDescriptor {
    pub kind: MediaElementKind,
    pub codecs: Vec<MediaCodec>,
    pub hardware_acceleration_preferred: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtcState { New, Connecting, Connected, Disconnected, Failed, Closed }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsApi { WebGl1, WebGl2, WebGpu }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackend { Vulkan, OpenGlEs, Software }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicsContextDescriptor {
    pub api: GraphicsApi,
    pub backend: GpuBackend,
    pub isolated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessRole { Browser, Renderer, Network, Gpu, Utility }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxPolicy {
    pub role: ProcessRole,
    pub network_allowed: bool,
    pub filesystem_allowed: bool,
    pub memory_limit_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessBoundary {
    pub role: ProcessRole,
    pub site_key: Option<String>,
    pub sandbox: SandboxPolicy,
}

impl SandboxPolicy {
    #[must_use]
    pub fn renderer(memory_limit_bytes: usize) -> Self {
        Self { role: ProcessRole::Renderer, network_allowed: false, filesystem_allowed: false, memory_limit_bytes }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateTrust { Trusted, UserAccepted, Expired, HostMismatch, Untrusted }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateInfo {
    pub subject: String,
    pub issuer: String,
    pub valid_from_unix: i64,
    pub valid_until_unix: i64,
    pub trust: CertificateTrust,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub permissions: Vec<String>,
    pub content_scripts: Vec<String>,
}

#[derive(Debug, Default)]
pub struct ExtensionRegistry { manifests: HashMap<String, ExtensionManifest> }

impl ExtensionRegistry {
    pub fn install(&mut self, manifest: ExtensionManifest) -> Result<(), String> {
        if manifest.id.trim().is_empty() || manifest.name.trim().is_empty() || manifest.version.trim().is_empty() {
            return Err("invalid extension manifest".to_owned());
        }
        self.manifests.insert(manifest.id.clone(), manifest);
        Ok(())
    }

    pub fn uninstall(&mut self, id: &str) -> bool { self.manifests.remove(id).is_some() }
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&ExtensionManifest> { self.manifests.get(id) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn capability_registry_does_not_overclaim_foundations() {
        let registry = CapabilityRegistry::default();
        assert_eq!(registry.level(WebCapability::WebAssembly), SupportLevel::Foundation);
        assert!(!registry.is_available(WebCapability::WebGpu));
        assert_eq!(registry.level(WebCapability::DedicatedWorkers), SupportLevel::Partial);
        assert!(registry.is_available(WebCapability::TabDiscarding));
    }

    #[test]
    fn dedicated_worker_round_trips_messages_on_a_native_thread() {
        let mut worker = DedicatedWorker::spawn(|message| match message {
            WorkerMessage::Text(text) => Some(WorkerMessage::Text(text.to_uppercase())), _ => None,
        });
        worker.post_message(WorkerMessage::Text("nexus".to_owned())).unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut reply = None;
        while Instant::now() < deadline {
            if let Some(WorkerEvent::Message(message)) = worker.try_event() { reply = Some(message); break; }
            thread::yield_now();
        }
        assert_eq!(reply, Some(WorkerMessage::Text("NEXUS".to_owned())));
        worker.shutdown();
    }

    #[test]
    fn observers_are_delivered_in_enqueue_order() {
        let mut hub = ObserverHub::default();
        hub.enqueue(ObserverRecord::Mutation { target: 2, kind: MutationKind::ChildList });
        hub.enqueue(ObserverRecord::Resize { target: 2, width: 320.0, height: 80.0 });
        assert_eq!(hub.pending(), 2);
        assert!(matches!(hub.drain()[0], ObserverRecord::Mutation { .. }));
    }

    #[test]
    fn custom_elements_require_a_hyphen_and_are_unique() {
        let mut registry = CustomElementRegistry::default();
        assert!(registry.define(CustomElementDefinition { name: "nexus-card".to_owned(), observed_attributes: vec!["title".to_owned()] }).is_ok());
        assert!(registry.define(CustomElementDefinition { name: "card".to_owned(), observed_attributes: Vec::new() }).is_err());
        assert!(registry.get("NEXUS-CARD").is_some());
    }

    #[test]
    fn streaming_body_consumes_network_chunks_incrementally() {
        let mut body = StreamingBody::default();
        body.push(FetchChunk { bytes: vec![1, 2], final_chunk: false });
        body.push(FetchChunk { bytes: vec![3], final_chunk: true });
        assert_eq!(body.read().unwrap().bytes, vec![1, 2]);
        assert_eq!(body.consumed_bytes(), 2);
        assert!(body.read().unwrap().final_chunk);
    }

    #[test]
    fn renderer_sandbox_denies_direct_io() {
        let policy = SandboxPolicy::renderer(256 * 1024 * 1024);
        assert!(!policy.network_allowed);
        assert!(!policy.filesystem_allowed);
    }

    #[test]
    fn extension_registry_validates_and_tracks_manifests() {
        let mut registry = ExtensionRegistry::default();
        registry.install(ExtensionManifest { id: "reader".to_owned(), name: "Reader".to_owned(), version: "1.0".to_owned(), permissions: vec!["tabs".to_owned()], content_scripts: vec!["reader.js".to_owned()] }).unwrap();
        assert!(registry.get("reader").is_some());
        assert!(registry.uninstall("reader"));
    }
}
