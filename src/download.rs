//! Download manager for Nexus Engine 1.02.
//!
//! Downloads are explicit browser jobs. They reuse Nexus' cookie/HSTS-aware
//! `NetworkClient` but stream to disk instead of loading the whole file in RAM.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{NexusError, NexusResult};
use crate::network::NetworkClient;

pub const DEFAULT_MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadStatus {
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadItem {
    pub id: u64,
    pub requested_url: String,
    pub final_url: Option<String>,
    pub file_name: String,
    pub path: PathBuf,
    pub bytes_written: u64,
    pub content_type: Option<String>,
    pub status: DownloadStatus,
    pub error: Option<String>,
    pub finished_at_ms: u64,
}

pub struct DownloadManager {
    network: NetworkClient,
    directory: PathBuf,
    max_bytes: u64,
    items: Vec<DownloadItem>,
    next_id: u64,
    history_file: Option<PathBuf>,
}

impl DownloadManager {
    #[must_use]
    pub fn new(network: NetworkClient, directory: PathBuf, history_file: Option<PathBuf>) -> Self {
        let items = history_file
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Vec<DownloadItem>>(&bytes).ok())
            .unwrap_or_default();
        let next_id = items.iter().map(|item| item.id).max().unwrap_or(0).saturating_add(1);
        Self {
            network,
            directory,
            max_bytes: DEFAULT_MAX_DOWNLOAD_BYTES,
            items,
            next_id,
            history_file,
        }
    }

    #[must_use]
    pub fn max_bytes(mut self, max_bytes: u64) -> Self {
        self.max_bytes = max_bytes.max(1024 * 1024);
        self
    }

    pub fn download(&mut self, url: &Url, suggested_name: Option<&str>) -> NexusResult<DownloadItem> {
        std::fs::create_dir_all(&self.directory)?;
        let fallback = filename_from_url(url).unwrap_or_else(|| "download.bin".to_owned());
        let requested_name = suggested_name.filter(|value| !value.trim().is_empty()).unwrap_or(&fallback);
        let file_name = unique_file_name(&self.directory, &sanitize_file_name(requested_name));
        let path = self.directory.join(&file_name);
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);

        match self.network.download_to_file(url, &path, self.max_bytes) {
            Ok(meta) => {
                let item = DownloadItem {
                    id,
                    requested_url: url.as_str().to_owned(),
                    final_url: Some(meta.final_url.as_str().to_owned()),
                    file_name,
                    path,
                    bytes_written: meta.bytes_written,
                    content_type: meta.content_type,
                    status: DownloadStatus::Completed,
                    error: None,
                    finished_at_ms: now_ms(),
                };
                self.items.push(item.clone());
                self.persist_best_effort();
                Ok(item)
            }
            Err(error) => {
                let _ = std::fs::remove_file(&path);
                let item = DownloadItem {
                    id,
                    requested_url: url.as_str().to_owned(),
                    final_url: None,
                    file_name,
                    path,
                    bytes_written: 0,
                    content_type: None,
                    status: DownloadStatus::Failed,
                    error: Some(error.to_string()),
                    finished_at_ms: now_ms(),
                };
                self.items.push(item);
                self.persist_best_effort();
                Err(error)
            }
        }
    }

    #[must_use]
    pub fn items(&self) -> &[DownloadItem] {
        &self.items
    }

    #[must_use]
    pub fn completed_count(&self) -> usize {
        self.items.iter().filter(|item| item.status == DownloadStatus::Completed).count()
    }

    pub fn clear_history(&mut self) {
        self.items.clear();
        self.persist_best_effort();
    }

    fn persist_best_effort(&self) {
        let Some(path) = self.history_file.as_deref() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let Ok(bytes) = serde_json::to_vec_pretty(&self.items) else { return };
        let temp = path.with_extension("tmp");
        if std::fs::write(&temp, bytes).is_ok() {
            let _ = std::fs::rename(temp, path);
        }
    }
}

fn filename_from_url(url: &Url) -> Option<String> {
    url.path_segments()?
        .filter(|part| !part.is_empty())
        .next_back()
        .map(str::to_owned)
}

fn sanitize_file_name(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' | ' ') { ch } else { '_' })
        .collect::<String>();
    let sanitized = sanitized.trim().trim_matches('.').trim();
    if sanitized.is_empty() { "download.bin".to_owned() } else { sanitized.chars().take(120).collect() }
}

fn unique_file_name(directory: &Path, requested: &str) -> String {
    if !directory.join(requested).exists() {
        return requested.to_owned();
    }
    let path = Path::new(requested);
    let stem = path.file_stem().and_then(|value| value.to_str()).unwrap_or("download");
    let extension = path.extension().and_then(|value| value.to_str());
    for suffix in 1..10_000 {
        let candidate = match extension {
            Some(ext) => format!("{stem} ({suffix}).{ext}"),
            None => format!("{stem} ({suffix})"),
        };
        if !directory.join(&candidate).exists() {
            return candidate;
        }
    }
    format!("download-{}.bin", now_ms())
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
    fn sanitizes_unsafe_file_names() {
        assert_eq!(sanitize_file_name("../my:file?.pdf"), "_my_file_.pdf");
    }

    #[test]
    fn derives_filename_from_url() {
        let url = Url::parse("https://example.com/files/report.pdf?x=1").unwrap();
        assert_eq!(filename_from_url(&url).as_deref(), Some("report.pdf"));
    }
}
