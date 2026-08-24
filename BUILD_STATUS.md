# Nexus Engine 1.02 — Build Status

## Implemented in this release

- Capability-scoped imported globals with synchronized shared host state.
- Exact imported-global type and mutability validation before instantiation.
- Shared `global.get` / `global.set` behavior across linked instances.
- Typed i32/i64/f32/f64 Wasm globals with constant initializers.
- Mutable/immutable global declarations and `global.get` / `global.set`.
- Per-instance global persistence and cross-instance isolation.
- Global index, mutability and runtime type enforcement.
- Bounded MVP `funcref` table declarations with a 4,096-slot ceiling.
- Active element segments with checked per-instance initialization.
- Type-safe `call_indirect` across defined and imported functions.
- Null-element, table-bounds and indirect-signature traps.
- Wasm function-import parsing with module, name and type metadata.
- Unified function indices across imported and internally defined functions.
- Capability-scoped host registry with explicit linked instantiation.
- Pre-execution missing-import and exact-signature validation.
- Runtime host argument/result checking and controlled trap propagation.
- Nested Wasm `block`, `loop` and single-value `if` / `else` execution.
- Label-depth routing for `br`, `br_if` and `br_table`.
- Parser-side block/else/end matching and implicit function labels.
- 100,000-instruction per-call budget with deterministic exhaustion trap.
- Full-width i32/i64 and narrow 8/16/32-bit scalar memory operations.
- Signed/unsigned load extension and truncating narrow stores.
- i32/i64 equality, zero and signed relational comparisons.
- Type-checked Wasm `select` with integer condition semantics.
- Centralized little-endian address, overflow and bounds enforcement.
- Persistent WebAssembly instances with independently owned linear memory.
- Validated memory declarations capped at 256 pages per instance.
- Little-endian `i32.load` / `i32.store` and deterministic bounds traps.
- `memory.size` and maximum-aware `memory.grow`.
- Bit-preserving `f32` / `f64` constants, results and basic arithmetic.
- Independent QuickJS context ownership on each Worker thread.
- Bounded Worker memory, stack and execution duration.
- `postMessage`, `onmessage` and message listener delivery.
- Promise microtask checkpoints following messages.
- Worker timer queue and explicit event-loop checkpoints.
- Isolated script errors, queue backpressure and deterministic termination.
- Android `onTrimMemory()` integration with moderate/critical native pressure handling.
- Fail-closed release audit for package versions, TOML/XML, JNI parity and WebView exclusion.
- Pinned Rust 1.88.0 desktop and Android CI toolchains.
- One-command Rust test, release, NDK and debug-APK build procedure.
- Correct versioned CI artifacts, APK SHA-256 output and audit report publication.
- Cross-origin-isolated SharedArrayBuffer with bounded allocation.
- Sequentially consistent i32 atomic load/store and read-modify-write operations.
- Compare-exchange and per-cell Atomics wait/notify with timeout results.
- Structured clone values including shared-buffer identity.
- Bounded native Worker messaging, response events and backpressure.
- Deterministic Worker shutdown and shared-memory lifetime ownership.
- WebAssembly binary magic/version, section ordering and size validation.
- Bounded signed and unsigned LEB128 decoding.
- Type, function, export and code section parsing.
- Typed i32/i64 exported-function calls, locals and internal calls.
- Wrapping integer arithmetic and signed division traps.
- Stack/type/index/recursion validation with fail-closed unsupported features.
- SDP offer/answer, BUNDLE, RTP codec, ICE credential and DTLS fingerprint parsing.
- ICE host/reflexive/relay candidate validation and compatible pair selection.
- Peer signaling and ICE connection state transitions.
- MediaStream audio/video constraints and track lifecycle.
- DataChannel state, bounded queues and backpressure.
- Peer shutdown that closes channels and rejects later mutations.
- MP4, WebM, Ogg and ADTS-AAC container signature discovery.
- AV1, VP9, H.264, Opus and AAC codec-string resolution.
- Decoder capability registry with hardware, secure-stream and resolution selection.
- Encoded packet and timestamp-ordered decoded audio/video queues.
- Monotonic media clock with play, pause, seek and playback-rate semantics.
- Presentation scheduling, late-video dropping, buffering and EOS state handling.
- Validated WebGL 2 and WebGPU API/resource runtime.
- GLSL ES 3.00 and WGSL shader/API matching plus compute-stage policy.
- Buffer usages, bounded writes, texture limits and resource accounting.
- Render-pass lifecycle, draw and compute command encoding.
- Submission validation and draw/dispatch accounting.
- 30–120 Hz frame scheduler with missed-frame tracking.
- Explicit hardware adapter targets and an executable software adapter.
- Renderer execution broker with stable process/frame ownership.
- Site-key grouping and explicit frame-to-renderer routing.
- Bounded IPC command queues for document, paint, input and lifecycle messages.
- Renderer network/filesystem policy denial and retained-document memory limits.
- Crash/exit reporting that keeps renderer failure inside the broker boundary.
- Deterministic renderer shutdown and stale frame-route cleanup.
- Moderate/critical memory-pressure policies with LRU inactive-tab selection.
- Real release of loaded pages, decoded resources, QuickJS realms and event loops.
- Discard snapshots preserving URL, title, history position, scroll and zoom.
- Automatic discarded-tab reload and visual-state restoration on activation.
- Active, pinned and audible tab protection plus released-memory estimates.
- MutationObserver filtering, subtree matching and old-value records for Nexus DOM mutations.
- IntersectionObserver threshold processing against viewport/scroll geometry.
- Per-observer ResizeObserver checkpoints with duplicate suppression.
- Custom Element upgrade and lifecycle reaction queues.
- Open/closed Shadow Root ownership and template cloning.
- Named/default slot assignment for light-DOM children.
- Quota-controlled Cache Storage and offline fetch routing.
- Secure-origin Service Worker registrations and longest-scope controller selection.
- PWA manifest parsing with stable defaults.
- Native background worker execution and bidirectional message transport.
- Web-platform capability registry with non-inflated support levels.
- Service Worker, Wasm, Web Components, streaming Fetch, media, WebRTC and GPU integration contracts.
- Observer record delivery, renderer sandbox policy, certificate metadata and extension registry.
- Active/suspended/frozen/discarded tab lifecycle and memory-pressure discarding.
- Modern RGB/HSL functions, alpha syntax and hexadecimal alpha colors.
- Cascade-aware `currentColor` paint resolution.
- Expanded named-color compatibility and color feature queries.
- Aspect-ratio-aware `object-fit` and positioned replaced-content paint geometry.
- `visibility` paint suppression with inherited overrides.
- `pointer-events` filtering in shared visual hit-testing.
- CSS whitespace preservation and wrap control.
- Uppercase, lowercase and capitalize text transformation.
- First-line text indentation across style, layout and paint.
- CSS font weight/style, line height and text alignment.
- Text underline, overline and line-through Display List geometry.
- Compact `font` shorthand and typography-aware `@supports` declarations.
- Inherited typography connected to text layout and Skia typeface matching.
- Named `grid-template-areas` parsing and `grid-area` layout resolution.
- Rectangular template validation and automatic missing track generation.
- Modern `aspect-ratio` and `box-sizing` layout semantics.
- Flex wrapping, basis, `flex`/`flex-flow` shorthands and stable visual ordering.
- Advanced explicit and implicit CSS Grid tracks.
- Dynamic `auto-fill` / `auto-fit` Grid repetition.
- Grid/Flexbox container and item alignment plus `place-*` shorthands.
- `minmax()`, `fit-content()`, numbered-line and `span` item placement.
- px/percentage/fractional/intrinsic track sizing.
- Bounded integer `repeat()` expansion.
- Row/column/dense auto-flow.
- Independent row/column gaps and Taffy Grid mapping.
- Property/value and selector-based `@supports` feature queries.
- Boolean/nested support-condition evaluation.
- Responsive `min()` / `max()` / `clamp()` functions.
- Extended `calc()` arithmetic and nested CSS math.
- Advanced attribute matching operators and ASCII case-insensitive values.
- Forward/reverse `of-type` structural pseudo-classes.
- `inherit`, `initial` and `unset` for the Nexus computed-style model.
- Global-value behavior for custom properties.
- Adjacent/general sibling combinators.
- Functional `:not()` / `:is()` / `:where()` selector lists.
- Structural, link and form-state pseudo-classes.
- Property-aware normal/`!important` cascade passes, including inline declarations and custom properties.
- Variable-aware generated-content declarations.
- Descendant and direct-child selector combinators.
- Attribute presence/equality selectors and structural pseudo-classes.
- Inherited CSS custom properties with `var()` fallbacks.
- Initial `calc()` evaluation and responsive `rem`/`em`/viewport units.
- Responsive `@media` rules tied to the actual Nexus viewport.
- Taffy-backed relative/absolute positioning and inset geometry.
- Nexus fixed/sticky paint behavior during scroll.
- Overflow layout semantics plus ancestor paint clipping.
- `z-index` and a new renderer-neutral compositing/paint-order module.
- Transform foundation (`translate*`, `scale*`).
- Opacity foundation for colors/text/borders.
- `::before` / `::after` generated text content.
- Shared visual geometry for renderer and hit-testing.
- Android shell updated to versionCode 102 / versionName 1.02.0.

