# Changelog

## Nexus Engine 1.02.0 — WebAssembly Bulk Table Operations

- Added passive function-index element-segment parsing and per-instance lifecycle state.
- Added `table.init` and idempotent `elem.drop` execution.
- Added overlap-safe `table.copy` and bounded `table.fill` execution.
- Added pre-mutation source/destination bounds validation for atomic failure behavior.
- Preserved imported-table synchronization and indirect-call type enforcement.
- Added four passive-element and bulk-table regression tests.
- Android versionCode 102 / versionName 1.02.0.

## Nexus Engine 1.01.0 — WebAssembly Imported & Mutable Tables

- Added capability-scoped host registration for bounded shared `funcref` tables.
- Added table-import parsing and exact minimum/maximum linking validation.
- Added `ref.null`, `ref.func`, `table.get`, `table.set`, `table.size` and `table.grow` execution.
- Connected active elements and indirect calls to synchronized imported table storage.
- Added runtime validation for function references written into tables.
- Added four cross-instance and linking regression tests.
- Android versionCode 101 / versionName 1.01.0.

## Nexus Engine 1.00.0 — WebAssembly Imported Memory

- Added capability-scoped host registration for bounded Wasm linear memories.
- Added memory-import section parsing with module/name and minimum/maximum metadata.
- Added pre-instantiation memory presence and exact-limit validation.
- Connected all scalar loads/stores plus `memory.size` and `memory.grow` to synchronized shared storage.
- Added bounded host-side memory reads and writes.
- Added four end-to-end imported-memory regression tests.
- Android versionCode 100 / versionName 1.00.0.

## Nexus Engine 0.99.0 — WebAssembly Imported Globals

- Added capability-scoped typed global registration in the Wasm host linker.
- Added shared host-global state backed by synchronized safe Rust ownership.
- Added global import parsing for i32/i64/f32/f64 and mutability metadata.
- Added pre-instantiation global presence, type and mutability validation.
- Connected `global.get` / `global.set` to imported shared state.
- Added four end-to-end imported-global regression tests.
- Android versionCode 99 / versionName 0.99.0.

## Nexus Engine 0.98.0 — WebAssembly Globals

- Added typed i32/i64/f32/f64 global declarations and constant initializers.
- Added mutable and immutable global state owned independently by each instance.
- Added `global.get` and `global.set` execution.
- Added index, mutability and runtime value-type enforcement.
- Added a 1,024-global engine policy ceiling.
- Added seven global-state regression tests.
- Updated the platform matrix and advanced roadmap.
- Android versionCode 98 / versionName 0.98.0.

## Nexus Engine 0.97.0 — WebAssembly Tables & Indirect Calls

- Added bounded `funcref` table declarations with a 4,096-slot engine ceiling.
- Added active MVP element segments and per-instance table initialization.
- Added `call_indirect` across internal and capability-scoped imported functions.
- Added runtime signature checking for indirect calls.
- Added deterministic null-element, bounds and signature-mismatch traps.
- Added seven table and indirect-call regression tests.
- Updated the platform matrix and advanced roadmap.
- Android versionCode 97 / versionName 0.97.0.

## Nexus Engine 0.96.0 — WebAssembly Host Imports

- Added function-import section parsing with module/name/type metadata.
- Added unified Wasm function indices across imported and defined functions.
- Added capability-scoped host registration and explicit linked instantiation.
- Added pre-execution host signature and missing-import validation.
- Added typed argument/result checks and controlled host-trap propagation.
- Added public import metadata for JavaScript/embedder bridge construction.
- Added fourteen host ABI and binary-linking regression tests.
- Updated the platform matrix and advanced roadmap.
- Android versionCode 96 / versionName 0.96.0.

## Nexus Engine 0.95.0 — WebAssembly Structured Control Flow

- Added nested `block`, `loop` and single-value `if` / `else` execution.
- Added depth-aware `br`, conditional `br_if` and indexed `br_table`.
- Added an implicit function label for valid top-level branches.
- Added parser-side matching of block, else and end boundaries.
- Added a 100,000-instruction execution budget per function call.
- Added controlled traps for invalid branch depths and exhausted execution budgets.
- Added seven structured-control regression tests.
- Updated the platform matrix and advanced roadmap.
- Android versionCode 95 / versionName 0.95.0.

## Nexus Engine 0.94.0 — WebAssembly Scalar Memory & Comparisons

