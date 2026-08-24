//! Multi-tab browser core for Nexus Engine 1.02.
//!
//! `BrowserCore` sits above `BrowserSession`. Tabs share profile-wide state
//! (cookies/cache/localStorage/permissions/HSTS), while each tab keeps its own
//! navigation history, scroll, sessionStorage, QuickJS realm and WebSockets.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::autocomplete::{AddressHistory, AddressSuggestion, SuggestionSource};
use crate::bookmarks::{Bookmark, BookmarkStore};
use crate::download::{DownloadItem, DownloadManager};
use crate::internal_pages::{self, InternalPage};
use crate::settings::{BrowserSettings, SettingsStore};
use crate::engine::NexusEngine;
use crate::error::{NexusError, NexusResult};
use crate::favicon::{fetch_favicon, FaviconData};
use crate::forms::FormControlDescriptor;
use crate::layout::Viewport;
use crate::network::{NetworkClient, DEFAULT_MAX_BODY_BYTES};
use crate::origin::Origin;
use crate::permissions::{PermissionKind, PermissionState};
use crate::session::{BrowserSession, InteractionResult, NavigationResult, SessionSnapshot};
use crate::selection::SelectionInfo;
use crate::state::BrowserState;
use crate::scheduler::{TabLifecycle, TabScheduler};

