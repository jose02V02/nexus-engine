# Nexus 1.02 Web Platform Matrix

This matrix is intentionally conservative. `Foundation` means that Nexus owns
typed lifecycle/integration boundaries, not that the corresponding browser
standard is complete.

| Capability | 0.80 level | Implemented now | Backend still required |
| --- | --- | --- | --- |
| WebAssembly | Partial | Passive function elements; bulk/shared funcref tables; imported memory/globals; call_indirect, function imports, nested control flow and numeric operations | Reference-expression elements, multi-value, SIMD, threads, direct JS bindings and optimizing compiler |
| Dedicated Workers | Partial | Independent QuickJS thread/realm, bounded message events, Promise/timer checkpoints, resource limits, shared memory and i32 Atomics | Page-level `Worker` constructor, script fetching, transferables and full structured clone bindings |
| Service Workers / PWA | Partial | Secure-origin registration, longest-scope control, Cache Storage, manifest and fetch strategies | JavaScript realm, automatic interception, persistence and update algorithm |
| Web Components | Partial | Custom-element upgrades/reactions, open/closed Shadow Roots, inert templates and slot assignment | JavaScript class callbacks, composed event path and style encapsulation |
| Observer APIs | Partial | DOM mutation filtering/subtree/old values, intersection thresholds and deduplicated resize checkpoints | Automatic microtask delivery and JavaScript callback bindings |
| Streaming Fetch | Partial | Incremental chunks, byte accounting and cache/network route decisions | Network backpressure, ReadableStream, interception and abort wiring |
| Audio/video | Partial | Container/codec discovery, decoder selection, packet/frame queues, playback clock, seek, buffering, EOS and A/V frame timing | Android MediaExtractor/MediaCodec decode, audio sink, video surface and DOM controls/events |
| WebRTC | Partial | SDP offer/answer and RTP codec parsing, ICE candidates/pair selection, signaling states, MediaStream tracks and DataChannel backpressure | STUN/TURN sockets, ICE checks, DTLS-SRTP, SCTP transport and Android capture |
| WebGL/WebGPU | Partial | API-specific shader validation, buffers, textures, render/compute pipelines, command submission and 120 Hz frame scheduler | Vulkan/WGPU hardware execution, canvas presentation and full conformance |
| Multi-process sandbox | Partial | Renderer broker, site/frame ownership, bounded IPC, memory budgets, policy denial, shutdown and crash reporting | Android isolated-service transport, cross-process serialization and OS-enforced seccomp isolation |
| Tab lifecycle | Available | Memory-pressure LRU selection, real in-process resource release, preserved navigation/visual state, activation restore, active/pinned/audible protection | OS renderer-process snapshots and termination after the multi-process broker exists |
| Browser shell | Partial | Multi-tab, history, TLS networking and certificate descriptor | Full certificate UI and platform trust exception workflow |
| WebExtensions | Foundation | Validated manifest registry and permissions/content-script model | Isolated worlds, API permissions and extension packaging |

The next engineering gate is to replace one foundation at a time with an
executable backend and promote its level only after integration tests pass.
