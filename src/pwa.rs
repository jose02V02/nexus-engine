//! Offline PWA primitives for Nexus Engine 1.02.
//!
//! Cache Storage and scope selection are executable. JavaScript Service Worker
//! realms and the complete specification update algorithm remain separate
//! adapters and are not claimed by this module.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::web_platform::{ServiceWorkerRegistration, ServiceWorkerState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedFetchResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl CachedFetchResponse {
    #[must_use]
    pub fn storage_bytes(&self) -> usize {
        self.body.len() + self.headers.iter().map(|(name, value)| name.len() + value.len()).sum::<usize>()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheStorageError {
    InvalidCacheName,
    NonHttpUrl,
    QuotaExceeded { requested: usize, available: usize },
}

#[derive(Debug, Clone, Default)]
struct NamedCache { entries: HashMap<String, CachedFetchResponse> }

#[derive(Debug, Clone)]
pub struct CacheStorage {
    caches: HashMap<String, NamedCache>,
    maximum_bytes: usize,
    used_bytes: usize,
}

impl CacheStorage {
    #[must_use]
    pub fn new(maximum_bytes: usize) -> Self {
        Self { caches: HashMap::new(), maximum_bytes: maximum_bytes.max(1), used_bytes: 0 }
    }

    pub fn open(&mut self, name: &str) -> Result<(), CacheStorageError> {
        if name.trim().is_empty() { return Err(CacheStorageError::InvalidCacheName); }
        self.caches.entry(name.to_owned()).or_default();
        Ok(())
    }

    pub fn put(&mut self, cache_name: &str, url: &Url, response: CachedFetchResponse) -> Result<(), CacheStorageError> {
        if !matches!(url.scheme(), "http" | "https") { return Err(CacheStorageError::NonHttpUrl); }
        self.open(cache_name)?;
        let key = canonical_request_key(url, false);
        let old_bytes = self.caches.get(cache_name).and_then(|cache| cache.entries.get(&key))
            .map_or(0, CachedFetchResponse::storage_bytes);
        let requested = response.storage_bytes();
        let prospective = self.used_bytes.saturating_sub(old_bytes).saturating_add(requested);
        if prospective > self.maximum_bytes {
            return Err(CacheStorageError::QuotaExceeded {
                requested,
                available: self.maximum_bytes.saturating_sub(self.used_bytes.saturating_sub(old_bytes)),
            });
        }
        self.caches.get_mut(cache_name).expect("cache opened").entries.insert(key, response);
        self.used_bytes = prospective;
        Ok(())
    }

    #[must_use]
    pub fn match_request(&self, cache_name: &str, url: &Url, ignore_search: bool) -> Option<&CachedFetchResponse> {
        let cache = self.caches.get(cache_name)?;
        if !ignore_search { return cache.entries.get(&canonical_request_key(url, false)); }
        let wanted = canonical_request_key(url, true);
        cache.entries.iter().find_map(|(key, response)| {
            Url::parse(key).ok().filter(|candidate| canonical_request_key(candidate, true) == wanted).map(|_| response)
        })
    }

    pub fn delete_entry(&mut self, cache_name: &str, url: &Url) -> bool {
        let Some(response) = self.caches.get_mut(cache_name)
            .and_then(|cache| cache.entries.remove(&canonical_request_key(url, false))) else { return false };
        self.used_bytes = self.used_bytes.saturating_sub(response.storage_bytes());
        true
    }

    pub fn delete_cache(&mut self, cache_name: &str) -> bool {
        let Some(cache) = self.caches.remove(cache_name) else { return false };
        let removed = cache.entries.values().map(CachedFetchResponse::storage_bytes).sum::<usize>();
        self.used_bytes = self.used_bytes.saturating_sub(removed);
        true
    }

    #[must_use]
    pub fn keys(&self) -> Vec<&str> {
        let mut names = self.caches.keys().map(String::as_str).collect::<Vec<_>>();
        names.sort_unstable();
        names
    }

    #[must_use]
    pub fn used_bytes(&self) -> usize { self.used_bytes }
}

fn canonical_request_key(url: &Url, ignore_search: bool) -> String {
    let mut canonical = url.clone();
    canonical.set_fragment(None);
    if ignore_search { canonical.set_query(None); }
    canonical.to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceWorkerError { InsecureOrigin, CrossOriginScript, InvalidScope }

#[derive(Debug, Default)]
pub struct ServiceWorkerManager { registrations: Vec<ServiceWorkerRegistration> }

impl ServiceWorkerManager {
    pub fn register(&mut self, document: &Url, script: &Url, scope: &Url) -> Result<usize, ServiceWorkerError> {
        if document.scheme() != "https" && document.host_str() != Some("localhost") {
            return Err(ServiceWorkerError::InsecureOrigin);
        }
        if document.origin() != script.origin() { return Err(ServiceWorkerError::CrossOriginScript); }
        if document.origin() != scope.origin() { return Err(ServiceWorkerError::InvalidScope); }
        let registration = ServiceWorkerRegistration {
            scope: scope.as_str().to_owned(),
            script_url: script.as_str().to_owned(),
            state: ServiceWorkerState::Installing,
        };
        if let Some(index) = self.registrations.iter().position(|item| item.scope == registration.scope) {
            self.registrations[index] = registration;
            return Ok(index);
        }
        self.registrations.push(registration);
        Ok(self.registrations.len() - 1)
    }

    pub fn activate(&mut self, index: usize) -> bool {
        let Some(registration) = self.registrations.get_mut(index) else { return false };
        registration.activate();
        true
    }

    #[must_use]
    pub fn controller_for(&self, url: &Url) -> Option<&ServiceWorkerRegistration> {
        self.registrations.iter()
            .filter(|registration| registration.state == ServiceWorkerState::Activated && url.as_str().starts_with(&registration.scope))
            .max_by_key(|registration| registration.scope.len())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchStrategy { CacheFirst, NetworkFirst, StaleWhileRevalidate }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchRoute {
    Cached { response: CachedFetchResponse, revalidate: bool },
    Network { cached_fallback: Option<CachedFetchResponse> },
}

#[must_use]
pub fn route_fetch(storage: &CacheStorage, cache_name: &str, url: &Url, strategy: FetchStrategy) -> FetchRoute {
    let cached = storage.match_request(cache_name, url, false).cloned();
    match (strategy, cached) {
        (FetchStrategy::CacheFirst, Some(response)) => FetchRoute::Cached { response, revalidate: false },
        (FetchStrategy::StaleWhileRevalidate, Some(response)) => FetchRoute::Cached { response, revalidate: true },
        (_, fallback) => FetchRoute::Network { cached_fallback: fallback },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PwaManifest {
    pub name: String,
    #[serde(default)]
    pub short_name: String,
    #[serde(default = "default_start_url")]
    pub start_url: String,
    #[serde(default = "default_display")]
    pub display: String,
    #[serde(default)]
    pub theme_color: Option<String>,
    #[serde(default)]
    pub background_color: Option<String>,
}

fn default_start_url() -> String { "/".to_owned() }
fn default_display() -> String { "browser".to_owned() }

pub fn parse_pwa_manifest(json: &str) -> Result<PwaManifest, serde_json::Error> {
    serde_json::from_str(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(value: &str) -> Url { Url::parse(value).unwrap() }
    fn response(body: &[u8]) -> CachedFetchResponse {
        CachedFetchResponse { status: 200, headers: vec![("content-type".to_owned(), "text/plain".to_owned())], body: body.to_vec() }
    }

    #[test]
    fn cache_storage_put_match_delete_and_accounting() {
        let mut storage = CacheStorage::new(1024);
        let request = url("https://app.test/data#fragment");
        storage.put("v1", &request, response(b"offline")).unwrap();
        assert_eq!(storage.match_request("v1", &url("https://app.test/data"), false).unwrap().body, b"offline");
        assert!(storage.used_bytes() > 0);
        assert!(storage.delete_entry("v1", &request));
        assert_eq!(storage.used_bytes(), 0);
    }

    #[test]
    fn cache_quota_is_enforced_before_mutation() {
        let mut storage = CacheStorage::new(8);
        let error = storage.put("small", &url("https://app.test/large"), response(b"too-large")).unwrap_err();
        assert!(matches!(error, CacheStorageError::QuotaExceeded { .. }));
        assert_eq!(storage.used_bytes(), 0);
    }

    #[test]
    fn query_can_be_ignored_during_matching() {
        let mut storage = CacheStorage::new(1024);
        storage.put("v1", &url("https://app.test/data?v=1"), response(b"one")).unwrap();
        assert!(storage.match_request("v1", &url("https://app.test/data?v=2"), true).is_some());
    }

    #[test]
    fn longest_activated_scope_controls_the_page() {
        let mut workers = ServiceWorkerManager::default();
        let document = url("https://app.test/index.html");
        let broad = workers.register(&document, &url("https://app.test/sw.js"), &url("https://app.test/")).unwrap();
        let narrow = workers.register(&document, &url("https://app.test/admin-sw.js"), &url("https://app.test/admin/")).unwrap();
        workers.activate(broad); workers.activate(narrow);
        assert!(workers.controller_for(&url("https://app.test/admin/panel")).unwrap().script_url.ends_with("admin-sw.js"));
    }

    #[test]
    fn insecure_and_cross_origin_registrations_are_rejected() {
        let mut workers = ServiceWorkerManager::default();
        assert_eq!(workers.register(&url("http://app.test/"), &url("http://app.test/sw.js"), &url("http://app.test/")), Err(ServiceWorkerError::InsecureOrigin));
        assert_eq!(workers.register(&url("https://app.test/"), &url("https://cdn.test/sw.js"), &url("https://app.test/")), Err(ServiceWorkerError::CrossOriginScript));
    }

    #[test]
    fn fetch_strategies_expose_cache_and_network_intent() {
        let mut storage = CacheStorage::new(1024);
        let request = url("https://app.test/offline");
        storage.put("v1", &request, response(b"cached")).unwrap();
        assert!(matches!(route_fetch(&storage, "v1", &request, FetchStrategy::CacheFirst), FetchRoute::Cached { revalidate: false, .. }));
        assert!(matches!(route_fetch(&storage, "v1", &request, FetchStrategy::NetworkFirst), FetchRoute::Network { cached_fallback: Some(_), .. }));
    }

    #[test]
    fn parses_installable_manifest_defaults() {
        let manifest = parse_pwa_manifest(r##"{"name":"Nexus Notes","short_name":"Notes","theme_color":"#112233"}"##).unwrap();
        assert_eq!(manifest.start_url, "/");
        assert_eq!(manifest.display, "browser");
    }
}
