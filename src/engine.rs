//! High-level Nexus Engine 1.02 orchestration.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use url::Url;

use crate::address::normalize_url;
use crate::css::StyleMap;
use crate::display_list::{build_display_list_with_resources, DisplayList};
use crate::dom::NexusDom;
use crate::encoding::{decode_html, EncodingSource};
use crate::error::NexusResult;
use crate::javascript::{JavascriptEngine, JavascriptReport, QuickJsEngine, QuickJsRealm};
use crate::layout::{compute_layout_with_intrinsics, LayoutTree, Viewport};
use crate::network::{NetworkClient, NetworkResponse, DEFAULT_MAX_BODY_BYTES};
use crate::policy::{CspPolicy, PageSecurityContext, ReferrerPolicy};
use crate::state::BrowserState;
use crate::storage::SessionStorage;
use crate::parser::parse_html;
use crate::renderer::{Renderer, SkiaRenderer};
use crate::resource::{
    load_page_resources, PageResources, ResourceCache, DEFAULT_CACHE_BYTES,
    DEFAULT_CACHE_ENTRIES,
};
use crate::style_engine::{NexusStyleEngine, StyleEngine};
use crate::text::ParleyTextEngine;

#[derive(Debug)]
pub struct LoadedPage {
    pub requested_url: Url,
    pub final_url: Url,
    pub status: u16,
    pub content_type: Option<String>,
    pub bytes_downloaded: usize,
    pub from_http_cache: bool,
    pub cache_revalidated: bool,
    pub hsts_upgraded: bool,
    pub security: PageSecurityContext,
    pub encoding: &'static str,
    pub encoding_source: EncodingSource,
    pub had_decode_errors: bool,
    pub dom: NexusDom,
    pub javascript: JavascriptReport,
    pub styles: StyleMap,
    pub resources: PageResources,
    pub layout: LayoutTree,
    pub display_list: DisplayList,
}

impl LoadedPage {
    #[must_use]
    pub fn max_scroll_y(&self) -> f32 {
        (self.display_list.content_height - self.layout.viewport.height).max(0.0)
    }
}

pub struct NexusEngineBuilder {
    max_body_bytes: usize,
    viewport: Viewport,
    style_engine: Box<dyn StyleEngine>,
    javascript_enabled: bool,
    javascript_engine: QuickJsEngine,
    cache_entries: usize,
    cache_bytes: usize,
    profile_dir: Option<PathBuf>,
    browser_state: Option<Arc<BrowserState>>,
}

impl Default for NexusEngineBuilder {
    fn default() -> Self {
        Self {
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            viewport: Viewport::default(),
            style_engine: Box::new(NexusStyleEngine),
            javascript_enabled: true,
            javascript_engine: QuickJsEngine::default(),
            cache_entries: DEFAULT_CACHE_ENTRIES,
            cache_bytes: DEFAULT_CACHE_BYTES,
            profile_dir: None,
            browser_state: None,
        }
    }
}

impl NexusEngineBuilder {
    #[must_use]
    pub fn max_body_bytes(mut self, bytes: usize) -> Self {
        self.max_body_bytes = bytes.max(1024);
        self
    }

    #[must_use]
    pub fn viewport(mut self, width: f32, height: f32) -> Self {
        self.viewport = Viewport {
            width: width.max(1.0),
            height: height.max(1.0),
        };
        self
    }

    #[must_use]
    pub fn style_engine(mut self, engine: impl StyleEngine + 'static) -> Self {
        self.style_engine = Box::new(engine);
        self
    }

    #[must_use]
    pub fn javascript_engine(mut self, engine: QuickJsEngine) -> Self {
        self.javascript_engine = engine;
        self.javascript_enabled = true;
        self
    }

    #[must_use]
    pub fn javascript_enabled(mut self, enabled: bool) -> Self {
        self.javascript_enabled = enabled;
        self
    }

    #[must_use]
    pub fn resource_cache(mut self, max_entries: usize, max_bytes: usize) -> Self {
        self.cache_entries = max_entries.max(1);
        self.cache_bytes = max_bytes.max(1024 * 1024);
        self
    }

    /// Optional browser profile directory. Nexus 0.20 persists localStorage
    /// and persistent cookies here. sessionStorage remains BrowserSession-scoped.
    #[must_use]
    pub fn profile_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.profile_dir = Some(path.into());
        self
    }


    /// Reuses an existing browser-profile state. Nexus 0.20 uses this to let
    /// multiple tabs share cookies, HTTP cache, localStorage, permissions and HSTS.
    #[must_use]
    pub fn browser_state(mut self, state: Arc<BrowserState>) -> Self {
        self.browser_state = Some(state);
        self
    }

    pub fn build(self) -> NexusResult<NexusEngine> {
        let browser_state = self.browser_state.unwrap_or_else(|| BrowserState::new(self.profile_dir));
        Ok(NexusEngine {
            network: NetworkClient::with_state(self.max_body_bytes, browser_state)?,
            viewport: self.viewport,
            style_engine: self.style_engine,
            javascript_enabled: self.javascript_enabled,
            javascript_engine: self.javascript_engine,
            text_engine: RefCell::new(ParleyTextEngine::new()),
            resource_cache: RefCell::new(ResourceCache::new(self.cache_entries, self.cache_bytes)),
        })
    }
}