- Added the complete MVP family of scalar integer load/store instructions.
- Added signed and unsigned 8/16/32-bit load extension.
- Added truncating 8/16/32-bit stores and full-width i64 memory access.
- Added signed i32/i64 comparisons, equality, zero tests and typed `select`.
- Centralized effective-address, overflow and bounds validation.
- Added seven scalar-memory and comparison regression tests.
- Updated the platform matrix and advanced roadmap.
- Android versionCode 94 / versionName 0.94.0.

## Nexus Engine 0.93.0 — WebAssembly Memory & Floating Point

- Added persistent WebAssembly instances with isolated linear memory.
- Added validated memory declarations with bounded minimum and maximum page counts.
- Added little-endian `i32.load` / `i32.store` with controlled out-of-bounds traps.
- Added `memory.size` and bounded `memory.grow` semantics.
- Added bit-preserving `f32` / `f64` constants and arithmetic.
- Added memory-limit, instance-isolation and floating-point regression coverage.
- Updated the platform matrix and advanced roadmap.
- Android versionCode 93 / versionName 0.93.0.

## Nexus Engine 0.92.0 — JavaScript Worker Realms

- Added one independent QuickJS context per native Worker thread.
- Added bounded memory, stack and execution-time limits.
- Added `postMessage`, `onmessage` and message event listeners.
- Added Promise job checkpoints after message delivery.
- Added Worker timer registration and explicit checkpoints.
- Added message, timer and microtask execution statistics.
- Added isolated startup errors and deterministic realm termination.
- Updated the advanced roadmap and added six regression tests.
- Android versionCode 92 / versionName 0.92.0.

## Nexus Engine 0.91.0 — Android Build & Release Gate

- Connected Android `onTrimMemory()` callbacks to the native Resource Governor.
- Added moderate/critical platform-memory-pressure classification.
- Added a fail-closed release audit for versions, source formats, JNI parity and WebView exclusion.
- Added a one-command Rust, NDK and debug-APK build script.
- Pinned the Rust toolchain to 1.88.0 across the project and CI.
- Corrected desktop and Android CI artifact names to the current release.
- Added APK integrity hashing and release audit publication.
- Added `RELEASE.md` with build, artifact and device-install gates.
- Android versionCode 91 / versionName 0.91.0.

## Nexus Engine 0.90.0 — Shared Memory, Atomics & Workers

- Added cross-origin-isolated SharedArrayBuffer allocation and bounds checks.
- Added sequentially consistent i32 atomic load/store and read-modify-write operations.
- Added compare-exchange plus per-cell Atomics wait/notify and timeouts.
- Added structured clone primitives, byte arrays, collections and shared buffers.
- Added bounded native Worker queues, response events and backpressure.
- Added zero-copy shared-buffer identity across Worker messages.
- Updated the advanced roadmap and added seven regression tests.
- Android versionCode 90 / versionName 0.90.0.

## Nexus Engine 0.89.0 — WebAssembly Execution Runtime

- Added WebAssembly binary magic/version and ordered-section validation.
- Added bounded signed/unsigned LEB128 decoding.
- Added type, function, export and code section parsing.
- Added typed exported-function invocation, locals and internal calls.
- Added wrapping i32/i64 arithmetic and signed division.
- Added controlled stack, type, division, overflow and recursion traps.
- Added fail-closed rejection for unsupported sections and instructions.
- Promoted WebAssembly capability from Foundation to Partial.
- Updated the advanced roadmap and added seven regression tests.
- Android versionCode 89 / versionName 0.89.0.

## Nexus Engine 0.88.0 — WebRTC & MediaStream Control Plane

- Added SDP offer/answer, BUNDLE, RTP codec and security-attribute parsing.
- Added ICE candidate parsing and priority-based candidate-pair selection.
- Added signaling and ICE connection state machines.
- Added MediaStream audio/video track constraints and lifecycle.
- Added bounded DataChannel queues with explicit backpressure.
- Added deterministic peer and channel shutdown behavior.
- Promoted WebRTC capability from Foundation to Partial.
- Updated the advanced roadmap and added seven regression tests.
- Android versionCode 88 / versionName 0.88.0.

## Nexus Engine 0.87.0 — Media Playback Runtime

- Added MP4, WebM, Ogg and ADTS-AAC container discovery.
- Added AV1, VP9, H.264, Opus and AAC codec-string parsing.
- Added resolution/secure-stream-aware decoder capability selection.
- Added Android MediaCodec, VideoToolbox, WGPU Video and software adapter boundaries.
- Added encoded-packet and timestamp-ordered decoded-frame queues.
- Added play, pause, seek and playback-rate clock semantics.
- Added late-video-frame dropping, buffering and end-of-stream transitions.
- Promoted Media Playback capability from Foundation to Partial.
- Updated the advanced roadmap and added seven regression tests.
- Android versionCode 87 / versionName 0.87.0.

