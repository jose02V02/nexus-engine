//! WebSocket URL/origin helpers for Nexus Engine 1.02.
//!
//! This module validates browser-facing WebSocket construction. Live socket
//! I/O is owned by `event_loop.rs`, which uses Tokio + tokio-tungstenite while
//! QuickJS remains confined to the BrowserSession thread.

use url::Url;

use crate::origin::Origin;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSocketRequest {
    pub url: Url,
    pub origin: Origin,
    pub protocols: Vec<String>,
}

impl WebSocketRequest {
    pub fn new(page_url: &Url, input: &str, protocols: Vec<String>) -> Result<Self, String> {
        let mut url = page_url.join(input).or_else(|_| Url::parse(input)).map_err(|err| err.to_string())?;
        match url.scheme() {
            "http" => {
                url.set_scheme("ws").map_err(|_| "cannot convert http URL to ws".to_owned())?;
            }
            "https" => {
                url.set_scheme("wss").map_err(|_| "cannot convert https URL to wss".to_owned())?;
            }
            "ws" | "wss" => {}
            other => return Err(format!("unsupported WebSocket scheme: {other}")),
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err("WebSocket URLs with embedded credentials are rejected".to_owned());
        }
        let origin = Origin::from_url(page_url);
        Ok(Self {
            url,
            origin,
            protocols: normalize_protocols(protocols)?,
        })
    }

    #[must_use]
    pub fn origin_header(&self) -> String {
        self.origin.serialize()
    }
}

fn normalize_protocols(protocols: Vec<String>) -> Result<Vec<String>, String> {
    let mut output = Vec::new();
    for protocol in protocols {
        let value = protocol.trim();
        if value.is_empty() || value.bytes().any(|byte| byte <= 0x20 || byte >= 0x7f || matches!(byte, b'(' | b')' | b'<' | b'>' | b'@' | b',' | b';' | b':' | b'\\' | b'"' | b'/' | b'[' | b']' | b'?' | b'=' | b'{' | b'}')) {
            return Err(format!("invalid WebSocket subprotocol: {protocol}"));
        }
        if output.iter().any(|existing| existing == value) {
            return Err(format!("duplicate WebSocket subprotocol: {value}"));
        }
        output.push(value.to_owned());
    }
    Ok(output)
}

/// Uses tungstenite's RFC6455 handshake primitive for deterministic tests and
/// protocol validation shared with the live tokio-tungstenite path.
#[must_use]
pub fn expected_accept_key(sec_websocket_key: &str) -> String {
    tungstenite::handshake::derive_accept_key(sec_websocket_key.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_accept_uses_tungstenite() {
        assert_eq!(
            expected_accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn https_relative_url_becomes_wss() {
        let page = Url::parse("https://example.com/app").unwrap();
        let request = WebSocketRequest::new(&page, "/socket", vec!["chat".to_owned()]).unwrap();
        assert_eq!(request.url.as_str(), "wss://example.com/socket");
        assert_eq!(request.origin_header(), "https://example.com");
    }
}