pub type TabId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabPrivacy {
    Normal,
    Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserDataKind {
    History,
    HttpCache,
    LocalStorage,
    Cookies,
    Permissions,
    Hsts,
    Downloads,
}

const SESSION_FILE: &str = "browser-session.json";
const DOWNLOAD_HISTORY_FILE: &str = "downloads.json";
const DOWNLOAD_DIRECTORY: &str = "downloads";

#[derive(Debug, Clone)]
pub struct BrowserCoreConfig {
    pub viewport: Viewport,
    pub profile_dir: Option<PathBuf>,
    pub max_tabs: usize,
    pub restore_on_start: bool,
}

impl Default for BrowserCoreConfig {
    fn default() -> Self {
        Self {
            viewport: Viewport::default(),
            profile_dir: None,
            max_tabs: 32,
            restore_on_start: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TabSummary {
    pub id: TabId,
    pub active: bool,
    pub title: String,
    pub url: Option<Url>,
    pub scroll_y: f32,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub favicon_url: Option<Url>,
    pub has_favicon: bool,
    pub privacy: TabPrivacy,
    pub lifecycle: TabLifecycle,
    pub pinned: bool,
    pub audible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressure { Moderate, Critical }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceReleaseReport {
    pub pressure: MemoryPressure,
    pub discarded_tabs: Vec<TabId>,
    pub released_bytes_estimate: usize,
}

struct BrowserTab {
    id: TabId,
    session: BrowserSession,
    favicon: Option<FaviconData>,
    privacy: TabPrivacy,
    pinned: bool,
    audible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedBrowserSession {
    version: u32,
    active_index: usize,
    tabs: Vec<PersistedTab>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedTab {
    url: Option<String>,
    scroll_y: f32,
}

pub struct BrowserCore {
    config: BrowserCoreConfig,
    state: Arc<BrowserState>,
    private_state: Arc<BrowserState>,
    network: NetworkClient,
    private_network: NetworkClient,
    tabs: Vec<BrowserTab>,
    active_index: Option<usize>,
    next_tab_id: TabId,
    history: AddressHistory,
    bookmarks: BookmarkStore,
    downloads: DownloadManager,
    private_downloads: DownloadManager,
    scheduler: TabScheduler,
    settings: SettingsStore,
}

impl BrowserCore {
    pub fn new(config: BrowserCoreConfig) -> NexusResult<Self> {
        let settings = SettingsStore::new(config.profile_dir.as_deref());
        let state = BrowserState::new(config.profile_dir.clone());
        let private_state = BrowserState::new(None);
        let network = NetworkClient::with_state(DEFAULT_MAX_BODY_BYTES, Arc::clone(&state))?;
        let private_network = NetworkClient::with_state(DEFAULT_MAX_BODY_BYTES, Arc::clone(&private_state))?;
        let history = AddressHistory::new(config.profile_dir.as_deref());
        let bookmarks = BookmarkStore::new(config.profile_dir.as_deref());
        let download_directory = config
            .profile_dir
            .as_deref()
            .map_or_else(|| PathBuf::from(DOWNLOAD_DIRECTORY), |dir| dir.join(DOWNLOAD_DIRECTORY));
        let download_history = config
            .profile_dir
            .as_deref()
            .map(|dir| dir.join(DOWNLOAD_HISTORY_FILE));
        let downloads = DownloadManager::new(network.clone(), download_directory.clone(), download_history);
        let private_downloads = DownloadManager::new(private_network.clone(), download_directory, None);
        let restore_on_start = config.restore_on_start && settings.get().restore_session;
        let mut browser = Self {
            config,
            state,
            private_state,
            network,
            private_network,
            tabs: Vec::new(),
            active_index: None,
            next_tab_id: 1,
            history,
            bookmarks,
            downloads,
            private_downloads,
            scheduler: TabScheduler::default(),
            settings,
        };
        if restore_on_start {
            let _ = browser.restore_session();
        }
        if browser.tabs.is_empty() {
            let _ = browser.new_tab(None, true)?;
        }
        Ok(browser)
    }

    #[must_use]
    pub fn browser_state(&self) -> Arc<BrowserState> {
        Arc::clone(&self.state)
    }

    #[must_use]
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    #[must_use]
    pub fn active_tab_id(&self) -> Option<TabId> {
        self.active_index.and_then(|index| self.tabs.get(index)).map(|tab| tab.id)
    }

    pub fn new_tab(&mut self, url: Option<&str>, activate: bool) -> NexusResult<TabId> {
        self.new_tab_with_privacy(url, activate, TabPrivacy::Normal)
    }

    pub fn new_private_tab(&mut self, url: Option<&str>, activate: bool) -> NexusResult<TabId> {
        self.new_tab_with_privacy(url, activate, TabPrivacy::Private)
    }

    fn new_tab_with_privacy(&mut self, url: Option<&str>, activate: bool, privacy: TabPrivacy) -> NexusResult<TabId> {
        if self.tabs.len() >= self.config.max_tabs.max(1) {
            return Err(NexusError::InvalidInput(format!(
                "maximum number of tabs reached ({})",
                self.config.max_tabs.max(1)
            )));
        }
        let engine = self.make_engine(privacy)?;
        let id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.saturating_add(1).max(1);
        let mut session = BrowserSession::new(engine);
        let default_zoom = f32::from(self.settings.get().default_zoom_percent) / 100.0;
        let _ = session.set_zoom(default_zoom, 0.0, 0.0);
        let mut tab = BrowserTab {
            id,
            session,
            favicon: None,
            privacy,
            pinned: false,
            audible: false,
        };
        if let Some(url) = url.filter(|value| !value.trim().is_empty()) {
            tab.session.navigate(url)?;
            self.refresh_favicon_for_tab(&mut tab);
            if privacy == TabPrivacy::Normal {
                record_session_page(&mut self.history, &tab.session);
            }
        }
        self.tabs.push(tab);
        let should_activate = activate || self.active_index.is_none();
        if should_activate {
            self.active_index = Some(self.tabs.len() - 1);
        }
        self.scheduler.register(id, should_activate);
        if should_activate {
            self.scheduler.activate(id);
        }
        self.save_session_best_effort();
        Ok(id)
    }

    pub fn close_tab(&mut self, id: TabId) -> NexusResult<()> {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return Err(NexusError::InvalidInput(format!("unknown tab id {id}")));
        };
        let closed = self.tabs.remove(index);
        self.scheduler.remove(closed.id);
        self.active_index = match (self.active_index, self.tabs.is_empty()) {
            (_, true) => None,
            (Some(active), false) if active > index => Some(active - 1),
            (Some(active), false) if active == index => Some(index.min(self.tabs.len() - 1)),
            (value, false) => value,
        };
        if self.tabs.is_empty() {
            let _ = self.new_tab(None, true)?;
        }
        if closed.privacy == TabPrivacy::Private && !self.tabs.iter().any(|tab| tab.privacy == TabPrivacy::Private) {
            self.reset_private_context()?;
        }
        if let Some(active) = self.active_tab_id() { self.scheduler.activate(active); }
        self.save_session_best_effort();
        Ok(())
    }

    pub fn switch_tab(&mut self, id: TabId) -> NexusResult<()> {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return Err(NexusError::InvalidInput(format!("unknown tab id {id}")));
        };
        if self.tabs[index].session.is_discarded() {
            let _ = self.tabs[index].session.restore_discarded()?;
        }
        self.active_index = Some(index);
        self.scheduler.activate(id);
        self.save_session_best_effort();
        Ok(())
    }

    #[must_use]
    pub fn tab_summaries(&self) -> Vec<TabSummary> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                let snapshot = tab.session.snapshot();
                TabSummary {
                    id: tab.id,
                    active: self.active_index == Some(index),
                    title: snapshot
                        .title
                        .clone()
                        .or_else(|| snapshot.url.as_ref().map(|url| url.as_str().to_owned()))
                        .unwrap_or_else(|| "New Tab".to_owned()),
                    url: snapshot.url,
                    scroll_y: snapshot.scroll_y,
                    can_go_back: snapshot.can_go_back,
                    can_go_forward: snapshot.can_go_forward,
                    favicon_url: tab.favicon.as_ref().map(|value| value.url.clone()),
                    has_favicon: tab.favicon.is_some(),
                    privacy: tab.privacy,
                    lifecycle: self.scheduler.lifecycle(tab.id),
                    pinned: tab.pinned,
                    audible: tab.audible,
                }
            })
            .collect()
    }

    pub fn set_tab_pinned(&mut self, id: TabId, pinned: bool) -> NexusResult<()> {
        let tab = self.tabs.iter_mut().find(|tab| tab.id == id)
            .ok_or_else(|| NexusError::InvalidInput(format!("unknown tab id {id}")))?;
        tab.pinned = pinned;
        Ok(())
    }

    pub fn set_tab_audible(&mut self, id: TabId, audible: bool) -> NexusResult<()> {
        let tab = self.tabs.iter_mut().find(|tab| tab.id == id)
            .ok_or_else(|| NexusError::InvalidInput(format!("unknown tab id {id}")))?;
        tab.audible = audible;
        Ok(())
    }

    pub fn handle_memory_pressure(&mut self, pressure: MemoryPressure) -> ResourceReleaseReport {
        let mut protected = self.tabs.iter()
            .filter(|tab| tab.pinned || tab.audible || tab.session.current_page().is_none())
            .map(|tab| tab.id)
            .collect::<Vec<_>>();
        if let Some(active) = self.active_tab_id() { protected.push(active); }
        let maximum = if pressure == MemoryPressure::Moderate { 1 } else { usize::MAX };
        let candidates = self.scheduler.discard_inactive_excluding(maximum, &protected);
        let mut discarded_tabs = Vec::new();
        let mut released_bytes_estimate = 0usize;
        for id in candidates {
            if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
                if let Some(state) = tab.session.discard_page_resources() {
                    released_bytes_estimate = released_bytes_estimate.saturating_add(state.released_bytes_estimate);
                    discarded_tabs.push(id);
                }
            }
        }
        ResourceReleaseReport { pressure, discarded_tabs, released_bytes_estimate }
    }

    pub fn navigate_active(&mut self, input: &str) -> NexusResult<NavigationResult> {
        let result = self.active_session_mut()?.navigate(input)?;
        self.after_active_navigation();
        Ok(result)
    }

    pub fn reload_active(&mut self) -> NexusResult<Option<NavigationResult>> {
        let result = self.active_session_mut()?.reload()?;
        if result.is_some() {
            self.after_active_navigation();
        }
        Ok(result)
    }

    pub fn go_back_active(&mut self) -> NexusResult<Option<NavigationResult>> {
        let result = self.active_session_mut()?.go_back()?;
        if result.is_some() {
            self.after_active_navigation();
        }
        Ok(result)
    }

    pub fn go_forward_active(&mut self) -> NexusResult<Option<NavigationResult>> {
        let result = self.active_session_mut()?.go_forward()?;
        if result.is_some() {
            self.after_active_navigation();
        }
        Ok(result)
    }

    pub fn interact_active(&mut self, x: f32, y: f32) -> NexusResult<InteractionResult> {
        let result = self.active_session_mut()?.interact_at(x, y)?;
        if result.navigation.is_some() {
            self.after_active_navigation();
        }
        Ok(result)
    }

    pub fn tick_active(&mut self) -> NexusResult<InteractionResult> {
        self.scheduler.refresh();
        let result = self.active_session_mut()?.tick()?;
        if result.navigation.is_some() {
            self.after_active_navigation();
        }
        Ok(result)
    }

    pub fn render_active_png(&self) -> NexusResult<Option<Vec<u8>>> {
        self.active_session()?.render_png()
    }

    pub fn scroll_active_by(&mut self, delta_y: f32) -> NexusResult<f32> {
        let value = self.active_session_mut()?.scroll_by(delta_y);
        self.save_session_best_effort();
        Ok(value)
    }

    /// Scroll delta expressed in physical viewport pixels (Android touch units).
    pub fn scroll_active_by_pixels(&mut self, delta_y: f32) -> NexusResult<f32> {
        let zoom = self.active_session()?.zoom_factor();
        let value = self.active_session_mut()?.scroll_by(delta_y / zoom.max(0.01));
        self.save_session_best_effort();
        Ok(value)
    }

    pub fn set_active_zoom(&mut self, zoom: f32, focal_x: f32, focal_y: f32) -> NexusResult<f32> {
        let value = self.active_session_mut()?.set_zoom(zoom, focal_x, focal_y)?;
        self.save_session_best_effort();
        Ok(value)
    }

    pub fn select_active_at(&mut self, x: f32, y: f32) -> NexusResult<Option<SelectionInfo>> {
        Ok(self.active_session_mut()?.select_at(x, y))
    }

    pub fn clear_active_selection(&mut self) -> NexusResult<()> {
        self.active_session_mut()?.clear_selection();
        Ok(())
    }

    pub fn set_active_input_value(&mut self, value: &str) -> NexusResult<InteractionResult> {
        self.active_session_mut()?.set_focused_input_value(value)
    }

    pub fn active_focused_control(&self) -> NexusResult<Option<FormControlDescriptor>> {
        Ok(self.active_session()?.focused_control())
    }

    pub fn set_active_checked(&mut self, checked: bool) -> NexusResult<InteractionResult> {
        self.active_session_mut()?.set_focused_checked(checked)
    }

    pub fn set_active_select_indices(&mut self, indices: &[usize]) -> NexusResult<InteractionResult> {
        self.active_session_mut()?.set_focused_select_indices(indices)
    }

    pub fn add_active_file(&mut self, path: PathBuf, name: String, mime_type: String, append: bool) -> NexusResult<InteractionResult> {
        self.active_session_mut()?.add_focused_file(path, name, mime_type, append)
    }

    pub fn clear_active_files(&mut self) -> NexusResult<InteractionResult> {
        self.active_session_mut()?.clear_focused_files()
    }

    pub fn submit_active_form(&mut self) -> NexusResult<InteractionResult> {
        let result = self.active_session_mut()?.submit_focused_form()?;
        if result.navigation.is_some() {
            self.after_active_navigation();
        }
        Ok(result)
    }

    pub fn active_snapshot(&self) -> NexusResult<SessionSnapshot> {
        Ok(self.active_session()?.snapshot())
    }

    #[must_use]
    pub fn active_favicon_png(&self) -> Option<&[u8]> {
        let index = self.active_index?;
        self.tabs.get(index)?.favicon.as_ref().map(|favicon| favicon.png.as_slice())
    }

    pub fn suggestions(&self, query: &str, limit: usize) -> Vec<AddressSuggestion> {
        let mut suggestions = Vec::new();
        let needle = query.trim().to_ascii_lowercase();
        let active_privacy = self.active_privacy();
        for item in self.bookmarks.matching(query, limit.saturating_mul(2)) {
            suggestions.push(AddressSuggestion {
                value: item.url, title: item.title, score: 3_000, source: SuggestionSource::Bookmark,
            });
        }
        for tab in &self.tabs {
            if tab.privacy != active_privacy { continue; }
            let snapshot = tab.session.snapshot();
            let Some(url) = snapshot.url else { continue };
            let title = snapshot.title.unwrap_or_else(|| url.as_str().to_owned());
            let hay_url = url.as_str().to_ascii_lowercase();
            let hay_title = title.to_ascii_lowercase();
            if needle.is_empty() || hay_url.contains(&needle) || hay_title.contains(&needle) {
                suggestions.push(AddressSuggestion {
                    value: url.as_str().to_owned(),
                    title,
                    score: 2_000 + if hay_url.starts_with(&needle) { 400 } else { 0 },
                    source: SuggestionSource::OpenTab,
                });
            }
        }
        if active_privacy == TabPrivacy::Normal {
            suggestions.extend(self.history.suggestions(query, limit.saturating_mul(2)));
        }
        if !query.trim().is_empty() && !suggestions.iter().any(|item| item.value == query.trim()) {
            suggestions.push(AddressSuggestion {
                value: query.trim().to_owned(),
                title: "Open input".to_owned(),
                score: 100,
                source: SuggestionSource::Direct,
            });
        }
        suggestions.sort_by_key(|item| std::cmp::Reverse(item.score));
        let mut seen = std::collections::HashSet::new();
        suggestions.retain(|item| seen.insert(item.value.clone()));
        suggestions.truncate(limit);
        suggestions
    }

    pub fn download_url(&mut self, url: &Url, suggested_name: Option<&str>) -> NexusResult<DownloadItem> {
        self.downloads.download(url, suggested_name)
    }

    pub fn download_active_page(&mut self) -> NexusResult<DownloadItem> {
        let url = self
            .active_session()?
            .current_page()
            .map(|page| page.final_url.clone())
            .ok_or_else(|| NexusError::InvalidInput("active tab has no page".to_owned()))?;
        if self.active_privacy() == TabPrivacy::Private {
            self.private_downloads.download(&url, None)
        } else {
            self.downloads.download(&url, None)
        }
    }

    #[must_use]
    pub fn downloads(&self) -> &[DownloadItem] {
        self.downloads.items()
    }

    pub fn active_permission(&self, kind: PermissionKind) -> NexusResult<PermissionState> {
        let origin = self.active_origin()?;
        Ok(self.active_browser_state().permission(&origin, kind))
    }

    pub fn set_active_permission(&self, kind: PermissionKind, value: PermissionState) -> NexusResult<()> {
        let origin = self.active_origin()?;
        self.active_browser_state().set_permission(&origin, kind, value);
        Ok(())
    }

    #[must_use]
    pub fn active_privacy(&self) -> TabPrivacy {
        self.active_index
            .and_then(|index| self.tabs.get(index))
            .map_or(TabPrivacy::Normal, |tab| tab.privacy)
    }

    #[must_use]
    pub fn active_is_private(&self) -> bool {
        self.active_privacy() == TabPrivacy::Private
    }

    pub fn active_is_bookmarked(&self) -> NexusResult<bool> {
        let url = self.active_session()?.current_page().map(|page| &page.final_url);
        Ok(url.is_some_and(|url| self.bookmarks.contains(url)))
    }

    pub fn toggle_active_bookmark(&mut self) -> NexusResult<bool> {
        let page = self.active_session()?.current_page()
            .ok_or_else(|| NexusError::InvalidInput("active tab has no page".to_owned()))?;
        let url = page.final_url.clone();
        let title = page.dom.title().unwrap_or_else(|| url.as_str().to_owned());
        self.bookmarks.toggle(&url, &title)
    }

    #[must_use]
    pub fn bookmarks(&self) -> &[Bookmark] {
        self.bookmarks.items()
    }

    #[must_use]
    pub fn new_tab_suggestions(&self, limit: usize) -> Vec<AddressSuggestion> {
        let mut output = self.bookmarks.matching("", limit).into_iter().map(|item| AddressSuggestion {
            value: item.url, title: item.title, score: 3_000, source: SuggestionSource::Bookmark,
        }).collect::<Vec<_>>();
        if self.active_privacy() == TabPrivacy::Normal && output.len() < limit {
            output.extend(self.history.suggestions("", limit - output.len()));
        }
        output.truncate(limit);
        output
    }

    #[must_use]
    pub fn settings(&self) -> &BrowserSettings {
        self.settings.get()
    }

    pub fn update_setting(&mut self, key: &str, value: &str) -> NexusResult<()> {
        self.settings.update(key, value)
    }

    pub fn show_internal_page(&mut self, page: InternalPage) -> NexusResult<NavigationResult> {
        let html = match page {
            InternalPage::History => {
                let items = if self.active_is_private() { Vec::new() } else { self.history.suggestions("", 100) };
                internal_pages::history_html(&items)
            }
            InternalPage::Bookmarks => internal_pages::bookmarks_html(self.bookmarks.items()),
            InternalPage::Downloads => {
                if self.active_is_private() { internal_pages::downloads_html(&[]) } else { internal_pages::downloads_html(self.downloads.items()) }
            }
            InternalPage::Settings => internal_pages::settings_html(self.settings.get()),
            InternalPage::Privacy => {
                let snapshot = self.active_snapshot()?;
                let state = self.active_browser_state();
                internal_pages::privacy_html(&state, snapshot.origin.as_deref().unwrap_or("opaque"), snapshot.csp_active)
            }
        };
        let url = Url::parse(page.url())?;
        let result = self.active_session_mut()?.show_internal_document(&url, html)?;
        self.save_session_best_effort();
        Ok(result)
    }

    pub fn show_error_page(&mut self, input: &str, message: &str) -> NexusResult<NavigationResult> {
        let html = internal_pages::error_html(input, message, self.settings.get().offline_error_pages);
        let url = Url::parse("nexus://error/")?;
        let result = self.active_session_mut()?.show_internal_document(&url, html)?;
        if let Some(index) = self.active_index {
            if let Some(tab) = self.tabs.get_mut(index) { tab.favicon = None; }
        }
        Ok(result)
    }

    pub fn clear_browser_data(&mut self, kind: BrowserDataKind) {
        let privacy = self.active_privacy();
        let state = self.active_browser_state();
        match kind {
            BrowserDataKind::History => { if privacy == TabPrivacy::Normal { self.history.clear(); } },
            BrowserDataKind::HttpCache => state.clear_http_cache(),
            BrowserDataKind::LocalStorage => state.clear_local_storage_all(),
            BrowserDataKind::Cookies => state.clear_cookies_all(),
            BrowserDataKind::Permissions => state.clear_permissions_all(),
            BrowserDataKind::Hsts => state.clear_hsts_all(),
            BrowserDataKind::Downloads => {
                if privacy == TabPrivacy::Private { self.private_downloads.clear_history(); } else { self.downloads.clear_history(); }
            }
        }
    }

    fn active_browser_state(&self) -> Arc<BrowserState> {
        match self.active_privacy() {
            TabPrivacy::Normal => Arc::clone(&self.state),
            TabPrivacy::Private => Arc::clone(&self.private_state),
        }
    }

    fn reset_private_context(&mut self) -> NexusResult<()> {
        let state = BrowserState::new(None);
        let network = NetworkClient::with_state(DEFAULT_MAX_BODY_BYTES, Arc::clone(&state))?;
        let directory = self.config.profile_dir.as_deref().map_or_else(|| PathBuf::from(DOWNLOAD_DIRECTORY), |dir| dir.join(DOWNLOAD_DIRECTORY));
        self.private_downloads = DownloadManager::new(network.clone(), directory, None);
        self.private_state = state;
        self.private_network = network;
        Ok(())
    }

    pub fn save_session(&self) -> NexusResult<()> {
        let Some(path) = self.session_file() else { return Ok(()) };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let normal_tabs = self.tabs.iter().enumerate().filter(|(_, tab)| tab.privacy == TabPrivacy::Normal).collect::<Vec<_>>();
        let tabs = normal_tabs
            .iter()
            .map(|(_, tab)| {
                let snapshot = tab.session.snapshot();
                PersistedTab {
                    url: snapshot.url.and_then(|url| (url.scheme() != "nexus").then(|| url.as_str().to_owned())),
                    scroll_y: snapshot.scroll_y,
                }
            })
            .collect();
        let active_index = self.active_index
            .and_then(|active| normal_tabs.iter().position(|(index, _)| *index == active))
            .unwrap_or(0);
        let data = PersistedBrowserSession {
            version: 2,
            active_index,
            tabs,
        };
        let bytes = serde_json::to_vec_pretty(&data)
            .map_err(|error| NexusError::Storage(format!("cannot serialize browser session: {error}")))?;
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, bytes)?;
        std::fs::rename(temp, path)?;
        Ok(())
    }

    pub fn restore_session(&mut self) -> NexusResult<usize> {
        let Some(path) = self.session_file() else { return Ok(0) };
        let Ok(bytes) = std::fs::read(path) else { return Ok(0) };
        let persisted = serde_json::from_slice::<PersistedBrowserSession>(&bytes)
            .map_err(|error| NexusError::Storage(format!("cannot parse browser session: {error}")))?;
        if persisted.version != 1 && persisted.version != 2 {
            return Ok(0);
        }
        self.tabs.clear();
        self.scheduler = TabScheduler::default();
        self.active_index = None;
        for tab in persisted.tabs.into_iter().take(self.config.max_tabs.max(1)) {
            let id = match self.new_tab(tab.url.as_deref(), false) {
                Ok(id) => id,
                Err(_) => self.new_tab(None, false)?,
            };
            if let Some(index) = self.tabs.iter().position(|candidate| candidate.id == id) {
                self.tabs[index].session.scroll_to(tab.scroll_y);
            }
        }
        if !self.tabs.is_empty() {
            self.active_index = Some(persisted.active_index.min(self.tabs.len() - 1));
            if let Some(active) = self.active_tab_id() { self.scheduler.activate(active); }
        }
        self.save_session_best_effort();
        Ok(self.tabs.len())
    }

    fn make_engine(&self, privacy: TabPrivacy) -> NexusResult<NexusEngine> {
        NexusEngine::builder()
            .viewport(self.config.viewport.width, self.config.viewport.height)
            .browser_state(match privacy {
                TabPrivacy::Normal => Arc::clone(&self.state),
                TabPrivacy::Private => Arc::clone(&self.private_state),
            })
            .javascript_enabled(self.settings.get().javascript_enabled)
            .build()
    }

    fn active_session(&self) -> NexusResult<&BrowserSession> {
        let index = self
            .active_index
            .ok_or_else(|| NexusError::InvalidInput("no active tab".to_owned()))?;
        self.tabs
            .get(index)
            .map(|tab| &tab.session)
            .ok_or_else(|| NexusError::InvalidInput("active tab index is invalid".to_owned()))
    }

    fn active_session_mut(&mut self) -> NexusResult<&mut BrowserSession> {
        let index = self
            .active_index
            .ok_or_else(|| NexusError::InvalidInput("no active tab".to_owned()))?;
        self.tabs
            .get_mut(index)
            .map(|tab| &mut tab.session)
            .ok_or_else(|| NexusError::InvalidInput("active tab index is invalid".to_owned()))
    }

    fn active_origin(&self) -> NexusResult<Origin> {
        let page = self
            .active_session()?
            .current_page()
            .ok_or_else(|| NexusError::InvalidInput("active tab has no origin".to_owned()))?;
        Ok(Origin::from_url(&page.final_url))
    }

    fn after_active_navigation(&mut self) {
        let Some(index) = self.active_index else { return };
        if let Some(tab) = self.tabs.get(index) {
            if tab.privacy == TabPrivacy::Normal {
                record_session_page(&mut self.history, &tab.session);
            }
        }
        self.refresh_active_favicon();
        self.save_session_best_effort();
    }

    fn refresh_active_favicon(&mut self) {
        let Some(index) = self.active_index else { return };
        let privacy = self.tabs.get(index).map(|tab| tab.privacy).unwrap_or(TabPrivacy::Normal);
        let network = match privacy { TabPrivacy::Normal => self.network.clone(), TabPrivacy::Private => self.private_network.clone() };
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.favicon = tab.session.current_page().and_then(|page| (page.final_url.scheme() != "nexus").then(|| fetch_favicon(&network, page)).flatten());
        }
    }

    fn refresh_favicon_for_tab(&self, tab: &mut BrowserTab) {
        let network = match tab.privacy { TabPrivacy::Normal => &self.network, TabPrivacy::Private => &self.private_network };
        tab.favicon = tab.session.current_page().and_then(|page| (page.final_url.scheme() != "nexus").then(|| fetch_favicon(network, page)).flatten());
    }

    fn session_file(&self) -> Option<PathBuf> {
        self.config.profile_dir.as_deref().map(|dir| dir.join(SESSION_FILE))
    }

    fn save_session_best_effort(&self) {
        let _ = self.save_session();
    }
}

