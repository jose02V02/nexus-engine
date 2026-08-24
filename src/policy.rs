//! Browser security policies for Nexus Engine 1.02.
//!
//! This module deliberately implements a conservative, testable subset of
//! browser policy: CSP source lists, Referrer-Policy, mixed-content blocking
//! and a persistent HSTS host store. Unsupported CSP syntax is ignored rather
//! than treated as permission to widen an explicit directive.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::origin::Origin;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferrerPolicy {
    NoReferrer,
    NoReferrerWhenDowngrade,
    Origin,
    OriginWhenCrossOrigin,
    SameOrigin,
    StrictOrigin,
    StrictOriginWhenCrossOrigin,
    UnsafeUrl,
}

impl Default for ReferrerPolicy {
    fn default() -> Self {
        Self::StrictOriginWhenCrossOrigin
    }
}

impl ReferrerPolicy {
    #[must_use]
    pub fn parse_header(value: Option<&str>) -> Self {
        let Some(value) = value else { return Self::default() };
        value
            .split(',')
            .filter_map(|token| match token.trim().to_ascii_lowercase().as_str() {
                "no-referrer" => Some(Self::NoReferrer),
                "no-referrer-when-downgrade" => Some(Self::NoReferrerWhenDowngrade),
                "origin" => Some(Self::Origin),
                "origin-when-cross-origin" => Some(Self::OriginWhenCrossOrigin),
                "same-origin" => Some(Self::SameOrigin),
                "strict-origin" => Some(Self::StrictOrigin),
                "strict-origin-when-cross-origin" => Some(Self::StrictOriginWhenCrossOrigin),
                "unsafe-url" => Some(Self::UnsafeUrl),
                _ => None,
            })
            .last()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn referer(self, source: &Url, target: &Url) -> Option<String> {
        if !matches!(source.scheme(), "http" | "https") || !matches!(target.scheme(), "http" | "https") {
            return None;
        }
        let same = Origin::from_url(source).is_same_origin(&Origin::from_url(target));
        let downgrade = source.scheme() == "https" && target.scheme() == "http";
        let full = sanitized_referrer(source);
        let origin = Origin::from_url(source).serialize();
        match self {
            Self::NoReferrer => None,
            Self::NoReferrerWhenDowngrade => (!downgrade).then_some(full),
            Self::Origin => Some(format!("{origin}/")),
            Self::OriginWhenCrossOrigin => Some(if same { full } else { format!("{origin}/") }),
            Self::SameOrigin => same.then_some(full),
            Self::StrictOrigin => (!downgrade).then_some(format!("{origin}/")),
            Self::StrictOriginWhenCrossOrigin => {
                if downgrade {
                    None
                } else if same {
                    Some(full)
                } else {
                    Some(format!("{origin}/"))
                }
            }
            Self::UnsafeUrl => Some(full),
        }
    }
}

fn sanitized_referrer(source: &Url) -> String {
    let mut url = source.clone();
    url.set_fragment(None);
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CspPolicy {
    directives: HashMap<String, Vec<String>>,
}

impl CspPolicy {
    #[must_use]
    pub fn parse(header: Option<&str>) -> Self {
        let Some(header) = header else { return Self::default() };
        let mut directives = HashMap::new();
        for raw in header.split(';') {
            let mut parts = raw.split_ascii_whitespace();
            let Some(name) = parts.next() else { continue };
            let name = name.to_ascii_lowercase();
            if name.is_empty() || directives.contains_key(&name) {
                continue;
            }
            directives.insert(name, parts.map(str::to_owned).collect());
        }
        Self { directives }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.directives.is_empty()
    }

    #[must_use]
    pub fn raw_directive(&self, name: &str) -> Option<&[String]> {
        self.directives.get(&name.to_ascii_lowercase()).map(Vec::as_slice)
    }

    #[must_use]
    pub fn allows_inline_script(&self) -> bool {
        let Some(sources) = self.sources_for("script-src") else { return true };
        sources.iter().any(|source| source.eq_ignore_ascii_case("'unsafe-inline'"))
    }

    #[must_use]
    pub fn allows_script_url(&self, page: &Url, target: &Url) -> bool {
        self.allows_url("script-src", page, target)
    }

    #[must_use]
    pub fn allows_connect_url(&self, page: &Url, target: &Url) -> bool {
        self.allows_url("connect-src", page, target)
    }

    #[must_use]
    pub fn allows_image_url(&self, page: &Url, target: &Url) -> bool {
        self.allows_url("img-src", page, target)
    }

    #[must_use]
    pub fn allows_url(&self, directive: &str, page: &Url, target: &Url) -> bool {
        let Some(sources) = self.sources_for(directive) else { return true };
        if sources.iter().any(|source| source.eq_ignore_ascii_case("'none'")) {
            return false;
        }
        sources.iter().any(|source| source_matches(source, page, target))
    }

    fn sources_for(&self, directive: &str) -> Option<&[String]> {
        self.directives
            .get(directive)
            .or_else(|| self.directives.get("default-src"))
            .map(Vec::as_slice)
    }
}

fn source_matches(source: &str, page: &Url, target: &Url) -> bool {
    let source = source.trim();
    if source == "*" {
        return matches!(target.scheme(), "http" | "https" | "ws" | "wss");
    }
    if source.eq_ignore_ascii_case("'self'") {
        let page_origin = Origin::from_url(page);
        let target_origin = Origin::from_url(target);
        if page_origin.is_same_origin(&target_origin) {
            return true;
        }
        return same_host_port_secure_transport_pair(page, target);
    }
    if source.ends_with(':') && !source.contains('/') {
        return target.scheme().eq_ignore_ascii_case(source.trim_end_matches(':'));
    }
    if let Some(host_suffix) = source.strip_prefix("*.") {
        return target.host_str().is_some_and(|host| {
            host.eq_ignore_ascii_case(host_suffix)
                || host.to_ascii_lowercase().ends_with(&format!(".{}", host_suffix.to_ascii_lowercase()))
        });
    }
    if let Ok(url) = Url::parse(source) {
        return Origin::from_url(&url).is_same_origin(&Origin::from_url(target));
    }
    false
}

fn same_host_port_secure_transport_pair(page: &Url, target: &Url) -> bool {
    let paired = matches!((page.scheme(), target.scheme()), ("https", "wss") | ("http", "ws"));
    paired
        && page.host_str().zip(target.host_str()).is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
        && page.port_or_known_default() == target.port_or_known_default()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageSecurityContext {
    pub document_url: Url,
    pub csp: CspPolicy,
    pub referrer_policy: ReferrerPolicy,
}

impl PageSecurityContext {
    #[must_use]
    pub fn permissive(document_url: Url) -> Self {
        Self {
            document_url,
            csp: CspPolicy::default(),
            referrer_policy: ReferrerPolicy::default(),
        }
    }
}

#[must_use]
pub fn is_mixed_active_content(source: &Url, target: &Url) -> bool {
    source.scheme() == "https" && matches!(target.scheme(), "http" | "ws")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HstsEntry {
    pub expires_at_unix: u64,
    pub include_subdomains: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HstsStore {
    hosts: HashMap<String, HstsEntry>,
}

impl HstsStore {
    pub fn observe_header(&mut self, response_url: &Url, value: &str) {
        if response_url.scheme() != "https" {
            return;
        }
        let Some(host) = response_url.host_str().map(|host| host.to_ascii_lowercase()) else {
            return;
        };
        let mut max_age = None;
        let mut include_subdomains = false;
        for part in value.split(';') {
            let part = part.trim();
            if let Some((name, value)) = part.split_once('=') {
                if name.trim().eq_ignore_ascii_case("max-age") {
                    max_age = value.trim().trim_matches('"').parse::<u64>().ok();
                }
            } else if part.eq_ignore_ascii_case("includeSubDomains") {
                include_subdomains = true;
            }
        }
        let Some(max_age) = max_age else { return };
        if max_age == 0 {
            self.hosts.remove(&host);
            return;
        }
        let expires_at_unix = now_unix().saturating_add(max_age);
        self.hosts.insert(host, HstsEntry { expires_at_unix, include_subdomains });
    }

    #[must_use]
    pub fn should_upgrade(&mut self, url: &Url) -> bool {
        if !matches!(url.scheme(), "http" | "ws") {
            return false;
        }
        self.prune_expired();
        let Some(host) = url.host_str().map(|host| host.to_ascii_lowercase()) else {
            return false;
        };
        if self.hosts.contains_key(&host) {
            return true;
        }
        self.hosts.iter().any(|(candidate, entry)| {
            entry.include_subdomains && host.ends_with(&format!(".{candidate}"))
        })
    }

    #[must_use]
    pub fn upgrade_url(&mut self, url: &Url) -> Option<Url> {
        if !self.should_upgrade(url) {
            return None;
        }
        let mut upgraded = url.clone();
        let secure_scheme = if url.scheme() == "ws" { "wss" } else { "https" };
        upgraded.set_scheme(secure_scheme).ok()?;
        if upgraded.port() == Some(80) {
            upgraded.set_port(None).ok()?;
        }
        Some(upgraded)
    }

    #[must_use]
    pub fn len(&mut self) -> usize {
        self.prune_expired();
        self.hosts.len()
    }

    pub fn clear(&mut self) {
        self.hosts.clear();
    }

    fn prune_expired(&mut self) {
        let now = now_unix();
        self.hosts.retain(|_, entry| entry.expires_at_unix > now);
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_origin_when_cross_origin_strips_path() {
        let source = Url::parse("https://a.example/private?q=1#x").unwrap();
        let target = Url::parse("https://b.example/").unwrap();
        assert_eq!(ReferrerPolicy::default().referer(&source, &target).as_deref(), Some("https://a.example/"));
    }

    #[test]
    fn mixed_http_from_https_is_blocked() {
        let source = Url::parse("https://a.example/").unwrap();
        let target = Url::parse("http://b.example/x").unwrap();
        assert!(is_mixed_active_content(&source, &target));
    }

    #[test]
    fn csp_self_and_inline_are_enforced() {
        let csp = CspPolicy::parse(Some("default-src 'self'; script-src 'self' 'unsafe-inline'; connect-src https://api.example"));
        let page = Url::parse("https://app.example/index").unwrap();
        assert!(csp.allows_inline_script());
        assert!(csp.allows_script_url(&page, &Url::parse("https://app.example/app.js").unwrap()));
        assert!(!csp.allows_script_url(&page, &Url::parse("https://evil.example/app.js").unwrap()));
        assert!(csp.allows_connect_url(&page, &Url::parse("https://api.example/data").unwrap()));
    }

    #[test]
    fn hsts_upgrades_subdomains() {
        let mut store = HstsStore::default();
        let response = Url::parse("https://example.com/").unwrap();
        store.observe_header(&response, "max-age=3600; includeSubDomains");
        let input = Url::parse("http://www.example.com/a").unwrap();
        assert_eq!(store.upgrade_url(&input).unwrap().scheme(), "https");
    }

    #[test]
    fn hsts_upgrades_websocket_to_wss() {
        let mut store = HstsStore::default();
        store.observe_header(&Url::parse("https://example.com/").unwrap(), "max-age=3600");
        let upgraded = store.upgrade_url(&Url::parse("ws://example.com/socket").unwrap()).unwrap();
        assert_eq!(upgraded.as_str(), "wss://example.com/socket");
    }
}
