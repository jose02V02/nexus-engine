# Nexus Engine 1.02 — WebAssembly Bulk Table Operations

Nexus 1.02 adds passive element lifecycles and atomic bulk operations to the bounded `funcref` table runtime.

## Pipeline

`URL -> Reqwest/Rustls -> html5ever -> Nexus DOM -> QuickJS-ng -> Nexus CSS -> Taffy -> Nexus Compositor -> Display List -> Skia -> Android`

The Android shell remains browser chrome/input integration only. Page content is rendered by the Rust Nexus engine; the app source contains no `android.webkit.WebView` dependency.

## New in 1.02

### Passive elements and bulk tables

- passive MVP function-index element segments
- per-instance passive segment lifecycle
- `table.init` with source and destination range validation
- idempotent `elem.drop`
- overlap-safe `table.copy`
- bounded `table.fill` with validated function references
- bounds failures occur before mutation

## Retained from 1.01

### Imported and mutable tables

- explicit `funcref` table imports with module/name registration
- exact minimum/maximum validation under the 4,096-slot policy
- synchronized table state shared across linked instances
- `ref.null`, `ref.func`, `table.get` and `table.set`
- bounded `table.size` and `table.grow`
- indirect calls continue to enforce initialization, bounds and exact signatures

### Linking guarantees

- missing or mismatched table capabilities stop instantiation
- a module cannot define and import a table simultaneously
- duplicate table registrations are rejected without replacement
- injected function references are validated against the module function index space
- poisoned synchronization state becomes a controlled runtime trap

## Retained from 1.00

### Imported memory capabilities

- MVP memory-import parsing with explicit `module` / `name` registration
- exact minimum/maximum limit validation under the 256-page engine policy
- synchronized byte storage shared across linked instances and the host
- shared `memory.size`, `memory.grow`, loads and stores
- bounded host read/write helpers with no ambient DOM, network, filesystem or Android access

### Imported-memory guarantees

- missing or mismatched memory capabilities stop instantiation
- a module cannot define and import linear memory simultaneously
- duplicate memory registrations are rejected without replacement
- growth is visible to every linked instance and stops at the declared maximum
- poisoned synchronization state becomes a controlled runtime trap

## Retained from 0.99

Capability-scoped typed global imports and synchronized shared global state remain available.

### Typed globals

- i32, i64, f32 and f64 global declarations
- constant initializers with exact type matching
- mutable and immutable global flags
- maximum 1,024 globals per module
- bit-preserving IEEE-754 initial values

### Instance state

- `global.get` and `global.set`
- state persists between calls on the same instance
- separate instances never share defined-global mutations
- writes enforce index, mutability and value type
- direct QuickJS global bindings remain future work

See `WEB_PLATFORM_102.md` and `ADVANCED_PLATFORM_ROADMAP.md`.

## Retained from 0.97

Bounded function tables, active element segments and type-safe indirect calls remain available.

## Retained from 0.96

Capability-scoped function imports and unified function indices remain available.

## Retained from 0.95

Nested Wasm control flow, depth-aware branches and the per-call execution budget remain available.

## Retained from 0.94

Scalar integer memory operations, comparisons and typed selection remain available.

## Retained from 0.93

Persistent linear memory, memory growth and IEEE-754 arithmetic remain available.

## Retained from 0.92

Independent QuickJS Worker realms, message events, Promise/timer checkpoints and termination remain available.

## Retained from 0.91

The audited Rust/NDK/APK release gate and Android memory-pressure integration remain available.

## Retained from 0.90

SharedArrayBuffer, sequentially consistent Atomics, wait/notify and bounded structured Worker messaging remain available.

## Retained from 0.89

The validated WebAssembly integer loader/interpreter, exports, calls and controlled traps remain available.

## Retained from 0.88

SDP/ICE validation, peer state, MediaStream tracks and DataChannel flow control remain available.

## Retained from 0.87

Container/codec discovery, decoder selection, media queues, playback clocks and A/V presentation remain available.

## Retained from 0.86

The WebGL 2/WebGPU resource and command runtime, compute dispatch validation and 120 Hz frame scheduler remain available.

## Retained from 0.85

The renderer broker, bounded IPC, sandbox policy, memory budgets and isolated crash reporting remain available.