impl Drop for BrowserCore {
    fn drop(&mut self) {
        self.save_session_best_effort();
    }
}

fn record_session_page(history: &mut AddressHistory, session: &BrowserSession) {
    let Some(page) = session.current_page() else { return };
    if page.final_url.scheme() == "nexus" { return; }
    let title = page.dom.title().unwrap_or_else(|| page.final_url.as_str().to_owned());
    history.record(page.final_url.as_str(), &title);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_browser_starts_with_one_tab() {
        let config = BrowserCoreConfig {
            restore_on_start: false,
            ..BrowserCoreConfig::default()
        };
        let browser = BrowserCore::new(config).unwrap();
        assert_eq!(browser.tab_count(), 1);
        assert!(browser.active_tab_id().is_some());
    }

    #[test]
    fn can_open_switch_and_close_blank_tabs() {
        let config = BrowserCoreConfig {
            restore_on_start: false,
            ..BrowserCoreConfig::default()
        };
        let mut browser = BrowserCore::new(config).unwrap();
        let first = browser.active_tab_id().unwrap();
        let second = browser.new_tab(None, true).unwrap();
        assert_eq!(browser.active_tab_id(), Some(second));
        browser.switch_tab(first).unwrap();
        assert_eq!(browser.active_tab_id(), Some(first));
        browser.close_tab(first).unwrap();
        assert_eq!(browser.tab_count(), 1);
        assert_eq!(browser.active_tab_id(), Some(second));
    }
}
