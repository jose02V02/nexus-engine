//! Small HTTP response cache with freshness and validator support.
//!
//! Nexus 0.20 intentionally implements a conservative subset of RFC HTTP cache
//! semantics: GET only, Cache-Control max-age/no-cache/no-store/private,
//! ETag/If-None-Match, Last-Modified/If-Modified-Since and 304 revalidation.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use url::Url;

pub const DEFAULT_HTTP_CACHE_ENTRIES: usize = 128;
pub const DEFAULT_HTTP_CACHE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub struct CacheMetadata {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub cache_control: Option<String>,
    pub expires: Option<String>,
    pub vary: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CachedHttpResponse {
    pub final_url: Url,
    pub status: u16,
    pub content_type: Option<String>,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub metadata: CacheMetadata,
    stored_at: Instant,
    max_age: Duration,
    requires_revalidation: bool,
}

impl CachedHttpResponse {
    #[must_use]
    pub fn is_fresh(&self) -> bool {
        !self.requires_revalidation && self.stored_at.elapsed() < self.max_age
    }

    #[must_use]
    pub fn validators(&self) -> (Option<&str>, Option<&str>) {
        (
            self.metadata.etag.as_deref(),
            self.metadata.last_modified.as_deref(),
        )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HttpCacheStats {
    pub entries: usize,
    pub bytes: usize,
    pub hits: usize,
    pub misses: usize,
    pub revalidations: usize,
}

#[derive(Debug)]
pub struct HttpCache {
    entries: HashMap<String, CachedHttpResponse>,
    order: VecDeque<String>,
    current_bytes: usize,
    max_entries: usize,
    max_bytes: usize,
    hits: usize,
    misses: usize,
    revalidations: usize,
}

impl Default for HttpCache {
    fn default() -> Self {
        Self::new(DEFAULT_HTTP_CACHE_ENTRIES, DEFAULT_HTTP_CACHE_BYTES)
    }
}

impl HttpCache {
    #[must_use]
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            current_bytes: 0,
            max_entries: max_entries.max(1),
            max_bytes: max_bytes.max(1024 * 1024),
            hits: 0,
            misses: 0,
            revalidations: 0,
        }
    }

    pub fn lookup(&mut self, url: &Url) -> Option<CachedHttpResponse> {
        let key = url.as_str();
        let entry = self.entries.get(key).cloned();
        match entry {
            Some(entry) => {
                self.hits = self.hits.saturating_add(1);
                self.touch(key);
                Some(entry)
            }
            None => {
                self.misses = self.misses.saturating_add(1);
                None
            }
        }
    }

    pub fn insert(
        &mut self,
        requested_url: &Url,
        final_url: Url,
        status: u16,
        content_type: Option<String>,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) {
        if !(200..300).contains(&status) || body.len() > self.max_bytes {
            return;
        }
        let cache_control = header(&headers, "cache-control").map(str::to_owned);
        if cache_control.as_deref().is_some_and(|value| {
            let lower = value.to_ascii_lowercase();
            lower.contains("no-store") || lower.contains("private")
        }) {
            return;
        }
        if headers.keys().any(|name| name.eq_ignore_ascii_case("set-cookie")) {
            return;
        }
        let max_age = cache_control
            .as_deref()
            .and_then(parse_max_age)
            .map(Duration::from_secs)
            .unwrap_or(Duration::ZERO);
        let requires_revalidation = max_age.is_zero()
            || cache_control
                .as_deref()
                .is_some_and(|value| value.to_ascii_lowercase().contains("no-cache"));
        let metadata = CacheMetadata {
            etag: header(&headers, "etag").map(str::to_owned),
            last_modified: header(&headers, "last-modified").map(str::to_owned),
            cache_control,
            expires: header(&headers, "expires").map(str::to_owned),
            vary: header(&headers, "vary").map(str::to_owned),
        };
        // Until Nexus keys cache entries by the request headers named in Vary,
        // skip every varied response. This is deliberately conservative and
        // avoids serving an Accept/Encoding/language variant to the wrong request.
        if metadata
            .vary
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            return;
        }
        let key = requested_url.as_str().to_owned();
        if let Some(previous) = self.entries.remove(&key) {
            self.current_bytes = self.current_bytes.saturating_sub(previous.body.len());
        }
        self.current_bytes = self.current_bytes.saturating_add(body.len());
        self.entries.insert(
            key.clone(),
            CachedHttpResponse {
                final_url,
                status,
                content_type,
                headers,
                body,
                metadata,
                stored_at: Instant::now(),
                max_age,
                requires_revalidation,
            },
        );
        self.order.retain(|item| item != &key);
        self.order.push_back(key);
        self.evict();
    }

    pub fn refresh_after_304(&mut self, url: &Url, response_headers: &HashMap<String, String>) {
        if let Some(entry) = self.entries.get_mut(url.as_str()) {
            entry.stored_at = Instant::now();
            entry.requires_revalidation = false;
            if let Some(control) = header(response_headers, "cache-control") {
                entry.metadata.cache_control = Some(control.to_owned());
                entry.max_age = parse_max_age(control)
                    .map(Duration::from_secs)
                    .unwrap_or(Duration::ZERO);
                entry.requires_revalidation = entry.max_age.is_zero()
                    || control.to_ascii_lowercase().contains("no-cache");
            }
            for (name, value) in response_headers {
                entry.headers.insert(name.clone(), value.clone());
            }
        }
        self.revalidations = self.revalidations.saturating_add(1);
    }

    #[must_use]
    pub fn stats(&self) -> HttpCacheStats {
        HttpCacheStats {
            entries: self.entries.len(),
            bytes: self.current_bytes,
            hits: self.hits,
            misses: self.misses,
            revalidations: self.revalidations,
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.current_bytes = 0;
    }

    fn touch(&mut self, key: &str) {
        self.order.retain(|item| item != key);
        self.order.push_back(key.to_owned());
    }

    fn evict(&mut self) {
        while self.entries.len() > self.max_entries || self.current_bytes > self.max_bytes {
            let Some(key) = self.order.pop_front() else { break };
            if let Some(entry) = self.entries.remove(&key) {
                self.current_bytes = self.current_bytes.saturating_sub(entry.body.len());
            }
        }
    }
}

fn parse_max_age(value: &str) -> Option<u64> {
    value.split(',').find_map(|directive| {
        let directive = directive.trim();
        let (name, value) = directive.split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case("max-age")
            .then(|| value.trim().trim_matches('"').parse().ok())
            .flatten()
    })
}

fn header<'a>(headers: &'a HashMap<String, String>, wanted: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(wanted))
        .map(|(_, value)| value.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_age_is_parsed() {
        assert_eq!(parse_max_age("public, max-age=120"), Some(120));
    }

    #[test]
    fn no_store_is_not_cached() {
        let mut cache = HttpCache::default();
        let url = Url::parse("https://example.com/").unwrap();
        let mut headers = HashMap::new();
        headers.insert("cache-control".to_owned(), "no-store".to_owned());
        cache.insert(&url, url.clone(), 200, None, headers, vec![1, 2, 3]);
        assert_eq!(cache.stats().entries, 0);
    }
}
