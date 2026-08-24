//! Address-bar history and autocomplete for Nexus Engine 1.02.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuggestionSource {
    OpenTab,
    Bookmark,
    History,
    Direct,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddressSuggestion {
    pub value: String,
    pub title: String,
    pub score: i64,
    pub source: SuggestionSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HistoryEntry {
    url: String,
    title: String,
    visits: u64,
    last_visit_ms: u64,
}

#[derive(Debug, Default)]
pub struct AddressHistory {
    entries: HashMap<String, HistoryEntry>,
    file: Option<PathBuf>,
}

impl AddressHistory {
    #[must_use]
    pub fn new(profile_dir: Option<&Path>) -> Self {
        let file = profile_dir.map(|dir| dir.join("history.json"));
        let entries = file
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Vec<HistoryEntry>>(&bytes).ok())
            .map(|items| items.into_iter().map(|entry| (entry.url.clone(), entry)).collect())
            .unwrap_or_default();
        Self { entries, file }
    }

    pub fn record(&mut self, url: &str, title: &str) {
        let now = now_ms();
        let entry = self.entries.entry(url.to_owned()).or_insert_with(|| HistoryEntry {
            url: url.to_owned(),
            title: title.to_owned(),
            visits: 0,
            last_visit_ms: now,
        });
        entry.visits = entry.visits.saturating_add(1);
        entry.last_visit_ms = now;
        if !title.trim().is_empty() {
            entry.title = title.to_owned();
        }
        self.persist_best_effort();
    }

    #[must_use]
    pub fn suggestions(&self, query: &str, limit: usize) -> Vec<AddressSuggestion> {
        let needle = query.trim().to_ascii_lowercase();
        if needle.is_empty() {
            let mut entries = self.entries.values().collect::<Vec<_>>();
            entries.sort_by_key(|entry| std::cmp::Reverse(entry.last_visit_ms));
            return entries
                .into_iter()
                .take(limit)
                .map(|entry| AddressSuggestion {
                    value: entry.url.clone(),
                    title: entry.title.clone(),
                    score: history_score(entry, 0),
                    source: SuggestionSource::History,
                })
                .collect();
        }

        let mut output = self
            .entries
            .values()
            .filter_map(|entry| {
                let url = entry.url.to_ascii_lowercase();
                let title = entry.title.to_ascii_lowercase();
                let rank = if url.starts_with(&needle) {
                    600
                } else if title.starts_with(&needle) {
                    520
                } else if url.contains(&needle) {
                    360
                } else if title.contains(&needle) {
                    300
                } else {
                    return None;
                };
                Some(AddressSuggestion {
                    value: entry.url.clone(),
                    title: entry.title.clone(),
                    score: history_score(entry, rank),
                    source: SuggestionSource::History,
                })
            })
            .collect::<Vec<_>>();
        output.sort_by_key(|item| std::cmp::Reverse(item.score));
        output.truncate(limit);
        output
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.persist_best_effort();
    }

    fn persist_best_effort(&self) {
        let Some(path) = self.file.as_deref() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut entries = self.entries.values().cloned().collect::<Vec<_>>();
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.last_visit_ms));
        let Ok(bytes) = serde_json::to_vec_pretty(&entries) else { return };
        let temp = path.with_extension("tmp");
        if std::fs::write(&temp, bytes).is_ok() {
            let _ = std::fs::rename(temp, path);
        }
    }
}

fn history_score(entry: &HistoryEntry, match_rank: i64) -> i64 {
    let visit_bonus = i64::try_from(entry.visits.min(100)).unwrap_or(100) * 8;
    let age_hours = now_ms().saturating_sub(entry.last_visit_ms) / 3_600_000;
    let recency = 300_i64.saturating_sub(i64::try_from(age_hours.min(300)).unwrap_or(300));
    match_rank + visit_bonus + recency
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
    fn prefix_matches_rank_above_contains() {
        let mut history = AddressHistory::default();
        history.record("https://nexus.example/path", "Nexus");
        history.record("https://example.com/nexus", "Other");
        let results = history.suggestions("nexus", 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].value, "https://nexus.example/path");
    }
}