pub struct NexusEngine {
    network: NetworkClient,
    viewport: Viewport,
    style_engine: Box<dyn StyleEngine>,
    javascript_enabled: bool,
    javascript_engine: QuickJsEngine,
    text_engine: RefCell<ParleyTextEngine>,
    resource_cache: RefCell<ResourceCache>,
}

impl NexusEngine {
    #[must_use]
    pub fn builder() -> NexusEngineBuilder {
        NexusEngineBuilder::default()
    }

    pub fn new() -> NexusResult<Self> {
        Self::builder().build()
    }

    /// Standalone one-shot page load. BrowserSession uses `load_url_raw` plus
    /// a persistent QuickJsRealm instead, but the CLI keeps this convenient
    /// self-contained path.
    pub fn load(&self, input: &str) -> NexusResult<LoadedPage> {
        let url = normalize_url(input)?;
        self.load_url(&url)
    }

    pub fn load_url(&self, url: &Url) -> NexusResult<LoadedPage> {
        let mut page = self.load_url_raw(url)?;
        if self.javascript_enabled {
            let (dom, report) = self
                .javascript_engine
                .execute_page(page.dom.clone(), &self.network, self.viewport, page.security.clone());
            self.rebuild_page(&mut page, dom, report)?;
        }
        Ok(page)
    }

    /// Fetches/parses/styles/layouts a page without executing JavaScript.
    /// BrowserSession then attaches a live realm and rebuilds from its DOM.
    pub fn load_url_raw(&self, url: &Url) -> NexusResult<LoadedPage> {
        let response = self.network.fetch(url)?;
        self.page_from_response(response)
    }

    /// Builds a trusted internal Nexus document without touching the network.
    pub fn load_internal_html(&self, url: &Url, html: &str) -> NexusResult<LoadedPage> {
        let mut headers = HashMap::new();
        headers.insert("content-type".to_owned(), "text/html; charset=utf-8".to_owned());
        self.page_from_response(NetworkResponse {
            requested_url: url.clone(),
            final_url: url.clone(),
            status: 200,
            content_type: Some("text/html; charset=utf-8".to_owned()),
            headers,
            body: html.as_bytes().to_vec(),
            from_http_cache: false,
            revalidated: false,
            hsts_upgraded: false,
        })
    }

    pub fn load_post_form_raw(&self, url: &Url, body: &str) -> NexusResult<LoadedPage> {
        let response = self.network.request(
            url,
            "POST",
            "text/html,application/xhtml+xml;q=0.9,*/*;q=0.1",
            Some(body.as_bytes()),
            Some("application/x-www-form-urlencoded"),
        )?;
        self.page_from_response(response)
    }

    pub fn load_post_raw(&self, url: &Url, body: &[u8], content_type: &str) -> NexusResult<LoadedPage> {
        let response = self.network.request(
            url,
            "POST",
            "text/html,application/xhtml+xml;q=0.9,*/*;q=0.1",
            Some(body),
            Some(content_type),
        )?;
        self.page_from_response(response)
    }

    pub fn create_javascript_realm(&self, dom: NexusDom, security: PageSecurityContext) -> Result<Option<QuickJsRealm>, String> {
        self.create_javascript_realm_with_storage(dom, SessionStorage::default(), security)
    }

    pub fn create_javascript_realm_with_storage(
        &self,
        dom: NexusDom,
        session_storage: SessionStorage,
        security: PageSecurityContext,
    ) -> Result<Option<QuickJsRealm>, String> {
        if !self.javascript_enabled {
            return Ok(None);
        }
        self.javascript_engine
            .create_realm(dom, self.network.clone(), self.viewport, session_storage, security)
            .map(Some)
    }