## Nexus Engine 0.86.0 — WebGL 2 & WebGPU Command Runtime

- Added validated WebGL 2 and WebGPU device/resource models.
- Added GLSL ES 3.00 and WGSL shader-stage/API validation.
- Added WebGPU compute pipelines and bounded workgroup dispatch.
- Added GPU buffers, usage validation, writes and texture descriptors.
- Added render-pass command encoding and submission reports.
- Added Vulkan/Metal/WGPU/OpenGL ES adapter targets plus an executable software adapter.
- Added frame pacing through 120 Hz with missed-frame accounting.
- Promoted WebGL and WebGPU capability levels from Foundation to Partial.
- Added `ADVANCED_PLATFORM_ROADMAP.md`, `WEB_PLATFORM_086.md` and six regression tests.
- Android versionCode 86 / versionName 0.86.0.

## Nexus Engine 0.85.0 — Process Broker & Sandbox IPC

- Added renderer execution ownership with stable process and frame identifiers.
- Added site-key grouping and frame-to-renderer routing.
- Added bounded IPC for commit, paint, input, capability and lifecycle commands.
- Added deny-by-default renderer network/filesystem policy enforcement.
- Added per-renderer retained-document memory budgets.
- Added isolated renderer crash and exit reporting plus deterministic cleanup.
- Promoted Process Sandbox capability from Foundation to Partial.
- Added `WEB_PLATFORM_085.md` and five regression tests.
- Android versionCode 85 / versionName 0.85.0.

## Nexus Engine 0.84.0 — Resource Governor & Real Tab Discarding

- Added moderate and critical memory-pressure handling.
- Added LRU inactive-tab selection with active, pinned and audible protection.
- Discarded tabs now release loaded pages, decoded resources, QuickJS realms and event loops.
- Preserved URL, title, history position, scroll and zoom across discard/restore.
- Added automatic reload and visual-state restoration when a discarded tab is activated.
- Added released-memory estimates to resource-governor reports.
- Added `WEB_PLATFORM_084.md` and four regression tests.
- Android versionCode 84 / versionName 0.84.0.

## Nexus Engine 0.83.0 — Modern Observer APIs

- Connected MutationObserver records to concrete Nexus DOM mutations.
- Added attribute filters, subtree matching and optional old values.
- Added independent observer queues and disconnect semantics.
- Added IntersectionObserver ratios and normalized threshold crossings.
- Added scroll-aware enter/leave records.
- Added deduplicated ResizeObserver geometry checkpoints.
- Added `WEB_PLATFORM_083.md` and five regression tests.
- Android versionCode 83 / versionName 0.83.0.

## Nexus Engine 0.82.0 — Web Components Runtime

- Added DOM candidate upgrades for valid Custom Element definitions.
- Added ordered upgrade/connected/disconnected/attribute-changed reactions.
- Added observed-attribute filtering with persistent upgraded-element state.
- Added open/closed Shadow Roots with one-root-per-host enforcement.
- Added inert HTML template parsing and cloning.
- Added named and default slot assignment for light-DOM children.
- Promoted Web Components capability from foundation to partial.
- Added `WEB_PLATFORM_082.md` and six regression tests.
- Android versionCode 82 / versionName 0.82.0.

## Nexus Engine 0.81.0 — Service Worker & Offline PWA

- Added quota-controlled named Cache Storage.
- Added canonical URL matching, ignore-search lookup and accurate deletion accounting.
- Added secure/same-origin Service Worker registration and activation.
- Added longest-scope controller selection.
- Added cache-first, network-first and stale-while-revalidate routing.
- Added PWA Web App Manifest parsing and defaults.
- Promoted Service Worker and Streaming Fetch capabilities from foundation to partial.
- Added `WEB_PLATFORM_081.md` and seven regression tests.
- Android versionCode 81 / versionName 0.81.0.

## Nexus Engine 0.80.0 — Web Platform Foundations

- Added a capability registry that distinguishes planned, foundation, partial and available support.
- Added real native dedicated-worker threads with message/event channels.
- Added Service Worker lifecycle and scope descriptors.
- Added Custom Element validation, Shadow Root and HTML Template boundaries.
- Added ordered MutationObserver, IntersectionObserver and ResizeObserver record queues.
- Added incremental Fetch streaming-body chunks.
- Added media codec, WebRTC and WebGL/WebGPU context descriptors.
- Added multi-process roles, renderer sandbox policy and certificate metadata.
- Added WebExtensions manifest registry.
- Added explicit tab discarding and memory-pressure selection.
- Added `WEB_PLATFORM_080.md` and eight new regression tests.
- Android versionCode 80 / versionName 0.80.0.