## Retained from 0.84

The Resource Governor still releases inactive page resources under memory pressure and restores URL, history, scroll and zoom on activation while protecting active, pinned and audible tabs.

## Retained from 0.83

MutationObserver filtering/subtree/old-value records and scroll-aware IntersectionObserver/ResizeObserver checkpoints remain available.

## Retained from 0.82

### Component model

- Custom Element definitions require valid hyphenated names
- existing DOM candidates are upgraded in document order
- ordered upgrade, connected, disconnected and attribute-changed reactions
- observed-attribute filtering
- one Shadow Root per valid element host
- open and closed Shadow Root visibility
- inert HTML template parsing and independent cloning
- named/default light-DOM slot assignment

The complete 0.82 Web Components runtime remains available.

## Retained from 0.81

### Offline PWA layer

- named Cache Storage containers with byte quotas
- exact and ignore-search request matching
- atomic cache replacement and deletion accounting
- secure-origin and same-origin Service Worker registration checks
- activated-controller selection using the longest matching scope
- cache-first, network-first and stale-while-revalidate route decisions
- PWA manifest parsing with installation defaults
- partial Service Worker and Streaming Fetch status promoted in the capability registry

The complete 0.81 offline PWA layer remains available.

## Retained from 0.80

### Executable foundations

- native dedicated-worker threads with bidirectional message channels
- ordered Mutation/Intersection/Resize observer records
- incremental streaming-body chunks and byte accounting
- active, suspended, frozen and discarded tab lifecycle states
- memory-pressure discarding that protects the active tab

### Stable integration boundaries

- WebAssembly module lifecycle
- Service Worker registration and activation
- Custom Elements, Shadow Roots and HTML template fragments
- audio/video codec and media pipeline descriptors
- WebRTC connection lifecycle
- WebGL/WebGPU backend selection
- browser/renderer/network/GPU process roles and sandbox policy
- certificate trust metadata
- WebExtensions manifests, permissions and content scripts

The full 0.80 platform contracts remain available.

## Retained from 0.71

### Modern colors

- legacy comma and modern space-separated `rgb()` / `rgba()`
- `hsl()` / `hsla()` with alpha slash syntax
- numeric and percentage alpha values
- `#RGBA` and `#RRGGBBAA`
- expanded named-color baseline
- cascade-aware `currentColor` for color, backgrounds and borders
- modern color values participate in `@supports`

## Retained from 0.70

### Replaced content and interaction

- `object-fit: fill | contain | cover | none | scale-down`
- `object-position` keywords and percentage pairs
- aspect-ratio-preserving image destination geometry
- clipping for covered and oversized replaced content
- inherited `visibility: visible | hidden | collapse`
- inherited `pointer-events: auto | none`
- paint and hit-testing consult the same visibility/interaction policy

## Retained from 0.26

### Text flow

- `white-space: normal | nowrap | pre | pre-wrap`
- `text-transform: none | uppercase | lowercase | capitalize`
- `text-indent` with resolved CSS lengths
- preserved explicit line breaks and spaces through the Parley boundary
- optional line breaking for nowrap/pre content
- first-line indentation included in alignment and decoration geometry

## Retained from 0.25

### Typography pipeline

- `font-weight: 1..1000 | normal | bold | bolder | lighter`
- `font-style: normal | italic | oblique`
- `line-height: normal | <number> | <percentage> | <px>`
- `text-align: start | end | left | right | center | justify`
- `text-decoration` and `text-decoration-line` with underline, overline and line-through
- compact `font` shorthand with optional style/weight and size/line-height
- inherited font metrics drive text-leaf layout height
- alignment and decoration geometry are emitted into the Nexus Display List
- Skia typeface matching receives weight and italic intent

## Retained from 0.24

### Semantic Grid layouts

- `grid-template-areas` with quoted rows
- `grid-area` named placement
- empty cells using dot tokens
- equal-column and rectangular-region validation
- automatic tracks when explicit sizes are omitted
- responsive area templates inside existing media queries

## Retained from 0.23

### Modern sizing

- `aspect-ratio` with number and ratio syntax
- `box-sizing: content-box | border-box`
- author ratios override intrinsic image ratios

### Adaptive Flexbox

