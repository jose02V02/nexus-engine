//! Persistent browser session for Nexus Engine 1.02.
//!
//! Owns the current document, a live QuickJS realm, back/forward history,
//! focus and scroll state. The realm is intentionally kept on the same thread
//! as the session so JavaScript globals/event handlers/timers persist safely.

use std::collections::HashMap;
use std::path::PathBuf;

use url::Url;

use crate::address::resolve_url;
use crate::dom::{NexusDom, NodeId};
use crate::engine::{LoadedPage, NexusEngine};
use crate::event_loop::BrowserEventLoop;
use crate::error::NexusResult;
use crate::hit_test::{hit_test_page, HitTestResult};
use crate::forms::{describe_control, selected_option_values, set_select_indices, FormControlDescriptor, SelectedFile};
use crate::layout::Viewport;
use crate::selection::{selection_at, SelectionInfo};
use crate::javascript::{JavascriptReport, QuickJsRealm};
use crate::storage::SessionStorage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationKind {
    New,
    Link,
    Script,
    Form,
    Reload,
    Back,
    Forward,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    pub url: Url,
    pub title: String,
    pub scroll_y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NavigationResult {
    pub kind: NavigationKind,
    pub final_url: Url,
    pub title: String,
    pub status: u16,
    pub history_index: usize,
    pub history_len: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InteractionResult {
    pub dirty: bool,
    pub default_prevented: bool,
    pub focused_node: Option<NodeId>,
    pub navigation: Option<NavigationResult>,
}

impl Default for InteractionResult {
    fn default() -> Self {
        Self {
            dirty: false,
            default_prevented: false,
            focused_node: None,
            navigation: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionSnapshot {
    pub url: Option<Url>,
    pub origin: Option<String>,
    pub title: Option<String>,
    pub status: Option<u16>,
    pub from_http_cache: bool,
    pub cache_revalidated: bool,
    pub cookie_count: usize,
    pub http_cache_entries: usize,
    pub http_cache_bytes: usize,
    pub http_cache_hits: usize,
    pub http_cache_misses: usize,
    pub http_cache_revalidations: usize,
    pub local_storage_origins: usize,
    pub permission_entries: usize,
    pub hsts_entries: usize,
    pub csp_active: bool,
    pub websocket_commands: usize,
    pub websocket_events: usize,
    pub active_websockets_hint: usize,
    pub js_scripts_executed: usize,
    pub js_dom_mutations: usize,
    pub js_events_dispatched: usize,
    pub js_fetch_requests: usize,
    pub js_timers_executed: usize,
    pub js_warnings: usize,
    pub js_next_timer_ms: Option<u64>,
    pub scroll_y: f32,
    pub max_scroll_y: f32,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub history_index: Option<usize>,
    pub history_len: usize,
    pub focused_node: Option<NodeId>,
    pub focused_tag: Option<String>,
    pub focused_value: Option<String>,
    pub zoom_factor: f32,
    pub selected_text: Option<String>,
    pub discarded: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiscardedPageState {
    pub url: Url,
    pub title: String,
    pub scroll_y: f32,
    pub zoom_factor: f32,
    pub released_bytes_estimate: usize,
}

pub struct BrowserSession {
    engine: NexusEngine,
    current: Option<LoadedPage>,
    realm: Option<QuickJsRealm>,
    history: Vec<HistoryEntry>,
    history_index: Option<usize>,
    scroll_y: f32,
    focused_node: Option<NodeId>,
    base_viewport: Viewport,
    zoom_factor: f32,
    selection: Option<SelectionInfo>,
    file_inputs: HashMap<NodeId, Vec<SelectedFile>>,
    session_storage: SessionStorage,
    event_loop: Option<BrowserEventLoop>,
    internal_documents: HashMap<String, String>,
    discarded: Option<DiscardedPageState>,
}

impl BrowserSession {
    #[must_use]
    pub fn new(engine: NexusEngine) -> Self {
        let base_viewport = engine.viewport();
        Self {
            engine,
            current: None,
            realm: None,
            history: Vec::new(),
            history_index: None,
            scroll_y: 0.0,
            focused_node: None,
            base_viewport,
            zoom_factor: 1.0,
            selection: None,
            file_inputs: HashMap::new(),
            session_storage: SessionStorage::default(),
            event_loop: BrowserEventLoop::new().ok(),
            internal_documents: HashMap::new(),
            discarded: None,
        }
    }

    pub fn navigate(&mut self, input: &str) -> NexusResult<NavigationResult> {
        let url = crate::address::normalize_url(input)?;
        self.navigate_url(&url, NavigationKind::New)
    }

    pub fn navigate_url(&mut self, url: &Url, kind: NavigationKind) -> NexusResult<NavigationResult> {
        if url.scheme() == "nexus" {
            let html = self.internal_documents.get(url.as_str())
                .cloned()
                .ok_or_else(|| crate::error::NexusError::InvalidInput(format!("unknown internal page: {url}")))?;
            let page = self.engine.load_internal_html(url, &html)?;
            return self.commit_new_page(page, None, kind);
        }
        let (page, realm) = self.load_with_script_navigation(url)?;
        self.commit_new_page(page, realm, kind)
    }

    pub fn show_internal_document(&mut self, url: &Url, html: String) -> NexusResult<NavigationResult> {
        self.internal_documents.insert(url.as_str().to_owned(), html.clone());
        let page = self.engine.load_internal_html(url, &html)?;
        self.commit_new_page(page, None, NavigationKind::New)
    }

    pub fn reload(&mut self) -> NexusResult<Option<NavigationResult>> {
        let Some(url) = self.current.as_ref().map(|page| page.final_url.clone()) else {
            return Ok(None);
        };
        self.persist_scroll();
        let (page, realm) = if url.scheme() == "nexus" {
            let html = self.internal_documents.get(url.as_str())
                .cloned()
                .ok_or_else(|| crate::error::NexusError::InvalidInput(format!("unknown internal page: {url}")))?;
            (self.engine.load_internal_html(&url, &html)?, None)
        } else {
            self.load_with_script_navigation(&url)?
        };
        self.scroll_y = self
            .history_index
            .and_then(|index| self.history.get(index))
            .map_or(0.0, |entry| entry.scroll_y);
        if let Some(event_loop) = self.event_loop.as_mut() { event_loop.reset_document(); }
        self.current = Some(page);
        self.realm = realm;
        self.discarded = None;
        self.focused_node = None;
        self.selection = None;
        self.file_inputs.clear();
        self.clamp_scroll();
        self.refresh_current_history_metadata();
        Ok(Some(self.navigation_result(NavigationKind::Reload)))
    }

    pub fn go_back(&mut self) -> NexusResult<Option<NavigationResult>> {
        let Some(index) = self.history_index else { return Ok(None) };
        if index == 0 {
            return Ok(None);
        }
        self.navigate_history(index - 1, NavigationKind::Back).map(Some)
    }

    pub fn go_forward(&mut self) -> NexusResult<Option<NavigationResult>> {
        let Some(index) = self.history_index else { return Ok(None) };
        if index + 1 >= self.history.len() {
            return Ok(None);
        }
        self.navigate_history(index + 1, NavigationKind::Forward).map(Some)
    }

    /// Compatibility wrapper used by 0.5/0.6 callers. Nexus 0.7+'s richer
    /// interaction path lives in `interact_at`.
    pub fn activate_at(&mut self, x: f32, y: f32) -> NexusResult<Option<NavigationResult>> {
        Ok(self.interact_at(x, y)?.navigation)
    }

    pub fn interact_at(&mut self, x: f32, y: f32) -> NexusResult<InteractionResult> {
        self.selection = None;
        let Some(hit) = self.hit_test(x, y) else {
            self.focused_node = None;
            return Ok(InteractionResult::default());
        };
        let target = self
            .current
            .as_ref()
            .and_then(|page| page.dom.closest_element(hit.node_id));
        let Some(target) = target else {
            return Ok(InteractionResult::default());
        };

        let mut result = InteractionResult::default();
        if let Some(realm) = self.realm.as_mut() {
            let activity = realm.dispatch_click(target);
            result.dirty |= activity.dom_changed;
            result.default_prevented = activity.default_prevented;
        }
        if result.dirty {
            self.sync_realm_into_page()?;
        }
        if let Some(navigation) = self.follow_realm_navigation()? {
            result.navigation = Some(navigation);
            result.dirty = true;
            return Ok(result);
        }
        if result.default_prevented {
            return Ok(result);
        }

        let current_dom = self.current.as_ref().map(|page| &page.dom);
        let Some(dom) = current_dom else { return Ok(result) };
        let tag = dom.element_tag_name(target).unwrap_or("").to_ascii_lowercase();

        if matches!(tag.as_str(), "input" | "textarea" | "select") {
            let input_type = dom.attribute(target, "type").unwrap_or("text").to_ascii_lowercase();
            if tag == "input" && matches!(input_type.as_str(), "submit" | "image") {
                if let Some(form) = dom.closest_ancestor_tag(target, "form") {
                    return self.submit_form_from_interaction(form, result);
                }
            }
            let was_checked = dom.attribute(target, "checked").is_some();
            self.focused_node = Some(target);
            result.focused_node = Some(target);
            if tag == "input" && input_type == "checkbox" {
                let mut toggled = self.set_focused_checked(!was_checked)?;
                toggled.dirty |= result.dirty;
                toggled.default_prevented |= result.default_prevented;
                return Ok(toggled);
            }
            if tag == "input" && input_type == "radio" {
                let mut toggled = self.set_focused_checked(true)?;
                toggled.dirty |= result.dirty;
                toggled.default_prevented |= result.default_prevented;
                return Ok(toggled);
            }
            return Ok(result);
        }

        let button = if tag == "button" {
            Some(target)
        } else {
            dom.closest_ancestor_tag(target, "button")
        };
        if let Some(button) = button {
            let button_type = dom.attribute(button, "type").unwrap_or("submit").to_ascii_lowercase();
            if button_type == "submit" {
                if let Some(form) = dom.closest_ancestor_tag(button, "form") {
                    return self.submit_form_from_interaction(form, result);
                }
            }
        }

        let link_url = self.resolve_current_link(target).or(hit.link_url);
        if let Some(url) = link_url {
            let navigation = self.navigate_url(&url, NavigationKind::Link)?;
            result.navigation = Some(navigation);
            result.dirty = true;
        }
        Ok(result)
    }

    pub fn set_focused_input_value(&mut self, value: &str) -> NexusResult<InteractionResult> {
        let Some(node) = self.focused_node else {
            return Ok(InteractionResult::default());
        };
        let mut result = InteractionResult {
            focused_node: Some(node),
            ..InteractionResult::default()
        };
        if let Some(realm) = self.realm.as_mut() {
            let activity = realm.dispatch_input(node, value);
            result.dirty = activity.dom_changed;
        } else if let Some(page) = self.current.as_mut() {
            let tag = page.dom.element_tag_name(node).unwrap_or("").to_ascii_lowercase();
            let changed = if tag == "textarea" {
                page.dom.set_text_content(node, value)
            } else {
                page.dom.set_attribute(node, "value", value)
            };
            if changed {
                let dom = page.dom.clone();
                let report = page.javascript.clone();
                self.engine.rebuild_page(page, dom, report)?;
                result.dirty = true;
            }
        }
        if result.dirty {
            self.sync_realm_into_page()?;
        }
        if let Some(navigation) = self.follow_realm_navigation()? {
            result.navigation = Some(navigation);
            result.dirty = true;
        }
        Ok(result)
    }

    pub fn submit_focused_form(&mut self) -> NexusResult<InteractionResult> {
        let Some(node) = self.focused_node else {
            return Ok(InteractionResult::default());
        };
        let form = self
            .current
            .as_ref()
            .and_then(|page| page.dom.closest_ancestor_tag(node, "form"));
        let Some(form) = form else {
            return Ok(InteractionResult::default());
        };
        self.submit_form_from_interaction(form, InteractionResult::default())
    }

    /// Advances persistent timers/promises. Android calls this periodically;
    /// desktop embedders can drive it from their own event loop.
    pub fn tick(&mut self) -> NexusResult<InteractionResult> {
        let mut result = InteractionResult::default();
        if let Some(realm) = self.realm.as_mut() {
            let activity = realm.pump();
            result.dirty |= activity.dom_changed;

            if let Some(event_loop) = self.event_loop.as_mut() {
                for command in realm.take_websocket_commands() {
                    if let Err(error) = event_loop.submit_websocket(command) {
                        let event = crate::event_loop::WebSocketEvent::Error { id: 0, message: error };
                        let delivered = realm.deliver_websocket_event(&event);
                        result.dirty |= delivered.dom_changed;
                    }
                }
                for event in event_loop.drain_websocket_events(128) {
                    let delivered = realm.deliver_websocket_event(&event);
                    result.dirty |= delivered.dom_changed;
                }
            }
        }
        if result.dirty {
            self.sync_realm_into_page()?;
        }
        if let Some(navigation) = self.follow_realm_navigation()? {
            result.navigation = Some(navigation);
            result.dirty = true;
        }
        result.focused_node = self.focused_node;
        Ok(result)
    }

    #[must_use]
    pub fn hit_test(&self, x: f32, y: f32) -> Option<HitTestResult> {
        let page = self.current.as_ref()?;
        hit_test_page(page, x / self.zoom_factor, y / self.zoom_factor, self.scroll_y)
    }

    /// Selects the DOM/layout node under a viewport point. The current Alpha
    /// selection granularity is node-level, which is enough for long-press copy
    /// and context menus while keeping selection state engine-owned.
    pub fn select_at(&mut self, x: f32, y: f32) -> Option<SelectionInfo> {
        let page = self.current.as_ref()?;
        let selection = selection_at(
            page,
            x / self.zoom_factor,
            y / self.zoom_factor,
            self.scroll_y,
        )?;
        self.selection = Some(selection.clone());
        Some(selection)
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    #[must_use]
    pub fn selection(&self) -> Option<&SelectionInfo> {
        self.selection.as_ref()
    }

    /// Changes page zoom while preserving the document point underneath the
    /// gesture focal point. The physical Android surface size stays constant;
    /// Nexus changes the effective CSS viewport and performs a real reflow.
    pub fn set_zoom(&mut self, requested: f32, focal_x: f32, focal_y: f32) -> NexusResult<f32> {
        let next = requested.clamp(0.75, 3.0);
        if (next - self.zoom_factor).abs() < 0.002 {
            return Ok(self.zoom_factor);
        }

        let old = self.zoom_factor;
        let document_focus_y = self.scroll_y + focal_y.max(0.0) / old;
        self.zoom_factor = next;
        let viewport = Viewport {
            width: self.base_viewport.width / next,
            height: self.base_viewport.height / next,
        };
        self.engine.set_viewport(viewport);
        if let Some(realm) = self.realm.as_mut() {
            realm.set_viewport(viewport);
        }
        let dom = self.realm.as_ref().map(QuickJsRealm::dom_snapshot)
            .or_else(|| self.current.as_ref().map(|page| page.dom.clone()));
        let report = self.realm.as_ref().map(|realm| realm.report().clone())
            .or_else(|| self.current.as_ref().map(|page| page.javascript.clone()));
        if let (Some(page), Some(dom), Some(report)) = (self.current.as_mut(), dom, report) {
            self.engine.rebuild_page(page, dom, report)?;
        }
        self.scroll_y = document_focus_y - focal_y.max(0.0) / next;
        self.selection = None;
        self.clamp_scroll();
        self.persist_scroll();
        Ok(self.zoom_factor)
    }

    #[must_use]
    pub fn zoom_factor(&self) -> f32 {
        self.zoom_factor
    }

    pub fn scroll_to(&mut self, y: f32) -> f32 {
        self.scroll_y = y.max(0.0);
        self.clamp_scroll();
        self.persist_scroll();
        self.scroll_y
    }

    pub fn scroll_by(&mut self, delta_y: f32) -> f32 {
        self.scroll_to(self.scroll_y + delta_y)
    }

    pub fn render_png(&self) -> NexusResult<Option<Vec<u8>>> {
        let Some(page) = self.current.as_ref() else { return Ok(None) };
        let mut display = self.engine.display_list_at_scroll(page, self.scroll_y);
        if let Some(selection) = self.selection.as_ref() {
            let rect = crate::display_list::PaintRect {
                x: selection.rect.x,
                y: selection.rect.y - self.scroll_y,
                width: selection.rect.width,
                height: selection.rect.height,
            };
            if rect.intersects_viewport(page.layout.viewport.width, page.layout.viewport.height) {
                display.commands.push(crate::display_list::DisplayCommand::FillRoundedRect {
                    node_id: selection.node_id,
                    rect,
                    radius: 3.0,
                    color: crate::css::Rgba { r: 80, g: 145, b: 255, a: 72 },
                });
            }
        }
        let mut renderer = crate::renderer::SkiaRenderer::new();
        crate::renderer::Renderer::render_png(&mut renderer, &display).map(Some)
    }

    #[must_use]
    pub fn current_page(&self) -> Option<&LoadedPage> {
        self.current.as_ref()
    }

    #[must_use]
    pub fn is_discarded(&self) -> bool { self.discarded.is_some() }

    pub fn discard_page_resources(&mut self) -> Option<DiscardedPageState> {
        self.persist_scroll();
        let page = self.current.take()?;
        let state = DiscardedPageState {
            url: page.final_url.clone(),
            title: page.dom.title().unwrap_or_else(|| page.final_url.as_str().to_owned()),
            scroll_y: self.scroll_y,
            zoom_factor: self.zoom_factor,
            released_bytes_estimate: estimate_loaded_page_bytes(&page),
        };
        self.realm = None;
        self.event_loop = None;
        self.focused_node = None;
        self.selection = None;
        self.file_inputs.clear();
        self.discarded = Some(state.clone());
        Some(state)
    }

    pub fn restore_discarded(&mut self) -> NexusResult<Option<NavigationResult>> {
        let Some(state) = self.discarded.take() else { return Ok(None) };
        let Some(index) = self.history_index else { self.discarded = Some(state); return Ok(None) };
        self.event_loop = BrowserEventLoop::new().ok();
        match self.navigate_history(index, NavigationKind::Reload) {
            Ok(result) => {
                self.zoom_factor = state.zoom_factor;
                self.scroll_y = state.scroll_y;
                self.clamp_scroll();
                Ok(Some(result))
            }
            Err(error) => {
                self.event_loop = None;
                self.discarded = Some(state);
                Err(error)
            }
        }
    }

    #[must_use]
    pub fn history(&self) -> &[HistoryEntry] {
        &self.history
    }

    #[must_use]
    pub fn scroll_y(&self) -> f32 {
        self.scroll_y
    }

    #[must_use]
    pub fn max_scroll_y(&self) -> f32 {
        self.current.as_ref().map_or(0.0, LoadedPage::max_scroll_y)
    }

    #[must_use]
    pub fn focused_node(&self) -> Option<NodeId> {
        self.focused_node
    }

    #[must_use]
    pub fn focused_control(&self) -> Option<FormControlDescriptor> {
        let node = self.focused_node?;
        let page = self.current.as_ref()?;
        let mut descriptor = describe_control(&page.dom, node)?;
        if descriptor.input_type == "file" {
            if let Some(files) = self.file_inputs.get(&node) {
                descriptor.value = files.iter().map(|file| file.name.as_str()).collect::<Vec<_>>().join(", ");
            }
        }
        Some(descriptor)
    }

    pub fn set_focused_checked(&mut self, checked: bool) -> NexusResult<InteractionResult> {
        let Some(node) = self.focused_node else { return Ok(InteractionResult::default()) };
        let Some(page) = self.current.as_ref() else { return Ok(InteractionResult::default()) };
        let input_type = page.dom.attribute(node, "type").unwrap_or("text").to_ascii_lowercase();
        if !matches!(input_type.as_str(), "checkbox" | "radio") {
            return Ok(InteractionResult::default());
        }

        let mut result = InteractionResult { focused_node: Some(node), dirty: true, ..InteractionResult::default() };
        if input_type == "radio" && checked {
            self.uncheck_radio_group(node)?;
        }
        if let Some(realm) = self.realm.as_mut() {
            let activity = realm.dispatch_checked(node, checked);
            result.dirty |= activity.dom_changed;
        } else if let Some(page) = self.current.as_mut() {
            if checked { page.dom.set_attribute(node, "checked", ""); }
            else { page.dom.remove_attribute(node, "checked"); }
            let dom = page.dom.clone();
            let report = page.javascript.clone();
            self.engine.rebuild_page(page, dom, report)?;
        }
        if result.dirty { self.sync_realm_into_page()?; }
        Ok(result)
    }

    pub fn set_focused_select_indices(&mut self, indices: &[usize]) -> NexusResult<InteractionResult> {
        let Some(node) = self.focused_node else { return Ok(InteractionResult::default()) };
        let Some(page) = self.current.as_ref() else { return Ok(InteractionResult::default()) };
        if page.dom.element_tag_name(node).is_none_or(|tag| !tag.eq_ignore_ascii_case("select")) {
            return Ok(InteractionResult::default());
        }
        let mut dom = self.realm.as_ref().map(QuickJsRealm::dom_snapshot).unwrap_or_else(|| page.dom.clone());
        let changed = set_select_indices(&mut dom, node, indices);
        if !changed { return Ok(InteractionResult { focused_node: Some(node), ..InteractionResult::default() }); }
        let selected = selected_option_values(&dom, node).first().cloned().unwrap_or_default();
        let mut result = InteractionResult { focused_node: Some(node), dirty: true, ..InteractionResult::default() };
        if let Some(realm) = self.realm.as_mut() {
            realm.replace_dom(dom);
            let activity = realm.dispatch_input(node, &selected);
            result.dirty |= activity.dom_changed;
        } else if let Some(page) = self.current.as_mut() {
            page.dom = dom;
            let snapshot = page.dom.clone();
            let report = page.javascript.clone();
            self.engine.rebuild_page(page, snapshot, report)?;
        }
        if result.dirty { self.sync_realm_into_page()?; }
        Ok(result)
    }

    pub fn add_focused_file(&mut self, path: PathBuf, name: String, mime_type: String, append: bool) -> NexusResult<InteractionResult> {
        const MAX_UPLOAD_FILE_BYTES: u64 = 16 * 1024 * 1024;
        let Some(node) = self.focused_node else { return Ok(InteractionResult::default()) };
        let Some(page) = self.current.as_ref() else { return Ok(InteractionResult::default()) };
        if page.dom.attribute(node, "type").unwrap_or("").to_ascii_lowercase() != "file" {
            return Ok(InteractionResult::default());
        }
        let accept = page.dom.attribute(node, "accept").unwrap_or("").to_owned();
        if !file_matches_accept(&accept, &name, &mime_type) {
            return Err(crate::error::NexusError::InvalidInput(format!("selected file does not match accept={accept}")));
        }
        let metadata = std::fs::metadata(&path)?;
        if metadata.len() > MAX_UPLOAD_FILE_BYTES {
            return Err(crate::error::NexusError::BodyTooLarge { limit: MAX_UPLOAD_FILE_BYTES as usize, actual: metadata.len() as usize });
        }
        let multiple = page.dom.attribute(node, "multiple").is_some();
        let files = self.file_inputs.entry(node).or_default();
        if !append || !multiple { files.clear(); }
        if files.len() >= 8 {
            return Err(crate::error::NexusError::InvalidInput("Nexus Alpha allows at most 8 files per input".to_owned()));
        }
        files.push(SelectedFile { path, name: sanitize_upload_name(&name), mime_type, size: metadata.len() });
        let display = files.iter().map(|file| file.name.as_str()).collect::<Vec<_>>().join(", ");
        let result = self.set_focused_input_value(&display)?;
        Ok(result)
    }

    pub fn clear_focused_files(&mut self) -> NexusResult<InteractionResult> {
        if let Some(node) = self.focused_node { self.file_inputs.remove(&node); }
        self.set_focused_input_value("")
    }

    fn uncheck_radio_group(&mut self, target: NodeId) -> NexusResult<()> {
        let Some(page) = self.current.as_ref() else { return Ok(()) };
        let name = page.dom.attribute(target, "name").unwrap_or("").to_owned();
        if name.is_empty() { return Ok(()); }
        let form = page.dom.closest_ancestor_tag(target, "form");
        let candidates = if let Some(form) = form { page.dom.form_controls(form) } else { page.dom.reachable_ids() };
        let peers = candidates.into_iter().filter(|&node| {
            node != target
                && page.dom.element_tag_name(node).is_some_and(|tag| tag.eq_ignore_ascii_case("input"))
                && page.dom.attribute(node, "type").is_some_and(|kind| kind.eq_ignore_ascii_case("radio"))
                && page.dom.attribute(node, "name") == Some(name.as_str())
                && page.dom.attribute(node, "checked").is_some()
        }).collect::<Vec<_>>();
        if peers.is_empty() { return Ok(()); }
        if let Some(realm) = self.realm.as_mut() {
            for peer in peers { let _ = realm.dispatch_checked(peer, false); }
            self.sync_realm_into_page()?;
        } else if let Some(page) = self.current.as_mut() {
            for peer in peers { page.dom.remove_attribute(peer, "checked"); }
            let dom = page.dom.clone();
            let report = page.javascript.clone();
            self.engine.rebuild_page(page, dom, report)?;
        }
        Ok(())
    }

    #[must_use]
    pub fn snapshot(&self) -> SessionSnapshot {
        let page = self.current.as_ref();
        let discarded = self.discarded.as_ref();
        let index = self.history_index;
        let report = page.map(|value| &value.javascript);
        let focused_tag = self.focused_node.and_then(|node| {
            page.and_then(|value| value.dom.element_tag_name(node).map(str::to_owned))
        });
        let focused_value = self.focused_node.and_then(|node| {
            page.map(|value| control_value(&value.dom, node))
        });
        let browser_state = self.engine.browser_state();
        let cache = browser_state.http_cache_stats();
        let event_stats = self.event_loop.as_ref().map(BrowserEventLoop::stats).unwrap_or_default();
        SessionSnapshot {
            url: page.map(|value| value.final_url.clone()).or_else(|| discarded.map(|value| value.url.clone())),
            origin: page.map(|value| crate::origin::Origin::from_url(&value.final_url).serialize())
                .or_else(|| discarded.map(|value| crate::origin::Origin::from_url(&value.url).serialize())),
            title: page.and_then(|value| value.dom.title()).or_else(|| discarded.map(|value| value.title.clone())),
            status: page.map(|value| value.status),
            from_http_cache: page.is_some_and(|value| value.from_http_cache),
            cache_revalidated: page.is_some_and(|value| value.cache_revalidated),
            cookie_count: browser_state.cookie_count(),
            http_cache_entries: cache.entries,
            http_cache_bytes: cache.bytes,
            http_cache_hits: cache.hits,
            http_cache_misses: cache.misses,
            http_cache_revalidations: cache.revalidations,
            local_storage_origins: browser_state.local_origin_count(),
            permission_entries: browser_state.permission_count(),
            hsts_entries: browser_state.hsts_count(),
            csp_active: page.is_some_and(|value| !value.security.csp.is_empty()),
            websocket_commands: event_stats.websocket_commands,
            websocket_events: event_stats.websocket_events,
            active_websockets_hint: event_stats.active_websockets_hint,
            js_scripts_executed: report.map_or(0, |value| value.scripts_executed),
            js_dom_mutations: report.map_or(0, |value| value.dom_mutations),
            js_events_dispatched: report.map_or(0, |value| value.events_dispatched),
            js_fetch_requests: report.map_or(0, |value| value.fetch_requests),
            js_timers_executed: report.map_or(0, |value| value.timers_executed),
            js_warnings: report.map_or(0, |value| value.warnings.len()),
            js_next_timer_ms: self.realm.as_ref().and_then(QuickJsRealm::next_timer_delay_ms),
            scroll_y: self.scroll_y,
            max_scroll_y: self.max_scroll_y(),
            can_go_back: index.is_some_and(|value| value > 0),
            can_go_forward: index.is_some_and(|value| value + 1 < self.history.len()),
            history_index: index,
            history_len: self.history.len(),
            focused_node: self.focused_node,
            focused_tag,
            focused_value,
            zoom_factor: self.zoom_factor,
            selected_text: self.selection.as_ref().map(|value| value.text.clone()).filter(|value| !value.is_empty()),
            discarded: discarded.is_some(),
        }
    }

    fn load_with_script_navigation(&mut self, url: &Url) -> NexusResult<(LoadedPage, Option<QuickJsRealm>)> {
        const MAX_SCRIPT_NAVIGATIONS: usize = 5;
        let mut target = url.clone();
        for hop in 0..=MAX_SCRIPT_NAVIGATIONS {
            let mut page = self.engine.load_url_raw(&target)?;
            let mut realm = match self.engine.create_javascript_realm_with_storage(page.dom.clone(), self.session_storage.clone(), page.security.clone()) {
                Ok(value) => value,
                Err(error) => {
                    let mut report = JavascriptReport {
                        enabled: true,
                        runtime: "QuickJS-ng via quickjs-rusty 0.13".to_owned(),
                        persistent_realm: false,
                        ..JavascriptReport::default()
                    };
                    report.warnings.push(format!("QuickJS realm could not start: {error}"));
                    let dom = page.dom.clone();
                    self.engine.rebuild_page(&mut page, dom, report)?;
                    None
                }
            };

            if let Some(active) = realm.as_mut() {
                let dom = active.dom_snapshot();
                let report = active.report().clone();
                self.engine.rebuild_page(&mut page, dom, report)?;
                if let Some(next) = active.take_navigation_request() {
                    if next == page.final_url {
                        page.javascript.warnings.push(
                            "JavaScript requested navigation to the current URL; loop suppressed".to_owned(),
                        );
                        return Ok((page, realm));
                    }
                    if hop == MAX_SCRIPT_NAVIGATIONS {
                        page.javascript.warnings.push(format!(
                            "JavaScript navigation limit reached ({MAX_SCRIPT_NAVIGATIONS})"
                        ));
                        return Ok((page, realm));
                    }
                    target = next;
                    continue;
                }
            }
            return Ok((page, realm));
        }
        unreachable!("bounded script navigation loop always returns")
    }

    fn load_post_with_realm(&mut self, url: &Url, body: &str) -> NexusResult<(LoadedPage, Option<QuickJsRealm>)> {
        let mut page = self.engine.load_post_form_raw(url, body)?;
        let mut realm = self
            .engine
            .create_javascript_realm_with_storage(page.dom.clone(), self.session_storage.clone(), page.security.clone())
            .ok()
            .flatten();
        if let Some(active) = realm.as_mut() {
            let dom = active.dom_snapshot();
            let report = active.report().clone();
            self.engine.rebuild_page(&mut page, dom, report)?;
        }
        Ok((page, realm))
    }

    fn load_post_bytes_with_realm(&mut self, url: &Url, body: &[u8], content_type: &str) -> NexusResult<(LoadedPage, Option<QuickJsRealm>)> {
        let mut page = self.engine.load_post_raw(url, body, content_type)?;
        let mut realm = self.engine
            .create_javascript_realm_with_storage(page.dom.clone(), self.session_storage.clone(), page.security.clone())
            .ok().flatten();
        if let Some(active) = realm.as_mut() {
            let dom = active.dom_snapshot();
            let report = active.report().clone();
            self.engine.rebuild_page(&mut page, dom, report)?;
        }
        Ok((page, realm))
    }

    fn commit_new_page(
        &mut self,
        page: LoadedPage,
        realm: Option<QuickJsRealm>,
        kind: NavigationKind,
    ) -> NexusResult<NavigationResult> {
        self.persist_scroll();
        if let Some(index) = self.history_index {
            self.history.truncate(index + 1);
        } else {
            self.history.clear();
        }

        let title = page.dom.title().unwrap_or_else(|| page.final_url.as_str().to_owned());
        self.history.push(HistoryEntry {
            url: page.final_url.clone(),
            title,
            scroll_y: 0.0,
        });
        self.history_index = Some(self.history.len() - 1);
        self.scroll_y = 0.0;
        self.focused_node = None;
        self.file_inputs.clear();
        if let Some(event_loop) = self.event_loop.as_mut() { event_loop.reset_document(); }
        self.current = Some(page);
        self.realm = realm;
        self.discarded = None;
        Ok(self.navigation_result(kind))
    }

    fn navigate_history(&mut self, target_index: usize, kind: NavigationKind) -> NexusResult<NavigationResult> {
        self.persist_scroll();
        let entry = self.history[target_index].clone();
        let (page, realm) = if entry.url.scheme() == "nexus" {
            let html = self.internal_documents.get(entry.url.as_str())
                .cloned()
                .ok_or_else(|| crate::error::NexusError::InvalidInput(format!("unknown internal page: {}", entry.url)))?;
            (self.engine.load_internal_html(&entry.url, &html)?, None)
        } else {
            self.load_with_script_navigation(&entry.url)?
        };
        if let Some(event_loop) = self.event_loop.as_mut() { event_loop.reset_document(); }
        self.current = Some(page);
        self.realm = realm;
        self.focused_node = None;
        self.selection = None;
        self.file_inputs.clear();
        self.history_index = Some(target_index);
        self.scroll_y = entry.scroll_y;
        self.clamp_scroll();
        self.refresh_current_history_metadata();
        Ok(self.navigation_result(kind))
    }

    fn navigation_result(&self, kind: NavigationKind) -> NavigationResult {
        let page = self.current.as_ref().expect("navigation result requires a page");
        let index = self.history_index.expect("navigation result requires history");
        NavigationResult {
            kind,
            final_url: page.final_url.clone(),
            title: page.dom.title().unwrap_or_else(|| page.final_url.as_str().to_owned()),
            status: page.status,
            history_index: index,
            history_len: self.history.len(),
        }
    }

    fn sync_realm_into_page(&mut self) -> NexusResult<()> {
        let Some(realm) = self.realm.as_ref() else { return Ok(()) };
        let dom = realm.dom_snapshot();
        let report = realm.report().clone();
        if let Some(page) = self.current.as_mut() {
            self.engine.rebuild_page(page, dom, report)?;
            if self.focused_node.is_some_and(|node| !page.dom.is_connected(node)) {
                self.focused_node = None;
            }
            self.clamp_scroll();
            self.refresh_current_history_metadata();
        }
        Ok(())
    }

    fn follow_realm_navigation(&mut self) -> NexusResult<Option<NavigationResult>> {
        let request = self.realm.as_mut().and_then(QuickJsRealm::take_navigation_request);
        request
            .map(|url| self.navigate_url(&url, NavigationKind::Script))
            .transpose()
    }

    fn submit_form_from_interaction(
        &mut self,
        form: NodeId,
        mut result: InteractionResult,
    ) -> NexusResult<InteractionResult> {
        if let Some(realm) = self.realm.as_mut() {
            let activity = realm.dispatch_submit(form);
            result.default_prevented |= activity.default_prevented;
            result.dirty |= activity.dom_changed;
        }
        if result.dirty {
            self.sync_realm_into_page()?;
        }
        if let Some(navigation) = self.follow_realm_navigation()? {
            result.navigation = Some(navigation);
            result.dirty = true;
            return Ok(result);
        }
        if result.default_prevented {
            return Ok(result);
        }

        let Some(page) = self.current.as_ref() else { return Ok(result) };
        let dom = &page.dom;
        let action_raw = dom.attribute(form, "action").unwrap_or("");
        let action = if action_raw.trim().is_empty() {
            page.final_url.clone()
        } else {
            resolve_url(&dom.base_url(), action_raw).unwrap_or_else(|| page.final_url.clone())
        };
        let method = dom.attribute(form, "method").unwrap_or("get").to_ascii_lowercase();
        validate_form(dom, form, &self.file_inputs)?;
        let fields = serialize_form(dom, form, &self.file_inputs);

        let (page, realm) = if method == "post" {
            let enctype = dom.attribute(form, "enctype").unwrap_or("application/x-www-form-urlencoded").to_ascii_lowercase();
            if enctype == "multipart/form-data" || fields.iter().any(|field| matches!(field, FormField::File { .. })) {
                let (body, content_type) = multipart_body(&fields)?;
                self.load_post_bytes_with_realm(&action, &body, &content_type)?
            } else if enctype == "text/plain" {
                let body = text_plain_form(&fields).into_bytes();
                self.load_post_bytes_with_realm(&action, &body, "text/plain; charset=UTF-8")?
            } else {
                let pairs = text_pairs(&fields);
                let body = form_urlencoded_string(&pairs);
                self.load_post_with_realm(&action, &body)?
            }
        } else {
            let mut url = action;
            {
                let mut query = url.query_pairs_mut();
                for (name, value) in text_pairs(&fields) { query.append_pair(&name, &value); }
            }
            self.load_with_script_navigation(&url)?
        };
        let navigation = self.commit_new_page(page, realm, NavigationKind::Form)?;
        result.navigation = Some(navigation);
        result.dirty = true;
        Ok(result)
    }

    fn resolve_current_link(&self, target: NodeId) -> Option<Url> {
        let page = self.current.as_ref()?;
        let link = page.dom.closest_ancestor_tag(target, "a")?;
        let href = page.dom.attribute(link, "href")?;
        resolve_url(&page.dom.base_url(), href)
    }

    fn persist_scroll(&mut self) {
        if let Some(index) = self.history_index {
            if let Some(entry) = self.history.get_mut(index) {
                entry.scroll_y = self.scroll_y;
            }
        }
    }

    fn clamp_scroll(&mut self) {
        self.scroll_y = self.scroll_y.clamp(0.0, self.max_scroll_y());
    }

    fn refresh_current_history_metadata(&mut self) {
        let Some(index) = self.history_index else { return };
        let Some(page) = self.current.as_ref() else { return };
        if let Some(entry) = self.history.get_mut(index) {
            entry.url = page.final_url.clone();
            entry.title = page.dom.title().unwrap_or_else(|| page.final_url.as_str().to_owned());
            entry.scroll_y = self.scroll_y;
        }
    }
}

fn estimate_loaded_page_bytes(page: &LoadedPage) -> usize {
    let decoded_images = page.resources.images.values().map(|image| image.rgba.len()).sum::<usize>();
    let dom = page.dom.nodes().iter().map(|node| {
        let payload = match &node.data {
            crate::dom::DomNodeData::Text(text) | crate::dom::DomNodeData::Comment(text) => text.len(),
            crate::dom::DomNodeData::Element { tag_name, attributes, .. } => tag_name.len()
                + attributes.iter().map(|attribute| attribute.name.len() + attribute.value.len()).sum::<usize>(),
            _ => 0,
        };
        std::mem::size_of_val(node) + payload
    }).sum::<usize>();
    page.bytes_downloaded
        .saturating_add(decoded_images)
        .saturating_add(dom)
        .saturating_add(page.layout.boxes.len() * std::mem::size_of::<crate::layout::LayoutBox>())
        .saturating_add(page.display_list.commands.len() * std::mem::size_of::<crate::display_list::DisplayCommand>())
}

fn control_value(dom: &NexusDom, node: NodeId) -> String {
    match dom.element_tag_name(node).unwrap_or("").to_ascii_lowercase().as_str() {
        "textarea" => dom.text_content_raw(node),
        _ => dom.attribute(node, "value").unwrap_or("").to_owned(),
    }
}

#[derive(Debug, Clone)]
enum FormField {
    Text { name: String, value: String },
    File { name: String, file: SelectedFile },
}

fn serialize_form(dom: &NexusDom, form: NodeId, files: &HashMap<NodeId, Vec<SelectedFile>>) -> Vec<FormField> {
    let mut fields = Vec::new();
    for control in dom.form_controls(form) {
        if dom.attribute(control, "disabled").is_some() { continue; }
        let Some(name) = dom.attribute(control, "name").map(str::trim).filter(|value| !value.is_empty()) else { continue; };
        let tag = dom.element_tag_name(control).unwrap_or("").to_ascii_lowercase();
        if tag == "button" { continue; }
        if tag == "select" {
            for value in selected_option_values(dom, control) {
                fields.push(FormField::Text { name: name.to_owned(), value });
            }
            continue;
        }
        if tag == "input" {
            let kind = dom.attribute(control, "type").unwrap_or("text").to_ascii_lowercase();
            if matches!(kind.as_str(), "submit" | "button" | "reset" | "image") { continue; }
            if matches!(kind.as_str(), "checkbox" | "radio") && dom.attribute(control, "checked").is_none() { continue; }
            if matches!(kind.as_str(), "checkbox" | "radio") {
                fields.push(FormField::Text { name: name.to_owned(), value: dom.attribute(control, "value").unwrap_or("on").to_owned() });
                continue;
            }
            if kind == "file" {
                for file in files.get(&control).into_iter().flatten() {
                    fields.push(FormField::File { name: name.to_owned(), file: file.clone() });
                }
                continue;
            }
        }
        fields.push(FormField::Text { name: name.to_owned(), value: control_value(dom, control) });
    }
    fields
}

fn text_pairs(fields: &[FormField]) -> Vec<(String, String)> {
    fields.iter().map(|field| match field {
        FormField::Text { name, value } => (name.clone(), value.clone()),
        FormField::File { name, file } => (name.clone(), file.name.clone()),
    }).collect()
}

fn validate_form(dom: &NexusDom, form: NodeId, files: &HashMap<NodeId, Vec<SelectedFile>>) -> NexusResult<()> {
    for control in dom.form_controls(form) {
        if dom.attribute(control, "disabled").is_some() { continue; }
        let Some(info) = describe_control(dom, control) else { continue; };
        if info.required {
            let missing = match info.input_type.as_str() {
                "checkbox" => !info.checked,
                "radio" => {
                    let group = dom.attribute(control, "name").unwrap_or("");
                    let controls = dom.form_controls(form);
                    !controls.into_iter().any(|node| {
                        dom.element_tag_name(node).is_some_and(|tag| tag.eq_ignore_ascii_case("input"))
                            && dom.attribute(node, "type").is_some_and(|kind| kind.eq_ignore_ascii_case("radio"))
                            && dom.attribute(node, "name") == Some(group)
                            && dom.attribute(node, "checked").is_some()
                    })
                },
                "file" => files.get(&control).is_none_or(Vec::is_empty),
                "select" => selected_option_values(dom, control).iter().all(|value| value.is_empty()),
                _ => info.value.trim().is_empty(),
            };
            if missing { return Err(crate::error::NexusError::InvalidInput(format!("required form control is empty: {}", if info.name.is_empty() { info.input_type } else { info.name }))); }
        }
        if !info.value.is_empty() {
            if info.input_type == "email" && (!info.value.contains('@') || info.value.starts_with('@') || info.value.ends_with('@')) {
                return Err(crate::error::NexusError::InvalidInput(format!("invalid email value for {}", info.name)));
            }
            if info.input_type == "url" && url::Url::parse(&info.value).is_err() {
                return Err(crate::error::NexusError::InvalidInput(format!("invalid URL value for {}", info.name)));
            }
            if matches!(info.input_type.as_str(), "number" | "range") {
                let value = info.value.parse::<f64>().map_err(|_| crate::error::NexusError::InvalidInput(format!("invalid number value for {}", info.name)))?;
                if let Ok(min) = info.min.parse::<f64>() { if value < min { return Err(crate::error::NexusError::InvalidInput(format!("value below min for {}", info.name))); } }
                if let Ok(max) = info.max.parse::<f64>() { if value > max { return Err(crate::error::NexusError::InvalidInput(format!("value above max for {}", info.name))); } }
            }
        }
    }
    Ok(())
}

fn multipart_body(fields: &[FormField]) -> NexusResult<(Vec<u8>, String)> {
    const MAX_TOTAL_UPLOAD_BYTES: usize = 32 * 1024 * 1024;
    let boundary = format!("----NexusFormBoundary{:016x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() as u64);
    let mut body = Vec::new();
    for field in fields {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        match field {
            FormField::Text { name, value } => {
                body.extend_from_slice(format!("Content-Disposition: form-data; name=\"{}\"\r\n\r\n", escape_multipart(name)).as_bytes());
                body.extend_from_slice(value.as_bytes());
                body.extend_from_slice(b"\r\n");
            }
            FormField::File { name, file } => {
                let bytes = std::fs::read(&file.path)?;
                if body.len().saturating_add(bytes.len()) > MAX_TOTAL_UPLOAD_BYTES {
                    return Err(crate::error::NexusError::BodyTooLarge { limit: MAX_TOTAL_UPLOAD_BYTES, actual: body.len().saturating_add(bytes.len()) });
                }
                let mime = if file.mime_type.trim().is_empty() { "application/octet-stream" } else { file.mime_type.as_str() };
                body.extend_from_slice(format!("Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\nContent-Type: {}\r\n\r\n", escape_multipart(name), escape_multipart(&file.name), mime).as_bytes());
                body.extend_from_slice(&bytes);
                body.extend_from_slice(b"\r\n");
            }
        }
        if body.len() > MAX_TOTAL_UPLOAD_BYTES {
            return Err(crate::error::NexusError::BodyTooLarge { limit: MAX_TOTAL_UPLOAD_BYTES, actual: body.len() });
        }
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    Ok((body, format!("multipart/form-data; boundary={boundary}")))
}

fn text_plain_form(fields: &[FormField]) -> String {
    text_pairs(fields).into_iter().map(|(name, value)| format!("{name}={value}")).collect::<Vec<_>>().join("\r\n")
}

fn escape_multipart(value: &str) -> String {
    value.replace('\\', "_").replace('"', "_").replace('\r', " ").replace('\n', " ")
}

fn file_matches_accept(accept: &str, name: &str, mime_type: &str) -> bool {
    if accept.trim().is_empty() { return true; }
    let name = name.to_ascii_lowercase();
    let mime = mime_type.to_ascii_lowercase();
    accept.split(',').map(str::trim).filter(|value| !value.is_empty()).any(|rule| {
        let rule = rule.to_ascii_lowercase();
        if let Some(prefix) = rule.strip_suffix("/*") {
            mime.starts_with(&format!("{prefix}/"))
        } else if rule.starts_with('.') {
            name.ends_with(&rule)
        } else {
            mime == rule
        }
    })
}

fn sanitize_upload_name(value: &str) -> String {
    let name = std::path::Path::new(value).file_name().and_then(|value| value.to_str()).unwrap_or("upload.bin");
    let cleaned = name.chars().map(|ch| if ch.is_control() || matches!(ch, '/' | '\\') { '_' } else { ch }).collect::<String>();
    if cleaned.trim().is_empty() { "upload.bin".to_owned() } else { cleaned }
}

fn form_urlencoded_string(pairs: &[(String, String)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (name, value) in pairs {
        serializer.append_pair(name, value);
    }
    serializer.finish()
}