## Audit performed in this environment

The release is statically audited because the execution container does not ship Rust/Cargo.

- Cargo TOML files parse successfully.
- GitHub Actions YAML files parse successfully.
- Android XML files parse successfully.
- Android Kotlin/JNI source structure is included; Kotlin compilation remains a CI gate in this container.
- 39 Kotlin JNI declarations match 39 Rust JNI exports by name.
- No `WebView` / `android.webkit` use exists in Android app source.
- The 1.02 release audit executed successfully in this environment.
- `cssparser` 0.37 and Taffy 0.13 APIs used by this release were checked against current docs.rs documentation.
- 100 Rust source/test/native files are present.
- 23,081 Rust lines are present in the 1.02 source tree.
- 2 Kotlin files / 1,127 Kotlin lines are present.
- The 1.02 project declares 290 Rust tests.

## Compilation limitation

`cargo` and `rustc` are not installed in this execution environment. The network namespace used by shell commands also cannot resolve `static.rust-lang.org`, so a local Rust toolchain cannot be installed here. A successful Rust compilation is therefore **not claimed locally**.

The included GitHub Actions are the authoritative gates:

```text
cargo test --all-targets
cargo build --release
render smoke test
BrowserCore smoke test
Android NDK arm64 build
APK build
```

## Known 1.02 compatibility boundaries

