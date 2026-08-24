//! Persistent browser-level settings for Nexus Engine 1.02.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{NexusError, NexusResult};

const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserSettings {
    pub javascript_enabled: bool,
    pub restore_session: bool,
    pub offline_error_pages: bool,
    pub privacy_dashboard: bool,
    pub default_zoom_percent: u16,
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            javascript_enabled: true,
            restore_session: true,
            offline_error_pages: true,
            privacy_dashboard: true,
            default_zoom_percent: 100,
        }
    }
}

#[derive(Debug)]
pub struct SettingsStore {
    value: BrowserSettings,
    file: Option<PathBuf>,
}

impl SettingsStore {
    #[must_use]
    pub fn new(profile_dir: Option<&Path>) -> Self {
        let file = profile_dir.map(|dir| dir.join(SETTINGS_FILE));
        let value = file
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<BrowserSettings>(&bytes).ok())
            .unwrap_or_default();
        Self { value, file }
    }

    #[must_use]
    pub fn get(&self) -> &BrowserSettings { &self.value }

    pub fn update(&mut self, key: &str, value: &str) -> NexusResult<()> {
        match key {
            "javascript_enabled" => self.value.javascript_enabled = parse_bool(value)?,
            "restore_session" => self.value.restore_session = parse_bool(value)?,
            "offline_error_pages" => self.value.offline_error_pages = parse_bool(value)?,
            "privacy_dashboard" => self.value.privacy_dashboard = parse_bool(value)?,
            "default_zoom_percent" => {
                let parsed = value.parse::<u16>().map_err(|_| NexusError::InvalidInput("invalid zoom percent".to_owned()))?;
                self.value.default_zoom_percent = parsed.clamp(75, 300);
            }
            _ => return Err(NexusError::InvalidInput(format!("unknown setting: {key}"))),
        }
        self.persist()
    }

    fn persist(&self) -> NexusResult<()> {
        let Some(path) = self.file.as_deref() else { return Ok(()) };
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
        let bytes = serde_json::to_vec_pretty(&self.value)
            .map_err(|error| NexusError::Storage(format!("cannot serialize settings: {error}")))?;
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, bytes)?;
        std::fs::rename(temp, path)?;
        Ok(())
    }
}

fn parse_bool(value: &str) -> NexusResult<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "on" | "yes" => Ok(true),
        "0" | "false" | "off" | "no" => Ok(false),
        _ => Err(NexusError::InvalidInput(format!("invalid boolean: {value}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_default_zoom() {
        let mut store = SettingsStore::new(None);
        store.update("default_zoom_percent", "999").unwrap();
        assert_eq!(store.get().default_zoom_percent, 300);
    }
}