## Nexus Engine 0.71.0 — Modern CSS Colors

- Added legacy and modern `rgb()` / `rgba()` parsing.
- Added `hsl()` / `hsla()` conversion into renderer-owned RGBA colors.
- Added numeric/percentage alpha and slash-separated forms.
- Added four- and eight-digit hexadecimal alpha colors.
- Expanded the named-color baseline.
- Added cascade-aware `currentColor` for foregrounds, backgrounds, borders and pseudo-elements.
- Extended `@supports` color recognition.
- Added `examples/colors_071.html` and five regression tests.
- Android versionCode 71 / versionName 0.71.0.

## Nexus Engine 0.70.0 — Modern Web Compatibility

- Opened the 0.70 modern-web compatibility milestone without claiming full browser compatibility.
- Added `object-fit` fill, contain, cover, none and scale-down.
- Added centered, keyword and percentage `object-position` geometry.
- Replaced content now preserves intrinsic aspect ratio and clips cover overflow.
- Added inherited `visibility` to the paint pipeline.
- Added inherited `pointer-events` to visual hit-testing.
- Added `examples/modern_compat_070.html` and five regression tests.
- Android versionCode 70 / versionName 0.70.0.

## Nexus Engine 0.26.0 — Text Flow & Whitespace

- Added `white-space` normal, nowrap, pre and pre-wrap modes.
- Added inherited `text-transform` with uppercase, lowercase and capitalize behavior.
- Added inherited `text-indent` and first-line paint geometry.
- Extended the text engine boundary with collapse/wrap options while preserving backward compatibility.
- Explicit preformatted newlines now reserve layout height.
- Added `examples/text_flow_026.html` and four regression tests.
- Android versionCode 26 / versionName 0.26.0.

## Nexus Engine 0.25.0 — CSS Typography

- Added `font-weight`, `font-style`, `line-height` and `text-align` computed values.
- Added `text-decoration` / `text-decoration-line` with underline, overline and line-through paint geometry.
- Added a compact `font` shorthand covering style, weight, size, line height and family.
- Propagates typography through inherited styles, text layout and the renderer-neutral Display List.
- Skia now requests normal, bold, italic or bold-italic typeface variants.
- Added browser-like UA defaults for headings, `strong`/`b` and `em`/`i`.
- Added `examples/typography_025.html` and three regression tests.
- Android versionCode 25 / versionName 0.25.0.

## Nexus Engine 0.24.0 — Named Grid Areas

- Added `grid-template-areas` and named `grid-area` placement.
- Added quoted-row parsing, equal-column validation and rectangular-area validation.
- Added automatic explicit `auto` tracks when an area template omits track lists.
- Resolves named areas into numbered row/column lines before Taffy layout.
- Preserves declaration-order overrides between named and numbered placement.
- Extended `@supports` checks for named-area syntax.
- Added `examples/grid_areas_024.html` and five regression tests.
- Android versionCode 24 / versionName 0.24.0.

## Nexus Engine 0.23.0 — Adaptive Flexbox & Sizing

- Added aspect ratios, box sizing, Flexbox wrapping/basis/shorthands and stable visual order.
- Added four sizing and Flexbox regression tests.
- Android versionCode 23 / versionName 0.23.0.

## Nexus Engine 0.22.0 — Grid Auto-Repeat & Box Alignment

- Added dynamic `auto-fill`/`auto-fit`, container/item alignment and the `place-*` shorthands.
- Connected the new values to Taffy 0.13 and added four regression tests.
- Android versionCode 22 / versionName 0.22.0.

## Nexus Engine 0.21.0 — Advanced CSS Grid

- Added `minmax()`, `fit-content()`, implicit tracks and numbered/`span` item placement.
- Connected the advanced Grid values to Taffy and added four regression tests.
- Android versionCode 21 / versionName 0.21.0.

## Nexus Engine 0.20.0 — CSS Grid Layout

- Added typed explicit Grid tracks, bounded `repeat()`, auto-flow and independent gaps.
- Connected px, percentage, `fr`, intrinsic and automatic tracks to Taffy 0.13.
- Added Grid-aware `@supports`, a demonstration and geometry regression coverage.
- Android versionCode 20 / versionName 0.20.0.

## Nexus Engine 0.19.0 — Feature Queries & CSS Math