- The platform matrix distinguishes executable support from architecture foundations; see `WEB_PLATFORM_102.md`.
- WebAssembly imported/shared memory, globals and tables, passive elements, bulk funcref operations, indirect calls and function imports are executable; reference-expression elements, multi-value, SIMD, threads, direct JavaScript bindings and optimized compilation remain future work.
- Independent JavaScript Worker realms and event checkpoints are executable; page-level `new Worker(url)`, network script loading, transferables and complete structured clone bindings remain future work.
- Source and release metadata pass the local audit, but no APK is claimed: Rust/Cargo, Android SDK/NDK and Gradle are unavailable in this container, so CI must produce and device-test the first binary.
- Shared memory, i32 Atomics and bounded structured Worker messages are executable; independent QuickJS Worker realms, transferable ownership and direct JavaScript bindings remain future work.
- WebRTC signaling, SDP/ICE validation, streams and DataChannel queues are executable; STUN/TURN networking, connectivity checks, DTLS-SRTP, SCTP and Android capture remain backend work.
- Media discovery, decoder selection, queues, clock and presentation are executable; Android MediaCodec/MediaExtractor decode, AudioTrack output, Surface rendering and DOM media events remain backend work.
- WebGL 2/WebGPU resource, shader, command and compute validation is executable; Vulkan/WGPU hardware submission, canvas presentation and measured 120 FPS remain future backend work.
- Renderer ownership, bounded IPC, budgets, policy and crash isolation are executable through the portable native-thread backend. Android OS processes, Binder/shared-memory transport and kernel-enforced sandboxing remain future backend work.
- Tab discarding releases in-process engine resources and restores navigation/visual state; OS renderer-process termination and heap-exact accounting require the future multi-process broker.
- Observer computation and record queues are functional; automatic microtask checkpoints and JavaScript callback bindings remain future work.
- Web Components upgrade/reaction, Shadow Root, template and slot models are functional; JavaScript constructors, composed events and shadow CSS encapsulation remain future work.
- Service Worker caching and route selection are functional, but automatic network interception, persistent disk Cache Storage and independent JavaScript worker realms remain future work.
- Wasm, Service Worker fetch interception, complete Shadow DOM, hardware codecs, WebRTC secure transports and GPU hardware execution still require their dedicated backends and are not claimed complete.

- Media queries: basic min/max width/height + orientation, not the complete Media Queries specification.
- Grid: explicit/implicit tracks, integer/automatic repeat, advanced sizing, auto-flow, gaps, alignment, numbered placement and named areas are implemented; named line lists and subgrid remain future work.
- Flexbox: direction, wrapping, basis, grow/shrink, gaps, alignment and order are implemented; baseline edge cases and complete intrinsic sizing remain future work.
- Values: `var()` and initial additive `calc()` are implemented; multiplication/division, mixed percentage algebra and full cycle diagnostics remain future work.
- Stacking contexts: initial flattened paint-order model, not full nested CSS stacking contexts.
- Sticky: first top/bottom viewport clamp, not every scroll-container edge case.
- Transforms: translate/scale only; no rotate/skew/matrix/3D or transform-origin yet.
- Generated content: paint-only, not independent Taffy layout boxes.
- Typography: single-run inherited text styling; justification, font fallback runs, letter/word spacing and full decoration propagation remain future work.
- Text flow: normal/nowrap/pre/pre-wrap are implemented; `pre-line`, advanced emergency wrapping and hyphenation remain future work.
- Replaced content: object fit and percentage/keyword positioning are implemented; mixed length offsets and video/canvas sizing remain future work.
- Pointer events: HTML auto/none policy is implemented; SVG-specific pointer-event modes remain future work.
- Colors: RGB/HSL, alpha hex, named colors and `currentColor` are implemented; Lab/LCH/OKLab, `color()` and `color-mix()` remain future work.
- Opacity: applied to painted colors; image/group compositing opacity will move to the future GPU compositor.