    pub fn rebuild_page(
        &self,
        page: &mut LoadedPage,
        dom: NexusDom,
        javascript: JavascriptReport,
    ) -> NexusResult<()> {
        let styles = self.style_engine.compute_for_viewport(&dom, self.viewport.width, self.viewport.height);
        let resources = {
            let mut cache = self.resource_cache.borrow_mut();
            load_page_resources(&dom, &self.network, &mut cache, &page.security)
        };
        let intrinsic = resources.intrinsic_sizes();
        let layout = compute_layout_with_intrinsics(&dom, &styles, self.viewport, &intrinsic)?;
        let display_list = build_display_list_with_resources(
            &dom,
            &styles,
            &layout,
            &resources,
            &mut *self.text_engine.borrow_mut(),
            0.0,
        );

        page.dom = dom;
        page.javascript = javascript;
        page.styles = styles;
        page.resources = resources;
        page.layout = layout;
        page.display_list = display_list;
        Ok(())
    }

    fn page_from_response(&self, response: NetworkResponse) -> NexusResult<LoadedPage> {
        let bytes_downloaded = response.body.len();
        let from_http_cache = response.from_http_cache;
        let cache_revalidated = response.revalidated;
        let decoded = decode_html(&response.body, response.content_type.as_deref());
        let dom = parse_html(response.final_url.clone(), &decoded.text);
        let security = PageSecurityContext {
            document_url: response.final_url.clone(),
            csp: CspPolicy::parse(response.headers.get("content-security-policy").map(String::as_str)),
            referrer_policy: ReferrerPolicy::parse_header(
                response.headers.get("referrer-policy").map(String::as_str),
            ),
        };
        let styles = self.style_engine.compute_for_viewport(&dom, self.viewport.width, self.viewport.height);
        let resources = {
            let mut cache = self.resource_cache.borrow_mut();
            load_page_resources(&dom, &self.network, &mut cache, &security)
        };
        let intrinsic = resources.intrinsic_sizes();
        let layout = compute_layout_with_intrinsics(&dom, &styles, self.viewport, &intrinsic)?;
        let display_list = build_display_list_with_resources(
            &dom,
            &styles,
            &layout,
            &resources,
            &mut *self.text_engine.borrow_mut(),
            0.0,
        );

        Ok(LoadedPage {
            requested_url: response.requested_url,
            final_url: response.final_url,
            status: response.status,
            content_type: response.content_type,
            bytes_downloaded,
            from_http_cache,
            cache_revalidated,
            hsts_upgraded: response.hsts_upgraded,
            security,
            encoding: decoded.encoding,
            encoding_source: decoded.source,
            had_decode_errors: decoded.had_errors,
            dom,
            javascript: JavascriptReport::disabled(),
            styles,
            resources,
            layout,
            display_list,
        })
    }

    pub fn display_list_at_scroll(&self, page: &LoadedPage, scroll_y: f32) -> DisplayList {
        build_display_list_with_resources(
            &page.dom,
            &page.styles,
            &page.layout,
            &page.resources,
            &mut *self.text_engine.borrow_mut(),
            scroll_y,
        )
    }

    pub fn render_page_png(&self, page: &LoadedPage) -> NexusResult<Vec<u8>> {
        let mut renderer = SkiaRenderer::new();
        renderer.render_png(&page.display_list)
    }

    pub fn render_page_png_at_scroll(
        &self,
        page: &LoadedPage,
        scroll_y: f32,
    ) -> NexusResult<Vec<u8>> {
        let display_list = self.display_list_at_scroll(page, scroll_y);
        let mut renderer = SkiaRenderer::new();
        renderer.render_png(&display_list)
    }

    pub fn render_page_png_file(&self, page: &LoadedPage, path: &Path) -> NexusResult<()> {
        let mut renderer = SkiaRenderer::new();
        renderer.render_png_file(&page.display_list, path)
    }

    pub fn render_page_png_file_at_scroll(
        &self,
        page: &LoadedPage,
        path: &Path,
        scroll_y: f32,
    ) -> NexusResult<()> {
        let display_list = self.display_list_at_scroll(page, scroll_y);
        let mut renderer = SkiaRenderer::new();
        renderer.render_png_file(&display_list, path)
    }

    #[must_use]
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// Updates the CSS viewport used for the next style/layout pass. Nexus 0.20
    /// uses this for page zoom: the Android surface keeps its physical size while
    /// the CSS viewport shrinks/grows and the page is reflowed.
    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = Viewport {
            width: viewport.width.max(1.0),
            height: viewport.height.max(1.0),
        };
    }

    #[must_use]
    pub fn javascript_enabled(&self) -> bool {
        self.javascript_enabled
    }

    #[must_use]
    pub fn cache_stats(&self) -> (usize, usize, usize, usize) {
        let cache = self.resource_cache.borrow();
        (cache.len(), cache.bytes(), cache.hits(), cache.misses())
    }
    #[must_use]
    pub fn browser_state(&self) -> Arc<BrowserState> {
        self.network.browser_state()
    }

    #[must_use]
    pub fn http_cache_stats(&self) -> crate::cache::HttpCacheStats {
        self.network.browser_state().http_cache_stats()
    }

}
