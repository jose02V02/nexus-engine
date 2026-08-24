# Third-party components — Nexus Engine 0.25

Nexus remains an integration project. Review upstream licenses before redistribution.

| Component | Nexus role | Upstream license family |
|---|---|---|
| `url` / Servo rust-url ecosystem | URL parsing/resolution | MIT / Apache-2.0 |
| `reqwest` + `rustls` | HTTP/TLS transport | MIT / Apache-2.0 / ISC ecosystem |
| `encoding_rs` | Web text decoding | MIT / Apache-2.0 |
| `html5ever` + `markup5ever_rcdom` | HTML parsing | MIT / Apache-2.0 |
| `cssparser` 0.37 | CSS Syntax token/parser primitives | MPL-2.0 |
| `taffy` 0.13 | Block/Flex/Grid + position/overflow layout | MIT |
| `parley` | text layout | Apache-2.0 / MIT ecosystem |
| `skia-safe` / Skia | raster rendering | BSD-style upstream |
| `image` | image decoding | MIT / Apache-2.0 |
| QuickJS-ng via `quickjs-rusty` | JavaScript runtime | MIT ecosystem |
| Tokio / tokio-tungstenite / tungstenite | async event loop + WebSocket | MIT ecosystem |
| reqwest_cookie_store | cookie integration | MIT / Apache-2.0 ecosystem |

This file is a convenience summary, not legal advice. The exact license texts and transitive dependency obligations should be audited before public distribution of Nexus binaries.
