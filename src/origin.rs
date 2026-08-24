//! Web origin model used by Nexus Engine 1.02.
//!
//! Nexus keeps origin comparisons explicit instead of comparing URL strings.

use std::fmt::{Display, Formatter};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Origin {
    Tuple {
        scheme: String,
        host: String,
        port: u16,
    },
    Opaque,
}

impl Origin {
    #[must_use]
    pub fn from_url(url: &Url) -> Self {
        let scheme = url.scheme().to_ascii_lowercase();
        if !matches!(scheme.as_str(), "http" | "https" | "ws" | "wss") {
            return Self::Opaque;
        }
        let Some(host) = url.host_str() else {
            return Self::Opaque;
        };
        let port = url.port_or_known_default().unwrap_or(0);
        if port == 0 {
            return Self::Opaque;
        }
        Self::Tuple {
            scheme,
            host: host.to_ascii_lowercase(),
            port,
        }
    }

    #[must_use]
    pub fn is_same_origin(&self, other: &Self) -> bool {
        self == other && !matches!(self, Self::Opaque)
    }

    #[must_use]
    pub fn is_secure(&self) -> bool {
        matches!(self, Self::Tuple { scheme, .. } if matches!(scheme.as_str(), "https" | "wss"))
    }

    #[must_use]
    pub fn serialize(&self) -> String {
        match self {
            Self::Tuple { scheme, host, port } => {
                let default = matches!((scheme.as_str(), *port), ("http", 80) | ("https", 443) | ("ws", 80) | ("wss", 443));
                if default {
                    format!("{scheme}://{host}")
                } else {
                    format!("{scheme}://{host}:{port}")
                }
            }
            Self::Opaque => "null".to_owned(),
        }
    }
}

impl Display for Origin {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.serialize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ports_compare_as_same_origin() {
        let a = Url::parse("https://example.com/a").unwrap();
        let b = Url::parse("https://example.com:443/b").unwrap();
        assert!(Origin::from_url(&a).is_same_origin(&Origin::from_url(&b)));
    }

    #[test]
    fn different_ports_are_cross_origin() {
        let a = Url::parse("https://example.com/").unwrap();
        let b = Url::parse("https://example.com:444/").unwrap();
        assert!(!Origin::from_url(&a).is_same_origin(&Origin::from_url(&b)));
    }
}