- `flex-wrap` and `flex-basis`
- `flex-flow` and `flex` shorthands
- stable positive and negative `order`
- responsive wrapping with existing gaps and alignment

## Retained from 0.22

### Dynamic responsive tracks

- `repeat(auto-fill, ...)`
- `repeat(auto-fit, ...)`
- auto-repeat patterns remain dynamic until Taffy computes the available space

### Box alignment

- `align-items`, `justify-items`, `align-self`, `justify-self`
- `align-content`, `justify-content`
- `place-items`, `place-self`, `place-content`
- start/end, flex-start/flex-end, center, baseline and stretch
- space-between, space-around and space-evenly

## Retained from 0.21

### Advanced track sizing

- `minmax(<minimum>, <maximum>)`
- `fit-content(<length-percentage>)`
- `grid-auto-columns` and `grid-auto-rows`
- implicit tracks map directly into Taffy Grid sizing

### Grid item placement

- `grid-column` and `grid-row`
- `grid-column-start` / `grid-column-end`
- `grid-row-start` / `grid-row-end`
- positive and negative numbered lines
- positive `span` values

### Explicit grid tracks

- `grid-template-columns`
- `grid-template-rows`
- fixed pixel and percentage tracks
- fractional `fr` tracks
- `auto`, `min-content`, and `max-content`
- integer `repeat()` expansion with bounded track counts

### Grid flow and spacing

- `grid-auto-flow: row`
- `grid-auto-flow: column`
- dense row/column packing
- independent `row-gap` and `column-gap`
- `gap` initializes both axes while axis-specific declarations can override it

### Taffy integration

- Nexus computed styles retain typed grid-track lists
- typed tracks map to Taffy 0.13 Grid template components
- explicit rows/columns participate in real layout geometry
- responsive `@media`, `@supports`, variables and CSS math compose with Grid declarations

### Responsive layout retained

- `overflow`, `overflow-x`, `overflow-y`
- `visible`, `hidden`, `clip`, `scroll`, `auto`
- Taffy receives overflow semantics for layout
- Nexus Display List emits ancestor clipping for painted descendants
- hit-testing checks the same overflow clips as rendering

### Stacking/compositing retained

New `compositing.rs` separates paint order from layout.

- `z-index` parsing
- stable negative/normal/positive z-index paint phases
- stacking-context metadata for positioned z-index, fixed/sticky, opacity and transforms
- topmost hit-testing follows compositor paint order

This is a foundation, not yet a full CSS stacking-context implementation.

### Transform and opacity foundation

- `translate(x,y)`
- `translateX()` / `translateY()`
- `scale()`
- `scaleX()` / `scaleY()`
- `opacity`
- transformed paint geometry is shared with hit-testing and selection

`rotate`, `skew`, `matrix`, transform-origin and GPU layer promotion are intentionally deferred.

### Generated content

- `::before` / `:before`
- `::after` / `:after`
- quoted `content:` strings
- pseudo-element color/background/font-size

Generated content is currently paint-only and does not create independent layout boxes.

## Existing Alpha features retained

Nexus 0.25 keeps the previous networking, DOM, JavaScript, forms, Web Storage, cookies, cache, CORS/CSP/HSTS, WebSockets, multi-tab, private browsing, bookmarks, downloads, `nexus://` internal pages, mobile gestures, zoom and Android shell.

## Try the CSS layer

Demo documents include the earlier CSS demos plus `examples/grid_020.html` and `examples/grid_021.html`.

The CLI can inspect a live page with different viewports:

```bash
cargo run --release --bin nexus -- https://example.com --viewport 390x844 --styles --layout --render mobile.png
cargo run --release --bin nexus -- https://example.com --viewport 1280x720 --styles --layout --render desktop.png
```

## Build

```bash
cargo test --all-targets
cargo build --release
```

Android is built by the included GitHub Actions workflow with JDK 17, Android SDK/NDK and `cargo-ndk`.

## Important Alpha limitations

0.21 does **not** claim complete Grid compatibility. Named lines/areas, `subgrid`, auto-fill/auto-fit and advanced alignment remain future milestones.

The objective of 0.21 is to expand real-world Grid coverage while retaining clean DOM, layout and renderer boundaries.
