# Nexus Android Shell 1.02

The Android shell is browser chrome and input integration for the Rust Nexus engine; it does not use WebView.

## 1.02 behavior

- Wasm passive element segments retain per-instance lifecycle state.
- `table.init`, `elem.drop`, overlap-safe `table.copy` and bounded `table.fill` execute in Rust.
- Bulk range failures are checked before table mutation.

- Wasm modules can import explicitly registered bounded `funcref` tables.
- Linked instances share table mutations and growth through synchronized host storage.
- Reference creation, table get/set/size/grow and type-safe indirect calls execute in Rust.
- Missing table capabilities and limit mismatches stop instantiation.

- Wasm modules can import one explicitly registered bounded linear memory.
- Host and linked instances share synchronized bytes and memory growth.
- Missing capabilities and minimum/maximum mismatches stop instantiation.
- Loads, stores, `memory.size` and `memory.grow` operate on the shared memory.

- Wasm modules can import explicitly registered typed host globals.
- Linked instances share synchronized host-global state while defined globals remain isolated.
- Missing capabilities and type/mutability mismatches stop instantiation.

- Wasm instances now retain typed mutable and immutable global state.
- Global reads and writes enforce index, mutability and exact value type.
- Defined global mutations remain isolated between independent instances.

- Wasm instances now own bounded function-reference tables initialized by active element segments.
- Indirect calls verify table bounds, initialized slots and exact signatures before dispatch.
- Tables may reference both internal functions and explicitly authorized host imports.

- Wasm modules can declare typed function imports and call explicitly registered host functions.
- Missing capabilities and signature mismatches stop instantiation before module execution.
- No DOM, network, filesystem or Android API is exposed automatically to Wasm.

- WebAssembly now executes nested blocks, loops and conditional arms.
- Depth-aware branches and indexed branch tables run under a fixed execution budget.
- Imported/shared globals, memory and `funcref` tables are executable; multiple tables, reference-expression elements, multi-value blocks, SIMD, threads, direct JavaScript bindings and optimized compilation remain future stages.

- Native Worker threads now own independent QuickJS execution contexts.
- Messages, Promise checkpoints and Worker timers run outside the page realm.
- Worker memory, stack, runtime and queue limits constrain background scripts.
- Page-level Worker construction and URL script loading remain future bindings.

- Android memory-pressure callbacks now invoke the Rust Resource Governor.
- Moderate pressure discards one eligible inactive page; critical pressure can discard all eligible pages.
- Active, pinned, audible and empty tabs remain protected.
- CI builds the ARM64 JNI library and debug APK, then publishes its SHA-256 and audit report.

- Cross-origin-isolated pages can own bounded shared-memory buffers in Rust.
- Sequentially consistent Atomics and per-cell wait/notify are available to Worker adapters.
- Structured messages and shared-buffer references cross native Worker channels.
- Independent QuickJS Worker realms and direct JavaScript typed-array bindings remain future work.

- Validated WebAssembly integer modules can execute inside the Rust engine.
- Export calls enforce typed arguments, locals, call targets and controlled traps.
- Unsupported Wasm features fail closed instead of entering the runtime.
- Imported globals/memory/tables, passive elements, bulk table mutation and indirect calls are executable; reference-expression elements, SIMD, threads, direct JavaScript bindings and optimized compilation remain future stages.

- SDP, ICE candidates and WebRTC signaling are validated in Rust.
- MediaStream tracks carry capture constraints and explicit lifecycle state.
- DataChannel queues enforce memory backpressure before transport.
- Camera/microphone capture, STUN/TURN, DTLS-SRTP and SCTP remain Android/native adapters.

- The media runtime discovers MP4/WebM/Ogg/AAC streams and resolves modern codecs.
- Decoder selection can prefer Android MediaCodec while respecting resolution and secure-stream constraints.
- Playback queues, clock, seek, buffering, EOS and A/V timing are handled by Rust.
- JNI MediaExtractor/MediaCodec, AudioTrack and Surface adapters remain the next device backend.

- WebGL 2 and WebGPU now have validated resource, shader and command models.
- WebGPU compute dispatch and device limits are enforced before backend submission.
- Frame pacing accepts Android display targets through 120 Hz.
- Vulkan/WGPU hardware surfaces and on-device performance verification remain backend work.

- The engine exposes a renderer broker with bounded IPC and per-site frame routing.
- Renderer policy denies direct network/filesystem access and applies memory budgets.
- Renderer failures are reported to the browser boundary without taking down the broker.
- Android isolated services and Binder/shared memory remain the next transport adapter.

- Android memory-pressure signals can drive moderate or critical inactive-tab discarding.
- Discard releases the loaded page, decoded resources, QuickJS realm and event loop.
- Active, pinned and audible tabs are protected; activation restores URL, history, scroll and zoom.

- DOM mutation and layout geometry checkpoints now feed the native Observer runtime.

- Custom Element upgrades, Shadow Roots, templates and slot assignment run in the Rust DOM layer.

- Offline Cache Storage, PWA manifests and Service Worker scope selection are available to the Rust browser core.

- Native worker threads and explicit tab-discarding states are available to the Rust browser core.
- Media/GPU/process/certificate boundaries are ready for Android backend adapters without adding WebView.

- Modern RGB/HSL, alpha and `currentColor` values are resolved before native Skia paint commands.

- Modern image fitting and positioning now feed the native Skia display list.
- Hidden content is not painted and pointer-disabled content is excluded from touch hit-testing.

- Preserved whitespace, nowrap/pre wrapping, text transformation and first-line indentation reach the native pipeline.

- Text weight, italic/oblique intent, line height, alignment and decorations now reach the native renderer.
- The Android shell remains unchanged: all page typography is painted by Nexus/Skia.

The UI is intentionally stable while the engine gains advanced CSS compatibility. Existing touch, fling, pinch zoom, long press, forms, file picker, tabs, private mode, bookmarks, downloads and `nexus://` browser pages remain available.

Viewport changes caused by device size or browser zoom trigger viewport-aware CSS recomputation, including responsive named Grid templates. Fixed/sticky/transformed elements remain painted and hit-tested using shared visual geometry.

Android package version: `1.2.0` (`versionCode 102`).
