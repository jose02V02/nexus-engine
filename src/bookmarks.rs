//! Persistent bookmarks for Nexus Engine 1.02.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{NexusError, NexusResult};

const BOOKMARKS_FILE: &str = "bookmarks.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bookmark {
    pub url: String,
    pub title: String,
    pub created_ms: u64,
}

#[derive(Debug, Default)]
pub struct BookmarkStore {
    items: Vec<Bookmark>,
    file: Option<PathBuf>,
}

impl BookmarkStore {
    #[must_use]
    pub fn new(profile_dir: Option<&Path>) -> Self {
        let file = profile_dir.map(|dir| dir.join(BOOKMARKS_FILE));
        let items = file
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Vec<Bookmark>>(&bytes).ok())
            .unwrap_or_default();
        Self { items, file }
    }

    #[must_use]
    pub fn items(&self) -> &[Bookmark] {
        &self.items
    }

    #[must_use]
    pub fn contains(&self, url: &Url) -> bool {
        self.items.iter().any(|item| item.url == url.as_str())
    }

    pub fn add(&mut self, url: &Url, title: &str) -> NexusResult<()> {
        if let Some(existing) = self.items.iter_mut().find(|item| item.url == url.as_str()) {
            if !title.trim().is_empty() {
                existing.title = title.trim().to_owned();
            }
            return self.persist();
        }
        self.items.push(Bookmark {
            url: url.as_str().to_owned(),
            title: if title.trim().is_empty() { url.as_str().to_owned() } else { title.trim().to_owned() },
            created_ms: now_ms(),
        });
        self.persist()
    }

    pub fn remove(&mut self, url: &Url) -> NexusResult<bool> {
        let before = self.items.len();
        self.items.retain(|item| item.url != url.as_str());
        let changed = self.items.len() != before;
        if changed {
            self.persist()?;
        }
        Ok(changed)
    }

    pub fn toggle(&mut self, url: &Url, title: &str) -> NexusResult<bool> {
        if self.contains(url) {
            self.remove(url)?;
            Ok(false)
        } else {
            self.add(url, title)?;
            Ok(true)
        }
    }

    #[must_use]
    pub fn matching(&self, query: &str, limit: usize) -> Vec<Bookmark> {
        let needle = query.trim().to_ascii_lowercase();
        let mut items = self
            .items
            .iter()
            .filter(|item| {
                needle.is_empty()
                    || item.url.to_ascii_lowercase().contains(&needle)
                    || item.title.to_ascii_lowercase().contains(&needle)
            })
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by_key(|item| std::cmp::Reverse(item.created_ms));
        items.truncate(limit);
        items
    }

    fn persist(&self) -> NexusResult<()> {
        let Some(path) = self.file.as_deref() else { return Ok(()) };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(&self.items)
            .map_err(|error| NexusError::Storage(format!("cannot serialize bookmarks: {error}")))?;
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, bytes)?;
        std::fs::rename(temp, path)?;
        Ok(())
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_bookmark() {
        let mut store = BookmarkStore::default();
        let url = Url::parse("https://example.com/").unwrap();
        assert!(store.toggle(&url, "Example").unwrap());
        assert!(store.contains(&url));
        assert!(!store.toggle(&url, "Example").unwrap());
        assert!(!store.contains(&url));
    }
}
