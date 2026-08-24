//! Nexus Engine 1.02
//!
//! Pipeline:
//! `URL/origin -> Reqwest + cookie jar + HTTP cache -> HTML -> Nexus DOM ->
//! persistent QuickJS realm + SOP/CORS/Web Storage -> CSS/Taffy -> Display List
//! -> Skia`.
//!
//! 1.02 adds passive WebAssembly element segments and bounded bulk table
//! initialization, dropping, copying, and filling.

pub mod address;
pub mod settings;
pub mod internal_pages;
pub mod browser;
pub mod bookmarks;
pub mod scheduler;
pub mod autocomplete;
pub mod download;
pub mod favicon;
pub mod forms;
pub mod gpu_runtime;
pub mod event_loop;
pub mod policy;
pub mod pwa;
pub mod websocket;
pub mod worker_realm;
pub mod wasm_runtime;
pub mod wasm_host;
pub mod webrtc_runtime;
pub mod web_platform;
pub mod storage;
pub mod state;
pub mod security;
pub mod permissions;
pub mod origin;
pub mod observers;
pub mod cache;
pub mod css;
pub mod compositing;
pub mod concurrency;
pub mod components;
pub mod display_list;
pub mod dom;
pub mod encoding;
pub mod engine;
pub mod error;
pub mod hit_test;
pub mod selection;
pub mod javascript;
pub mod layout;
pub mod media_runtime;
pub mod network;
pub mod parser;
pub mod process_broker;
pub mod renderer;
pub mod resource;
pub mod session;
pub mod style_engine;
pub mod text;


