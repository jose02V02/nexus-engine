//! Origin-scoped Web Storage for Nexus Engine 1.02.
//!
//! localStorage may be persisted to JSON when a profile path is configured.
//! sessionStorage is kept in a per-BrowserSession namespace.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::origin::Origin;

pub const DEFAULT_STORAGE_QUOTA_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SerializableArea {
    values: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct StorageArea {
    values: BTreeMap<String, String>,
    quota_bytes: usize,
}

impl Default for StorageArea {
    fn default() -> Self {
        Self::new(DEFAULT_STORAGE_QUOTA_BYTES)
    }
}

impl StorageArea {
    #[must_use]
    pub fn new(quota_bytes: usize) -> Self {
        Self {
            values: BTreeMap::new(),
            quota_bytes: quota_bytes.max(1024),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), StorageError> {
        let previous = self.values.insert(key.to_owned(), value.to_owned());
        if self.bytes() > self.quota_bytes {
            match previous {
                Some(previous) => {
                    self.values.insert(key.to_owned(), previous);
                }
                None => {
                    self.values.remove(key);
                }
            }
            return Err(StorageError::QuotaExceeded);
        }
        Ok(())
    }

    pub fn remove(&mut self, key: &str) {
        self.values.remove(key);
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }

    #[must_use]
    pub fn key(&self, index: usize) -> Option<String> {
        self.values.keys().nth(index).cloned()
    }

    #[must_use]
    pub fn bytes(&self) -> usize {
        self.values
            .iter()
            .map(|(key, value)| key.len().saturating_add(value.len()))
            .sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    QuotaExceeded,
    OpaqueOrigin,
}

#[derive(Debug)]
pub struct LocalStorageStore {
    areas: HashMap<String, StorageArea>,
    profile_file: Option<PathBuf>,
    quota_bytes: usize,
}

impl LocalStorageStore {
    #[must_use]
    pub fn new(profile_file: Option<PathBuf>, quota_bytes: usize) -> Self {
        let mut store = Self {
            areas: HashMap::new(),
            profile_file,
            quota_bytes: quota_bytes.max(1024),
        };
        store.load_from_disk();
        store
    }

    pub fn get(&self, origin: &Origin, key: &str) -> Result<Option<String>, StorageError> {
        let origin = storage_key(origin)?;
        Ok(self.areas.get(&origin).and_then(|area| area.get(key)))
    }

    pub fn set(&mut self, origin: &Origin, key: &str, value: &str) -> Result<(), StorageError> {
        let origin = storage_key(origin)?;
        self.areas
            .entry(origin)
            .or_insert_with(|| StorageArea::new(self.quota_bytes))
            .set(key, value)?;
        self.persist_best_effort();
        Ok(())
    }

    pub fn remove(&mut self, origin: &Origin, key: &str) -> Result<(), StorageError> {
        let origin = storage_key(origin)?;
        if let Some(area) = self.areas.get_mut(&origin) {
            area.remove(key);
        }
        self.persist_best_effort();
        Ok(())
    }

    pub fn clear(&mut self, origin: &Origin) -> Result<(), StorageError> {
        let origin = storage_key(origin)?;
        self.areas.remove(&origin);
        self.persist_best_effort();
        Ok(())
    }

    pub fn key(&self, origin: &Origin, index: usize) -> Result<Option<String>, StorageError> {
        let origin = storage_key(origin)?;
        Ok(self.areas.get(&origin).and_then(|area| area.key(index)))
    }

    pub fn len(&self, origin: &Origin) -> Result<usize, StorageError> {
        let origin = storage_key(origin)?;
        Ok(self.areas.get(&origin).map_or(0, StorageArea::len))
    }

    #[must_use]
    pub fn origin_count(&self) -> usize {
        self.areas.len()
    }

    pub fn clear_all(&mut self) {
        self.areas.clear();
        self.persist_best_effort();
    }

    fn load_from_disk(&mut self) {
        let Some(path) = self.profile_file.as_deref() else { return };
        let Ok(bytes) = fs::read(path) else { return };
        let Ok(serialized) = serde_json::from_slice::<HashMap<String, SerializableArea>>(&bytes) else {
            return;
        };
        self.areas = serialized
            .into_iter()
            .map(|(origin, area)| {
                (
                    origin,
                    StorageArea {
                        values: area.values,
                        quota_bytes: self.quota_bytes,
                    },
                )
            })
            .collect();
    }

    fn persist_best_effort(&self) {
        let Some(path) = self.profile_file.as_deref() else { return };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let serializable: HashMap<String, SerializableArea> = self
            .areas
            .iter()
            .map(|(origin, area)| {
                (
                    origin.clone(),
                    SerializableArea {
                        values: area.values.clone(),
                    },
                )
            })
            .collect();
        if let Ok(bytes) = serde_json::to_vec_pretty(&serializable) {
            let temp = path.with_extension("tmp");
            if fs::write(&temp, bytes).is_ok() {
                let _ = fs::rename(temp, path);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionStorage {
    areas: Arc<Mutex<HashMap<String, StorageArea>>>,
    quota_bytes: usize,
}

impl Default for SessionStorage {
    fn default() -> Self {
        Self::new(DEFAULT_STORAGE_QUOTA_BYTES)
    }
}

impl SessionStorage {
    #[must_use]
    pub fn new(quota_bytes: usize) -> Self {
        Self {
            areas: Arc::new(Mutex::new(HashMap::new())),
            quota_bytes: quota_bytes.max(1024),
        }
    }

    pub fn get(&self, origin: &Origin, key: &str) -> Result<Option<String>, StorageError> {
        let origin = storage_key(origin)?;
        Ok(self
            .areas
            .lock()
            .ok()
            .and_then(|areas| areas.get(&origin).and_then(|area| area.get(key))))
    }

    pub fn set(&self, origin: &Origin, key: &str, value: &str) -> Result<(), StorageError> {
        let origin = storage_key(origin)?;
        let Ok(mut areas) = self.areas.lock() else { return Ok(()) };
        areas
            .entry(origin)
            .or_insert_with(|| StorageArea::new(self.quota_bytes))
            .set(key, value)
    }

    pub fn remove(&self, origin: &Origin, key: &str) -> Result<(), StorageError> {
        let origin = storage_key(origin)?;
        if let Ok(mut areas) = self.areas.lock() {
            if let Some(area) = areas.get_mut(&origin) {
                area.remove(key);
            }
        }
        Ok(())
    }

    pub fn clear(&self, origin: &Origin) -> Result<(), StorageError> {
        let origin = storage_key(origin)?;
        if let Ok(mut areas) = self.areas.lock() {
            areas.remove(&origin);
        }
        Ok(())
    }

    pub fn key(&self, origin: &Origin, index: usize) -> Result<Option<String>, StorageError> {
        let origin = storage_key(origin)?;
        Ok(self
            .areas
            .lock()
            .ok()
            .and_then(|areas| areas.get(&origin).and_then(|area| area.key(index))))
    }

    pub fn len(&self, origin: &Origin) -> Result<usize, StorageError> {
        let origin = storage_key(origin)?;
        Ok(self
            .areas
            .lock()
            .ok()
            .and_then(|areas| areas.get(&origin).map(StorageArea::len))
            .unwrap_or(0))
    }
}

fn storage_key(origin: &Origin) -> Result<String, StorageError> {
    match origin {
        Origin::Opaque => Err(StorageError::OpaqueOrigin),
        _ => Ok(origin.serialize()),
    }
}

#[must_use]
pub fn profile_storage_file(profile_dir: &Path) -> PathBuf {
    profile_dir.join("local-storage.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn storage_is_origin_scoped() {
        let a = Origin::from_url(&Url::parse("https://a.example/").unwrap());
        let b = Origin::from_url(&Url::parse("https://b.example/").unwrap());
        let mut store = LocalStorageStore::new(None, 1024);
        store.set(&a, "key", "A").unwrap();
        store.set(&b, "key", "B").unwrap();
        assert_eq!(store.get(&a, "key").unwrap().as_deref(), Some("A"));
        assert_eq!(store.get(&b, "key").unwrap().as_deref(), Some("B"));
    }

    #[test]
    fn quota_is_enforced() {
        let origin = Origin::from_url(&Url::parse("https://example.com/").unwrap());
        let mut store = LocalStorageStore::new(None, 1024);
        assert_eq!(
            store.set(&origin, "large", &"x".repeat(2048)),
            Err(StorageError::QuotaExceeded)
        );
    }
}
