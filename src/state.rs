//! Shared browser-profile state for Nexus Engine 1.02.

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use reqwest_cookie_store::{CookieStore, CookieStoreMutex};

use crate::cache::{HttpCache, HttpCacheStats, DEFAULT_HTTP_CACHE_BYTES, DEFAULT_HTTP_CACHE_ENTRIES};
use crate::origin::Origin;
use crate::policy::HstsStore;
use crate::permissions::{PermissionKind, PermissionState, PermissionStore};
use crate::storage::{profile_storage_file, LocalStorageStore, StorageError, DEFAULT_STORAGE_QUOTA_BYTES};

#[derive(Debug)]
pub struct BrowserState {
    cookies: Arc<CookieStoreMutex>,
    http_cache: Mutex<HttpCache>,
    local_storage: Mutex<LocalStorageStore>,
    permissions: Mutex<PermissionStore>,
    hsts: Mutex<HstsStore>,
    cookie_file: Option<PathBuf>,
    hsts_file: Option<PathBuf>,
}

impl BrowserState {
    #[must_use]
    pub fn new(profile_dir: Option<PathBuf>) -> Arc<Self> {
        let storage_file = profile_dir.as_deref().map(profile_storage_file);
        let cookie_file = profile_dir.as_deref().map(|dir| dir.join("cookies.json"));
        let hsts_file = profile_dir.as_deref().map(|dir| dir.join("hsts.json"));
        let cookie_store = cookie_file
            .as_deref()
            .and_then(load_cookie_store)
            .unwrap_or_else(CookieStore::new);
        let hsts_store = hsts_file
            .as_deref()
            .and_then(load_hsts_store)
            .unwrap_or_default();
        Arc::new(Self {
            cookies: Arc::new(CookieStoreMutex::new(cookie_store)),
            http_cache: Mutex::new(HttpCache::new(
                DEFAULT_HTTP_CACHE_ENTRIES,
                DEFAULT_HTTP_CACHE_BYTES,
            )),
            local_storage: Mutex::new(LocalStorageStore::new(
                storage_file,
                DEFAULT_STORAGE_QUOTA_BYTES,
            )),
            permissions: Mutex::new(PermissionStore::default()),
            hsts: Mutex::new(hsts_store),
            cookie_file,
            hsts_file,
        })
    }

    #[must_use]
    pub fn cookies(&self) -> Arc<CookieStoreMutex> {
        Arc::clone(&self.cookies)
    }

    pub fn http_cache(&self) -> &Mutex<HttpCache> {
        &self.http_cache
    }

    #[must_use]
    pub fn http_cache_stats(&self) -> HttpCacheStats {
        self.http_cache
            .lock()
            .map(|cache| cache.stats())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn cookie_count(&self) -> usize {
        self.cookies
            .lock()
            .map(|store| store.iter_unexpired().count())
            .unwrap_or(0)
    }

    pub fn persist_cookies_best_effort(&self) {
        let Some(path) = self.cookie_file.as_deref() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let temp = path.with_extension("tmp");
        let Ok(file) = File::create(&temp) else { return };
        let mut writer = BufWriter::new(file);
        let Ok(store) = self.cookies.lock() else { return };
        #[allow(deprecated)]
        if store.save_json(&mut writer).is_ok() {
            drop(writer);
            let _ = std::fs::rename(temp, path);
        }
    }

    pub fn local_get(&self, origin: &Origin, key: &str) -> Result<Option<String>, StorageError> {
        self.local_storage
            .lock()
            .map_or(Ok(None), |store| store.get(origin, key))
    }

    pub fn local_set(&self, origin: &Origin, key: &str, value: &str) -> Result<(), StorageError> {
        self.local_storage
            .lock()
            .map_or(Ok(()), |mut store| store.set(origin, key, value))
    }

    pub fn local_remove(&self, origin: &Origin, key: &str) -> Result<(), StorageError> {
        self.local_storage
            .lock()
            .map_or(Ok(()), |mut store| store.remove(origin, key))
    }

    pub fn local_clear(&self, origin: &Origin) -> Result<(), StorageError> {
        self.local_storage
            .lock()
            .map_or(Ok(()), |mut store| store.clear(origin))
    }

    pub fn local_key(&self, origin: &Origin, index: usize) -> Result<Option<String>, StorageError> {
        self.local_storage
            .lock()
            .map_or(Ok(None), |store| store.key(origin, index))
    }

    pub fn local_len(&self, origin: &Origin) -> Result<usize, StorageError> {
        self.local_storage
            .lock()
            .map_or(Ok(0), |store| store.len(origin))
    }

    #[must_use]
    pub fn local_origin_count(&self) -> usize {
        self.local_storage
            .lock()
            .map(|store| store.origin_count())
            .unwrap_or(0)
    }

    #[must_use]
    pub fn permission(&self, origin: &Origin, kind: PermissionKind) -> PermissionState {
        self.permissions
            .lock()
            .map(|store| store.get(origin, kind))
            .unwrap_or(PermissionState::Prompt)
    }

    pub fn set_permission(&self, origin: &Origin, kind: PermissionKind, state: PermissionState) {
        if let Ok(mut store) = self.permissions.lock() {
            store.set(origin, kind, state);
        }
    }

    pub fn hsts_upgrade(&self, url: &url::Url) -> Option<url::Url> {
        self.hsts.lock().ok().and_then(|mut store| store.upgrade_url(url))
    }

    pub fn observe_hsts(&self, response_url: &url::Url, header: &str) {
        if let Ok(mut store) = self.hsts.lock() {
            store.observe_header(response_url, header);
        }
        self.persist_hsts_best_effort();
    }

    #[must_use]
    pub fn hsts_count(&self) -> usize {
        self.hsts.lock().map(|mut store| store.len()).unwrap_or(0)
    }

    fn persist_hsts_best_effort(&self) {
        let Some(path) = self.hsts_file.as_deref() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let Ok(store) = self.hsts.lock() else { return };
        let Ok(bytes) = serde_json::to_vec_pretty(&*store) else { return };
        let temp = path.with_extension("tmp");
        if std::fs::write(&temp, bytes).is_ok() {
            let _ = std::fs::rename(temp, path);
        }
    }

    pub fn clear_http_cache(&self) {
        if let Ok(mut cache) = self.http_cache.lock() { cache.clear(); }
    }

    pub fn clear_local_storage_all(&self) {
        if let Ok(mut store) = self.local_storage.lock() { store.clear_all(); }
    }

    pub fn clear_permissions_all(&self) {
        if let Ok(mut store) = self.permissions.lock() { store.clear_all(); }
    }

    pub fn clear_hsts_all(&self) {
        if let Ok(mut store) = self.hsts.lock() { store.clear(); }
        self.persist_hsts_best_effort();
    }

    pub fn clear_cookies_all(&self) {
        if let Ok(mut store) = self.cookies.lock() { *store = CookieStore::new(); }
        self.persist_cookies_best_effort();
    }

    #[must_use]
    pub fn permission_count(&self) -> usize {
        self.permissions.lock().map(|store| store.len()).unwrap_or(0)
    }
}

#[must_use]
pub fn profile_dir_from_path(path: impl AsRef<Path>) -> PathBuf {
    path.as_ref().to_path_buf()
}


#[allow(deprecated)]
fn load_cookie_store(path: &Path) -> Option<CookieStore> {
    let file = File::open(path).ok()?;
    CookieStore::load_json(BufReader::new(file)).ok()
}

fn load_hsts_store(path: &Path) -> Option<HstsStore> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}