pub use settings::{BrowserSettings, SettingsStore};
pub use internal_pages::InternalPage;
pub use autocomplete::{AddressHistory, AddressSuggestion, SuggestionSource};
pub use bookmarks::{Bookmark, BookmarkStore};
pub use scheduler::{SchedulerPolicy, TabLifecycle, TabScheduler};
pub use browser::{BrowserCore, BrowserCoreConfig, BrowserDataKind, MemoryPressure, ResourceReleaseReport, TabId, TabPrivacy, TabSummary};
pub use download::{DownloadItem, DownloadManager, DownloadStatus, DEFAULT_MAX_DOWNLOAD_BYTES};
pub use favicon::{discover_favicon_urls, fetch_favicon, FaviconData};
pub use gpu_runtime::{
    BufferDescriptor, BufferUsage, CommandEncoder, EncodedCommand, FrameScheduler, GpuAdapter,
    GpuBackend as RuntimeGpuBackend, GpuDevice, GpuError, GpuLimits, GpuResourceId,
    GraphicsStandard, ShaderLanguage, ShaderModule, ShaderStage, SubmissionReport,
    TextureDescriptor, TextureFormat,
};
pub use cache::{CachedHttpResponse, HttpCache, HttpCacheStats};
pub use event_loop::{BrowserEventLoop, EventLoopStats, WebSocketCommand, WebSocketEvent};
pub use policy::{CspPolicy, HstsEntry, HstsStore, PageSecurityContext, ReferrerPolicy, is_mixed_active_content};
pub use pwa::{
    route_fetch, CacheStorage, CacheStorageError, CachedFetchResponse, FetchRoute, FetchStrategy,
    PwaManifest, ServiceWorkerError, ServiceWorkerManager, parse_pwa_manifest,
};
pub use origin::Origin;
pub use observers::{
    IntersectionEntry, MutationObserverOptions, MutationRecord, MutationRecordKind, ObservedDom,
    ObserverId, ObserverRuntime, ResizeEntry,
};
pub use permissions::{PermissionKind, PermissionState, PermissionStore};
pub use process_broker::{
    FrameId, ProcessBroker, ProcessId, RendererCommand, RendererEvent, RendererProcessSnapshot,
    SandboxCapability,
};
pub use security::{CredentialsMode, FetchMode, SecurityError};
pub use state::BrowserState;
pub use storage::{LocalStorageStore, SessionStorage, StorageArea, StorageError};
pub use websocket::WebSocketRequest;
pub use worker_realm::{JavaScriptWorker, WorkerRealmError, WorkerRealmEvent, WorkerRealmStats};
pub use wasm_runtime::{WasmError as RuntimeWasmError, WasmExport, WasmGlobal, WasmGlobalImport, WasmImport, WasmInstance, WasmMemoryImport, WasmModule as RuntimeWasmModule, WasmTableImport, WasmValue, WasmValueType};
pub use wasm_host::{HostError as WasmHostError, HostImportKey, HostSignature, WasmHostRegistry};
pub use webrtc_runtime::{
    CandidatePair, DataChannel, DataChannelState, DataMessage, IceCandidate, IceCandidateType,
    IceConnectionState, IceTransport, MediaDirection, MediaKind, MediaStream, MediaStreamTrack,
    MediaTrackConstraints, PeerConnection, RtcError, RtpCodec, SdpMediaSection, SdpType,
    SessionDescription, SignalingState, TrackState,
};
pub use web_platform::{
    CapabilityRegistry, CertificateInfo, CertificateTrust, CustomElementDefinition,
    CustomElementRegistry, DedicatedWorker, ExtensionManifest, ExtensionRegistry, FetchChunk, GraphicsApi,
    GraphicsContextDescriptor, GpuBackend, MediaCodec, MediaElementKind, MediaPipelineDescriptor,
    MediaState, MutationKind, ObserverHub, ObserverRecord, ProcessBoundary, ProcessRole, RtcState,
    SandboxPolicy, ServiceWorkerRegistration, ServiceWorkerState, ShadowRootDescriptor,
    ShadowRootMode, StreamingBody, SupportLevel, WasmModuleDescriptor, WasmModuleState,
    WebCapability, WorkerEvent, WorkerMessage, HtmlTemplateContent,
};
pub use network::{DownloadTransfer, SubresourceKind};
pub use media_runtime::{
    parse_codec_string, sniff_container, DecodedMedia, DecoderBackend, DecoderCapability,
    DecoderRegistry, DecoderRequest, EncodedPacket, MediaClock, MediaContainer, MediaError,
    MediaPipeline, MediaTrack, PixelFormat, PlaybackSnapshot,
};
pub use css::{
    compute_styles_for_viewport, ComputedStyle, CssBoxSizing, CssDisplay, CssFlexDirection, CssFlexWrap, CssGridAutoFlow,
    CssContentAlignment, CssFontStyle, CssGridBreadth, CssGridLine, CssGridPlacement, CssGridRepeat,
    CssGridTemplateAreas, CssNamedGridArea, CssGridTrack,
    CssItemAlignment, CssLength, CssLineHeight, CssObjectFit, CssObjectPosition, CssPointerEvents, CssTextAlign,
    CssTextDecoration, CssTextTransform, CssVisibility, CssWhiteSpace,
    CssOverflow, CssPosition, CssTransform, EdgeSizes, MediaEnvironment, PseudoElement,
    PseudoStyle, Rgba, StyleMap,
};
pub use compositing::{effective_z_index, paint_order_indices, stacking_contexts, StackingContextEntry, StackingReason};
pub use concurrency::{AtomicWaitResult, ConcurrencyError, ConcurrentWorker, ConcurrentWorkerEvent, SharedArrayBuffer, StructuredCloneValue};
pub use components::{
    ComponentError, ComponentRuntime, CustomElementReaction, ShadowNode, ShadowTree,
    SlotAssignment, TemplateBlueprint,
};
pub use display_list::{
    build_display_list, build_display_list_with_resources, DisplayCommand, DisplayList,
    ImageAsset, PaintRect,
};
pub use dom::{
    DomAttribute, DomNode, DomNodeData, ImageReference, Link, NexusDom, NodeId, ScriptReference,
};
pub use engine::{LoadedPage, NexusEngine, NexusEngineBuilder};
pub use error::{NexusError, NexusResult};
pub use hit_test::{hit_test_page, HitTestResult};
pub use selection::{selection_at, SelectionInfo};
pub use javascript::{
    ConsoleEntry, JavascriptActivity, JavascriptEngine, JavascriptReport, QuickJsEngine,
    QuickJsRealm, DEFAULT_JS_EXECUTION_TIMEOUT_MS, DEFAULT_JS_MEMORY_LIMIT,
    DEFAULT_JS_STACK_LIMIT, DEFAULT_MAX_PAGE_SCRIPT_BYTES, DEFAULT_MAX_SCRIPT_BYTES,
    DEFAULT_MAX_SCRIPTS,
};
pub use layout::{
    compute_layout, compute_layout_with_intrinsics, IntrinsicSizeMap, LayoutBox, LayoutTree,
    Viewport,
};
pub use renderer::{Renderer, SkiaRenderer};
pub use resource::{
    ImageResource, PageResources, ResourceCache, DEFAULT_MAX_IMAGE_PIXELS,
    DEFAULT_MAX_PAGE_DECODED_IMAGE_BYTES,
};
pub use session::{
    BrowserSession, DiscardedPageState, HistoryEntry, InteractionResult, NavigationKind, NavigationResult,
    SessionSnapshot,
};
pub use style_engine::{NexusStyleEngine, StyleEngine};
pub use text::{ParleyTextEngine, TextLayout, TextLayoutEngine, TextLayoutOptions, TextLine};

pub use forms::{FormControlDescriptor, SelectOption, SelectedFile};