- Added `@supports (property: value)` evaluation against Nexus CSS capabilities.
- Added `@supports selector(...)` queries.
- Added boolean `not`, `and`, `or` and nested parentheses for support conditions.
- Preserved inherited media-query constraints inside supported blocks.
- Added responsive `min()`, `max()` and `clamp()` length functions.
- Added nested math-function evaluation with viewport/font-relative units.
- Extended `calc()` with subtraction, multiplication, division and finite-result checks.
- Added `examples/supports_math_019.html` and three dedicated regression tests.
- Android versionCode 19 / versionName 0.19.0.

## Nexus Engine 0.18.0 — Selector Matching & Global Values

- Added attribute token (`~=`), dash (`|=`), prefix (`^=`), suffix (`$=`) and substring (`*=`) operators.
- Added the ASCII case-insensitive attribute selector flag (`i`).
- Added `:first-of-type`, `:last-of-type`, `:only-of-type`, `:nth-of-type()` and `:nth-last-of-type()`.
- Added reverse structural matching through `:nth-last-child()`.
- Added global `inherit`, `initial` and `unset` handling for computed properties.
- Added global-value behavior for inherited custom properties.
- Added `examples/selectors_values_018.html` and three dedicated regression tests.
- Android versionCode 18 / versionName 0.18.0.

## Nexus Engine 0.17.0 — Advanced Selectors & Cascade

- Added adjacent (`+`) and general (`~`) sibling combinators.
- Added functional `:not()`, `:is()` and zero-specificity `:where()` selector lists.
- Added `:only-child`, standards-aligned `:empty`, `:disabled`, `:checked`, `:link` and `:any-link`.
- Added functional pseudo-class specificity calculation.
- Replaced the previous discarded `!important` marker with normal/important cascade passes.
- Added importance-aware custom-property cascading and variable resolution for generated content.
- Fixed selector-list splitting so commas inside functional pseudo-classes remain nested.
- Added `examples/cascade_selectors_017.html` and three dedicated regression tests.
- Android versionCode 17 / versionName 0.17.0.

## Nexus Engine 0.16.0 — CSS Compatibility Layer

- Added descendant and direct-child selector combinators.
- Added attribute presence and exact-value selectors.
- Added `:root`, `:first-child`, `:last-child`, and `:nth-child(an+b)` with odd/even forms.
- Added specificity accumulation across complex selectors.
- Added inherited CSS custom properties and `var()` fallback resolution with a bounded recursion limit.
- Added initial `calc()` length addition/subtraction.
- Added `rem`, `em`, `vw`, `vh`, `vmin`, and `vmax` resolution against the active viewport.
- Added `examples/css_compat_016.html` and dedicated compatibility regression tests.
- Retained the 0.15 responsive, positioned-layout, compositor and visual hit-test pipeline.
- Android versionCode 16 / versionName 0.16.0.

## Nexus Engine 0.15.0 — Advanced CSS & Responsive Layout

- Added viewport-aware `StyleEngine::compute_for_viewport`.
- Added `MediaEnvironment` and responsive `@media` evaluation for min/max width/height and orientation.
- Added `CssPosition`: static, relative, absolute, fixed and sticky.
- Added CSS inset properties (`top/right/bottom/left`).
- Mapped relative/absolute positioning and inset geometry into Taffy 0.13.
- Added `CssOverflow` and Taffy overflow integration.
- Added ancestor overflow clipping in Nexus Display Lists.
- Added `z-index` and new renderer-neutral `compositing.rs` paint-order/stacking metadata.
- Added initial fixed/sticky viewport behavior during scroll.
- Added `CssTransform` with translate/translateX/translateY and scale/scaleX/scaleY.
- Added opacity for painted text/background/borders.
- Added `::before` / `::after` quoted generated content.
- Unified advanced visual geometry with hit-testing so fixed/transformed/z-indexed targets remain interactive where they are painted.
- Updated long-press selection geometry to follow advanced painted positions.
- Added `examples/responsive_015.html`.
- Added advanced CSS, compositor, fixed/sticky, overflow, transform, generated-content and hit-test coverage.
- Android versionCode 15 / versionName 0.15.0.

## Nexus Engine 0.14.0 — Alpha Browser Shell

- Added trusted `nexus://` internal pages, profile settings, privacy dashboard, granular data clearing and rendered error pages.

## Nexus Engine 0.13.0 — Mobile Forms & Input Alpha

- Added mobile form descriptors, checkbox/radio/select state, file picker integration, multipart POST and basic constraint validation.
