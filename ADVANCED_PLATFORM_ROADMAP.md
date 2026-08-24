# Nexus Advanced Platform Roadmap

Capabilities are promoted only when an executable backend and integration tests
exist; interface declarations alone do not count as implementation.

| Workstream | Current executable baseline | Completion gate |
| --- | --- | --- |
| WebGL 2 / WebGPU compute | 0.86 validated resources, shaders, commands, compute dispatch and software adapter | Android Vulkan/WGPU execution, GLSL/WGSL conformance and canvas integration |
| GPU compositing at high refresh | 0.86 frame scheduler supports targets through 120 Hz | Hardware surfaces, tiled raster and device measurements sustaining target refresh |
| Audio/video codecs | 0.87 container/codec discovery, decoder selection, queues, A/V clock, seek and presentation | MediaExtractor/MediaCodec decode, audio sink, video surface and DOM controls/events |
| WebRTC / MediaStream | 0.88 SDP/RTP parsing, ICE candidates, peer states, MediaStream tracks and DataChannel queues | STUN/TURN sockets, ICE checks, DTLS-SRTP, SCTP transport and Android capture |
| JavaScript JIT | Persistent QuickJS runtime | Maintained tiered-JIT integration, GC, deoptimization and security hardening |
| WebAssembly | 1.02 passive elements, bulk/shared funcref tables, imported memory/globals, indirect calls, capability imports and persistent memory | Declarative reference-expression elements, multi-value, SIMD, threads, JS bindings and conformance suites |
| Workers / shared memory | 0.92 independent QuickJS realms, bounded message events, Promise/timer checkpoints, SharedArrayBuffer and i32 Atomics | Page Worker constructor, script fetching, transferables, full typed-array bindings and agent-cluster integration |
| Site Isolation | 0.85 broker, frame routing, bounded IPC and crash boundary | Android isolated renderer services and cross-origin frame proxies |
| Kernel sandbox | Deny-by-default portable renderer policy | seccomp-bpf/AppContainer, brokered syscalls and exploit tests |
| WebExtensions | Manifest/permission/content-script registry | Isolated worlds, runtime/tabs/storage/webRequest APIs and real extension packaging |
| Web Platform Tests | Project regression suite | Automated WPT runner, reproducible expectations and published pass-rate dashboard |
| Mature browser shell | Tabs, history, private state, downloads, bookmarks and TLS metadata | Encrypted sync, password vault, certificate UI and process-backed incognito |

Hardware acceleration, sustained 120 FPS, kernel isolation and codec support
must be verified on each supported platform and device class.
