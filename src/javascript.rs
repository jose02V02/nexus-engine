//! Persistent JavaScript execution layer for Nexus Engine 1.02.
//!
//! QuickJS-ng is embedded through `quickjs-rusty`. Unlike Nexus 0.6, a
//! `QuickJsRealm` can remain alive for the full lifetime of a BrowserSession
//! document so globals, event listeners, promises and timers survive after the
//! initial page load.

use std::cell::Cell;
use std::ffi::c_void;
use std::os::raw::c_int;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use quickjs_rusty::console::Level;
use quickjs_rusty::value::q::JSRuntime;
use quickjs_rusty::{Context, OwnedJsValue};
use serde_json::json;
use url::Url;

use crate::address::resolve_url;
use crate::dom::{NexusDom, NodeId, ScriptReference};
use crate::encoding::decode_script;
use crate::layout::Viewport;
use crate::event_loop::{WebSocketCommand, WebSocketEvent};
use crate::network::{NetworkClient, SubresourceKind};
use crate::origin::Origin;
use crate::policy::{is_mixed_active_content, PageSecurityContext};
use crate::security::{CredentialsMode, FetchMode};
use crate::storage::{SessionStorage, StorageError};
use crate::websocket::WebSocketRequest;

pub const DEFAULT_JS_MEMORY_LIMIT: usize = 32 * 1024 * 1024;
pub const DEFAULT_JS_STACK_LIMIT: usize = 2 * 1024 * 1024;
pub const DEFAULT_JS_EXECUTION_TIMEOUT_MS: u64 = 1_500;
pub const DEFAULT_MAX_SCRIPT_BYTES: usize = 2 * 1024 * 1024;
pub const DEFAULT_MAX_PAGE_SCRIPT_BYTES: usize = 8 * 1024 * 1024;
pub const DEFAULT_MAX_SCRIPTS: usize = 64;
static NEXT_WEBSOCKET_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleEntry {
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JavascriptReport {
    pub enabled: bool,
    pub runtime: String,
    pub persistent_realm: bool,
    pub scripts_found: usize,
    pub scripts_executed: usize,
    pub inline_scripts_executed: usize,
    pub external_scripts_loaded: usize,
    pub script_bytes_executed: usize,
    pub dom_mutations: usize,
    pub events_dispatched: usize,
    pub timers_executed: usize,
    pub fetch_requests: usize,
    pub websocket_connections: usize,
    pub websocket_events: usize,
    pub console: Vec<ConsoleEntry>,
    pub warnings: Vec<String>,
    pub navigation_request: Option<Url>,
}

impl JavascriptReport {
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            runtime: "disabled".to_owned(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn pretty(&self) -> String {
        if !self.enabled {
            return "JavaScript: disabled\n".to_owned();
        }
        let mut out = format!(
            "JavaScript runtime: {}\nPersistent realm: {}\nScripts: {}/{} executed\nExternal loaded: {}\nBytes executed: {}\nDOM mutations: {}\nEvents: {}\nTimers: {}\nFetch: {}\nWebSockets: {} connections / {} events\n",
            self.runtime,
            self.persistent_realm,
            self.scripts_executed,
            self.scripts_found,
            self.external_scripts_loaded,
            self.script_bytes_executed,
            self.dom_mutations,
            self.events_dispatched,
            self.timers_executed,
            self.fetch_requests,
            self.websocket_connections,
            self.websocket_events,
        );
        if let Some(url) = &self.navigation_request {
            out.push_str(&format!("Navigation requested: {url}\n"));
        }
        if !self.console.is_empty() {
            out.push_str("Console:\n");
            for entry in &self.console {
                out.push_str(&format!("  [{}] {}\n", entry.level, entry.message));
            }
        }
        if !self.warnings.is_empty() {
            out.push_str("Warnings:\n");
            for warning in &self.warnings {
                out.push_str(&format!("  - {warning}\n"));
            }
        }
        out
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JavascriptActivity {
    pub dom_changed: bool,
    pub default_prevented: bool,
    pub timers_executed: usize,
    pub events_dispatched: usize,
}

pub trait JavascriptEngine {
    fn execute_page(
        &self,
        dom: NexusDom,
        network: &NetworkClient,
        viewport: Viewport,
        security: PageSecurityContext,
    ) -> (NexusDom, JavascriptReport);
}

#[derive(Debug, Clone)]
pub struct QuickJsEngine {
    memory_limit: usize,
    stack_limit: usize,
    max_script_bytes: usize,
    max_page_script_bytes: usize,
    max_scripts: usize,
    execution_timeout: Duration,
}

impl Default for QuickJsEngine {
    fn default() -> Self {
        Self {
            memory_limit: DEFAULT_JS_MEMORY_LIMIT,
            stack_limit: DEFAULT_JS_STACK_LIMIT,
            max_script_bytes: DEFAULT_MAX_SCRIPT_BYTES,
            max_page_script_bytes: DEFAULT_MAX_PAGE_SCRIPT_BYTES,
            max_scripts: DEFAULT_MAX_SCRIPTS,
            execution_timeout: Duration::from_millis(DEFAULT_JS_EXECUTION_TIMEOUT_MS),
        }
    }
}

impl QuickJsEngine {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn memory_limit(mut self, bytes: usize) -> Self {
        self.memory_limit = bytes.max(1024 * 1024);
        self
    }

    #[must_use]
    pub fn stack_limit(mut self, bytes: usize) -> Self {
        self.stack_limit = bytes.max(256 * 1024);
        self
    }

    #[must_use]
    pub fn script_limits(mut self, max_script_bytes: usize, max_page_bytes: usize) -> Self {
        self.max_script_bytes = max_script_bytes.max(1024);
        self.max_page_script_bytes = max_page_bytes.max(self.max_script_bytes);
        self
    }

    #[must_use]
    pub fn execution_timeout_ms(mut self, milliseconds: u64) -> Self {
        self.execution_timeout = Duration::from_millis(milliseconds.clamp(10, 30_000));
        self
    }

    pub fn create_realm(
        &self,
        dom: NexusDom,
        network: NetworkClient,
        viewport: Viewport,
        session_storage: SessionStorage,
        security: PageSecurityContext,
    ) -> Result<QuickJsRealm, String> {
        let scripts = dom.scripts();
        let state = Arc::new(Mutex::new(BridgeState::new(dom, security.clone())));
        let context = self.build_context(&state, network.clone(), viewport, session_storage)?;
        let mut report = JavascriptReport {
            enabled: true,
            runtime: "QuickJS-ng via quickjs-rusty 0.13".to_owned(),
            persistent_realm: true,
            scripts_found: scripts.len(),
            ..JavascriptReport::default()
        };

        let mut total_bytes = 0usize;
        for (index, script) in scripts.iter().enumerate() {
            if index >= self.max_scripts {
                report.warnings.push(format!(
                    "script limit reached ({}); remaining scripts skipped",
                    self.max_scripts
                ));
                break;
            }
            if script.is_module {
                report.warnings.push(format!(
                    "script #{} uses type=module; module loading is not enabled in Nexus 0.20 yet",
                    script.node_id
                ));
                continue;
            }
            if !is_classic_javascript_type(&script.script_type) {
                report.warnings.push(format!(
                    "script #{} skipped because type='{}' is not a classic JavaScript MIME",
                    script.node_id, script.script_type
                ));
                continue;
            }

            if script.src.is_none() && !security.csp.allows_inline_script() {
                report.warnings.push(format!("CSP blocked inline script #{}", script.node_id));
                continue;
            }
            let Some((code, external)) = load_script_source(script, &network, &security, &mut report) else {
                continue;
            };
            let bytes = code.len();
            if bytes > self.max_script_bytes {
                report.warnings.push(format!(
                    "script #{} is {} bytes, above the per-script limit of {} bytes",
                    script.node_id, bytes, self.max_script_bytes
                ));
                continue;
            }
            if total_bytes.saturating_add(bytes) > self.max_page_script_bytes {
                report.warnings.push(format!(
                    "page script budget exceeded ({} bytes); remaining scripts skipped",
                    self.max_page_script_bytes
                ));
                break;
            }

            context.update_stack_top();
            match with_js_deadline(self.execution_timeout, || context.eval(&code, false)) {
                Ok(_) => {
                    report.scripts_executed += 1;
                    report.script_bytes_executed += bytes;
                    total_bytes += bytes;
                    if external {
                        report.external_scripts_loaded += 1;
                    } else {
                        report.inline_scripts_executed += 1;
                    }
                    if let Err(error) =
                        with_js_deadline(self.execution_timeout, || context.execute_pending_job())
                    {
                        report.warnings.push(format!(
                            "pending promise job after script #{} failed: {error}",
                            script.node_id
                        ));
                    }
                }
                Err(error) => report
                    .warnings
                    .push(format!("script #{} threw: {error}", script.node_id)),
            }
        }

        let mut realm = QuickJsRealm {
            context,
            state,
            network,
            viewport,
            report,
            execution_timeout: self.execution_timeout,
        };
        realm.dispatch_document_event("DOMContentLoaded");
        realm.pump();
        realm.dispatch_document_event("load");
        realm.pump();
        realm.sync_report_from_state();
        Ok(realm)
    }

    fn build_context(
        &self,
        state: &Arc<Mutex<BridgeState>>,
        network: NetworkClient,
        viewport: Viewport,
        session_storage: SessionStorage,
    ) -> Result<Context, String> {
        let console_state = Arc::clone(state);
        let context = Context::builder()
            .memory_limit(self.memory_limit)
            .console(move |level: Level, values: Vec<OwnedJsValue>| {
                let message = values
                    .iter()
                    .map(js_value_to_string)
                    .collect::<Vec<_>>()
                    .join(" ");
                if let Ok(mut state) = console_state.lock() {
                    state.console.push(ConsoleEntry {
                        level: level.to_string(),
                        message,
                    });
                }
            })
            .build()
            .map_err(|error| error.to_string())?;

        context.set_max_stack_size(self.stack_limit);
        context.set_interrupt_handler(Some(nexus_interrupt_handler), std::ptr::null_mut());
        context.update_stack_top();
        install_native_callbacks(&context, state, network, session_storage)?;
        context
            .set_global("__nexus_inner_width", f64::from(viewport.width))
            .map_err(|error| error.to_string())?;
        context
            .set_global("__nexus_inner_height", f64::from(viewport.height))
            .map_err(|error| error.to_string())?;
        context
            .eval(WEB_API_BOOTSTRAP, false)
            .map_err(|error| error.to_string())?;
        Ok(context)
    }
}

impl JavascriptEngine for QuickJsEngine {
    fn execute_page(
        &self,
        dom: NexusDom,
        network: &NetworkClient,
        viewport: Viewport,
        security: PageSecurityContext,
    ) -> (NexusDom, JavascriptReport) {
        match self.create_realm(dom.clone(), network.clone(), viewport, SessionStorage::default(), security) {
            Ok(mut realm) => {
                realm.pump();
                let pending_websockets = realm.take_websocket_commands().len();
                let dom = realm.dom_snapshot();
                let mut report = realm.report().clone();
                if pending_websockets > 0 {
                    report.warnings.push(format!(
                        "{pending_websockets} WebSocket command(s) were queued in one-shot mode; use BrowserSession to keep sockets alive"
                    ));
                }
                (dom, report)
            }
            Err(error) => {
                let mut report = JavascriptReport {
                    enabled: true,
                    runtime: "QuickJS-ng via quickjs-rusty 0.13".to_owned(),
                    ..JavascriptReport::default()
                };
                report
                    .warnings
                    .push(format!("QuickJS context could not start: {error}"));
                (dom, report)
            }
        }
    }
}

/// A live JavaScript realm tied to one Nexus document.
///
/// `Context` must stay on the thread that created it; BrowserSession and the
/// Android native worker deliberately serialize all calls on one thread.
pub struct QuickJsRealm {
    context: Context,
    state: Arc<Mutex<BridgeState>>,
    network: NetworkClient,
    viewport: Viewport,
    report: JavascriptReport,
    execution_timeout: Duration,
}

impl QuickJsRealm {
    #[must_use]
    pub fn dom_snapshot(&self) -> NexusDom {
        self.state
            .lock()
            .map_or_else(|poisoned| poisoned.into_inner().dom.clone(), |state| state.dom.clone())
    }

    pub fn replace_dom(&mut self, dom: NexusDom) {
        match self.state.lock() {
            Ok(mut state) => { state.dom = dom; state.mutations = state.mutations.saturating_add(1); }
            Err(poisoned) => { let mut state = poisoned.into_inner(); state.dom = dom; state.mutations = state.mutations.saturating_add(1); }
        }
        self.sync_report_from_state();
    }

    #[must_use]
    pub fn report(&self) -> &JavascriptReport {
        &self.report
    }

    #[must_use]
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// Keeps window.innerWidth/innerHeight aligned with Nexus' effective CSS
    /// viewport after a user zoom operation.
    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
        let code = format!(
            "globalThis.innerWidth = {}; globalThis.innerHeight = {};",
            viewport.width.max(1.0),
            viewport.height.max(1.0)
        );
        if let Err(error) = self.eval(&code) {
            self.report.warnings.push(format!("viewport JS sync failed: {error}"));
        }
    }

    #[must_use]
    pub fn network(&self) -> &NetworkClient {
        &self.network
    }

    pub fn take_navigation_request(&mut self) -> Option<Url> {
        let request = self
            .state
            .lock()
            .ok()
            .and_then(|mut state| state.navigation_request.take());
        self.sync_report_from_state();
        request
    }

    pub fn take_websocket_commands(&mut self) -> Vec<WebSocketCommand> {
        let commands = self
            .state
            .lock()
            .map(|mut state| std::mem::take(&mut state.websocket_commands))
            .unwrap_or_default();
        self.sync_report_from_state();
        commands
    }

    pub fn deliver_websocket_event(&mut self, event: &WebSocketEvent) -> JavascriptActivity {
        let (id, kind, payload) = match event {
            WebSocketEvent::Open { id, protocol } => (*id, "open", json!({"protocol": protocol})),
            WebSocketEvent::Text { id, text } => (*id, "message", json!({"data": text, "binary": false})),
            WebSocketEvent::Binary { id, data } => (*id, "message", json!({"data": data, "binary": true})),
            WebSocketEvent::Error { id, message } => (*id, "error", json!({"message": message})),
            WebSocketEvent::Closed { id, code, reason } => (*id, "close", json!({"code": code, "reason": reason})),
        };
        if let Ok(mut state) = self.state.lock() {
            state.websocket_events = state.websocket_events.saturating_add(1);
        }
        let kind = serde_json::to_string(kind).unwrap_or_else(|_| "\"error\"".to_owned());
        let payload = payload.to_string();
        let before = self.mutation_count();
        if let Err(error) = self.eval(&format!("globalThis.__nexusWsDeliver({id}, {kind}, {payload});")) {
            self.report.warnings.push(format!("WebSocket event delivery failed: {error}"));
        }
        let pumped = self.pump();
        self.sync_report_from_state();
        JavascriptActivity {
            dom_changed: self.mutation_count() != before || pumped.dom_changed,
            timers_executed: pumped.timers_executed,
            events_dispatched: 1,
            default_prevented: false,
        }
    }

    pub fn dispatch_click(&mut self, node: NodeId) -> JavascriptActivity {
        self.dispatch_element_event(node, "click")
    }

    pub fn dispatch_input(&mut self, node: NodeId, value: &str) -> JavascriptActivity {
        let value_json = serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned());
        let code = format!(
            "globalThis.__nexusSetControlValue({node}, {value_json});\n\
             globalThis.__nexusDispatchElementEvent({node}, 'input');\n\
             globalThis.__nexusDispatchElementEvent({node}, 'change');"
        );
        let before = self.mutation_count();
        let mut activity = JavascriptActivity::default();
        match self.eval(&code) {
            Ok(_) => {
                activity.events_dispatched = 2;
                self.report.events_dispatched += 2;
            }
            Err(error) => self
                .report
                .warnings
                .push(format!("input event failed: {error}")),
        }
        let pumped = self.pump();
        activity.timers_executed = pumped.timers_executed;
        activity.dom_changed = self.mutation_count() != before || pumped.dom_changed;
        self.sync_report_from_state();
        activity
    }

    pub fn dispatch_checked(&mut self, node: NodeId, checked: bool) -> JavascriptActivity {
        let checked_json = if checked { "true" } else { "false" };
        let code = format!(
            "globalThis.__nexusSetControlChecked({node}, {checked_json});\n\
             globalThis.__nexusDispatchElementEvent({node}, 'input');\n\
             globalThis.__nexusDispatchElementEvent({node}, 'change');"
        );
        let before = self.mutation_count();
        let mut activity = JavascriptActivity::default();
        match self.eval(&code) {
            Ok(_) => { activity.events_dispatched = 2; self.report.events_dispatched += 2; }
            Err(error) => self.report.warnings.push(format!("checked event failed: {error}")),
        }
        let pumped = self.pump();
        activity.timers_executed = pumped.timers_executed;
        activity.dom_changed = self.mutation_count() != before || pumped.dom_changed;
        self.sync_report_from_state();
        activity
    }

    pub fn dispatch_submit(&mut self, form: NodeId) -> JavascriptActivity {
        self.dispatch_element_event(form, "submit")
    }

    pub fn dispatch_element_event(&mut self, node: NodeId, event: &str) -> JavascriptActivity {
        let event_json = serde_json::to_string(event).unwrap_or_else(|_| "\"event\"".to_owned());
        let code = format!(
            "globalThis.__nexusDispatchElementEvent({node}, {event_json}) ? 1 : 0"
        );
        let before = self.mutation_count();
        let mut activity = JavascriptActivity {
            events_dispatched: 1,
            ..JavascriptActivity::default()
        };
        match self.eval_i32(&code) {
            Ok(value) => activity.default_prevented = value != 0,
            Err(error) => self
                .report
                .warnings
                .push(format!("dispatching {event} failed: {error}")),
        }
        self.report.events_dispatched += 1;
        let pumped = self.pump();
        activity.timers_executed = pumped.timers_executed;
        activity.dom_changed = self.mutation_count() != before || pumped.dom_changed;
        self.sync_report_from_state();
        activity
    }

    pub fn dispatch_document_event(&mut self, event: &str) -> JavascriptActivity {
        let event_json = serde_json::to_string(event).unwrap_or_else(|_| "\"event\"".to_owned());
        let code = format!("globalThis.__nexusDispatchDocumentEvent({event_json});");
        let before = self.mutation_count();
        if let Err(error) = self.eval(&code) {
            self.report
                .warnings
                .push(format!("dispatching {event} failed: {error}"));
        }
        self.report.events_dispatched += 1;
        let pumped = self.pump();
        self.sync_report_from_state();
        JavascriptActivity {
            dom_changed: self.mutation_count() != before || pumped.dom_changed,
            events_dispatched: 1,
            timers_executed: pumped.timers_executed,
            default_prevented: false,
        }
    }

    /// Runs due timers and pending Promise jobs. The realm remains alive after
    /// this call, so future ticks can continue from the same JavaScript state.
    pub fn pump(&mut self) -> JavascriptActivity {
        let before = self.mutation_count();
        let mut timers = 0usize;
        match self.eval_i32("globalThis.__nexusDrainDueTimers(Date.now())") {
            Ok(count) => timers = usize::try_from(count.max(0)).unwrap_or(0),
            Err(error) => self
                .report
                .warnings
                .push(format!("timer queue failed: {error}")),
        }
        if let Err(error) =
            with_js_deadline(self.execution_timeout, || self.context.execute_pending_job())
        {
            self.report
                .warnings
                .push(format!("pending Promise job failed: {error}"));
        }
        self.report.timers_executed += timers;
        self.sync_report_from_state();
        JavascriptActivity {
            dom_changed: self.mutation_count() != before,
            timers_executed: timers,
            ..JavascriptActivity::default()
        }
    }

    #[must_use]
    pub fn next_timer_delay_ms(&self) -> Option<u64> {
        match self.eval_f64("globalThis.__nexusNextTimerDelay(Date.now())") {
            Ok(value) if value.is_finite() && value >= 0.0 => Some(value.ceil() as u64),
            _ => None,
        }
    }

    fn eval(&self, code: &str) -> Result<OwnedJsValue, String> {
        self.context.update_stack_top();
        with_js_deadline(self.execution_timeout, || self.context.eval(code, false))
            .map_err(|error| error.to_string())
    }

    fn eval_i32(&self, code: &str) -> Result<i32, String> {
        self.eval(code)?
            .to_int()
            .map_err(|error| error.to_string())
    }

    fn eval_f64(&self, code: &str) -> Result<f64, String> {
        self.eval(code)?
            .to_float()
            .map_err(|error| error.to_string())
    }

    fn mutation_count(&self) -> usize {
        self.state
            .lock()
            .map_or_else(|poisoned| poisoned.into_inner().mutations, |state| state.mutations)
    }

    fn sync_report_from_state(&mut self) {
        match self.state.lock() {
            Ok(state) => {
                self.report.dom_mutations = state.mutations;
                self.report.fetch_requests = state.fetch_requests;
                self.report.websocket_connections = state.websocket_connections;
                self.report.websocket_events = state.websocket_events;
                self.report.console = state.console.clone();
                self.report.navigation_request = state.navigation_request.clone();
            }
            Err(poisoned) => {
                let state = poisoned.into_inner();
                self.report.dom_mutations = state.mutations;
                self.report.fetch_requests = state.fetch_requests;
                self.report.websocket_connections = state.websocket_connections;
                self.report.websocket_events = state.websocket_events;
                self.report.console = state.console.clone();
                self.report.navigation_request = state.navigation_request.clone();
                self.report
                    .warnings
                    .push("JavaScript bridge state was poisoned; state recovered".to_owned());
            }
        }
    }
}

#[derive(Debug)]
struct BridgeState {
    dom: NexusDom,
    mutations: usize,
    fetch_requests: usize,
    websocket_connections: usize,
    websocket_events: usize,
    websocket_commands: Vec<WebSocketCommand>,
    security: PageSecurityContext,
    console: Vec<ConsoleEntry>,
    navigation_request: Option<Url>,
}

impl BridgeState {
    fn new(dom: NexusDom, security: PageSecurityContext) -> Self {
        Self {
            dom,
            mutations: 0,
            fetch_requests: 0,
            websocket_connections: 0,
            websocket_events: 0,
            websocket_commands: Vec::new(),
            security,
            console: Vec::new(),
            navigation_request: None,
        }
    }

    fn mark_mutation(&mut self, changed: bool) -> bool {
        if changed {
            self.mutations += 1;
        }
        changed
    }
}

fn install_native_callbacks(
    context: &Context,
    state: &Arc<Mutex<BridgeState>>,
    network: NetworkClient,
    session_storage: SessionStorage,
) -> Result<(), String> {
    let query_state = Arc::clone(state);
    context
        .add_callback("__nexus_query", move |selector: String| -> i32 {
            query_state
                .lock()
                .ok()
                .and_then(|state| state.dom.query_selector(&selector))
                .and_then(|id| i32::try_from(id).ok())
                .unwrap_or(-1)
        })
        .map_err(|error| error.to_string())?;

    let by_id_state = Arc::clone(state);
    context
        .add_callback("__nexus_get_element_by_id", move |wanted: String| -> i32 {
            by_id_state
                .lock()
                .ok()
                .and_then(|state| state.dom.find_element_by_id(&wanted))
                .and_then(|id| i32::try_from(id).ok())
                .unwrap_or(-1)
        })
        .map_err(|error| error.to_string())?;

    let first_state = Arc::clone(state);
    context
        .add_callback("__nexus_first_element", move |tag: String| -> i32 {
            first_state
                .lock()
                .ok()
                .and_then(|state| state.dom.find_first_element(&tag))
                .and_then(|id| i32::try_from(id).ok())
                .unwrap_or(-1)
        })
        .map_err(|error| error.to_string())?;

    let parent_state = Arc::clone(state);
    context
        .add_callback("__nexus_parent_element", move |id: i32| -> i32 {
            node_id(id)
                .and_then(|id| parent_state.lock().ok().and_then(|state| state.dom.parent_element(id)))
                .and_then(|id| i32::try_from(id).ok())
                .unwrap_or(-1)
        })
        .map_err(|error| error.to_string())?;

    let text_state = Arc::clone(state);
    context
        .add_callback("__nexus_get_text", move |id: i32| -> String {
            node_id(id)
                .and_then(|id| text_state.lock().ok().map(|state| state.dom.text_content_raw(id)))
                .unwrap_or_default()
        })
        .map_err(|error| error.to_string())?;

    let set_text_state = Arc::clone(state);
    context
        .add_callback("__nexus_set_text", move |id: i32, value: String| -> bool {
            let Some(id) = node_id(id) else { return false };
            let Ok(mut state) = set_text_state.lock() else { return false };
            let changed = state.dom.set_text_content(id, &value);
            state.mark_mutation(changed)
        })
        .map_err(|error| error.to_string())?;

    let attr_state = Arc::clone(state);
    context
        .add_callback("__nexus_get_attr", move |id: i32, name: String| -> String {
            node_id(id)
                .and_then(|id| {
                    attr_state
                        .lock()
                        .ok()
                        .and_then(|state| state.dom.attribute(id, &name).map(str::to_owned))
                })
                .unwrap_or_default()
        })
        .map_err(|error| error.to_string())?;

    let has_attr_state = Arc::clone(state);
    context
        .add_callback("__nexus_has_attr", move |id: i32, name: String| -> bool {
            node_id(id).is_some_and(|id| {
                has_attr_state
                    .lock()
                    .ok()
                    .is_some_and(|state| state.dom.attribute(id, &name).is_some())
            })
        })
        .map_err(|error| error.to_string())?;

    let set_attr_state = Arc::clone(state);
    context
        .add_callback(
            "__nexus_set_attr",
            move |id: i32, name: String, value: String| -> bool {
                let Some(id) = node_id(id) else { return false };
                let Ok(mut state) = set_attr_state.lock() else { return false };
                let changed = state.dom.set_attribute(id, &name, &value);
                state.mark_mutation(changed)
            },
        )
        .map_err(|error| error.to_string())?;

    let remove_attr_state = Arc::clone(state);
    context
        .add_callback("__nexus_remove_attr", move |id: i32, name: String| -> bool {
            let Some(id) = node_id(id) else { return false };
            let Ok(mut state) = remove_attr_state.lock() else { return false };
            let changed = state.dom.remove_attribute(id, &name);
            state.mark_mutation(changed)
        })
        .map_err(|error| error.to_string())?;

    let tag_state = Arc::clone(state);
    context
        .add_callback("__nexus_get_tag", move |id: i32| -> String {
            node_id(id)
                .and_then(|id| {
                    tag_state
                        .lock()
                        .ok()
                        .and_then(|state| state.dom.element_tag_name(id).map(str::to_owned))
                })
                .unwrap_or_default()
        })
        .map_err(|error| error.to_string())?;

    let create_state = Arc::clone(state);
    context
        .add_callback("__nexus_create_element", move |tag: String| -> i32 {
            let Ok(mut state) = create_state.lock() else { return -1 };
            let Some(id) = state.dom.create_element(&tag) else { return -1 };
            state.mutations += 1;
            i32::try_from(id).unwrap_or(-1)
        })
        .map_err(|error| error.to_string())?;

    let append_state = Arc::clone(state);
    context
        .add_callback("__nexus_append_child", move |parent: i32, child: i32| -> bool {
            let (Some(parent), Some(child)) = (node_id(parent), node_id(child)) else {
                return false;
            };
            let Ok(mut state) = append_state.lock() else { return false };
            let changed = state.dom.append_child(parent, child);
            state.mark_mutation(changed)
        })
        .map_err(|error| error.to_string())?;

    let remove_child_state = Arc::clone(state);
    context
        .add_callback("__nexus_remove_child", move |parent: i32, child: i32| -> bool {
            let (Some(parent), Some(child)) = (node_id(parent), node_id(child)) else {
                return false;
            };
            let Ok(mut state) = remove_child_state.lock() else { return false };
            let changed = state.dom.remove_child(parent, child);
            state.mark_mutation(changed)
        })
        .map_err(|error| error.to_string())?;

    let title_state = Arc::clone(state);
    context
        .add_callback("__nexus_get_title", move || -> String {
            title_state
                .lock()
                .ok()
                .and_then(|state| state.dom.title())
                .unwrap_or_default()
        })
        .map_err(|error| error.to_string())?;

    let set_title_state = Arc::clone(state);
    context
        .add_callback("__nexus_set_title", move |value: String| -> bool {
            let Ok(mut state) = set_title_state.lock() else { return false };
            let changed = state.dom.set_title(&value);
            state.mark_mutation(changed)
        })
        .map_err(|error| error.to_string())?;

    let url_state = Arc::clone(state);
    context
        .add_callback("__nexus_document_url", move || -> String {
            url_state
                .lock()
                .ok()
                .map(|state| state.dom.document_url().as_str().to_owned())
                .unwrap_or_default()
        })
        .map_err(|error| error.to_string())?;

    let base_state = Arc::clone(state);
    context
        .add_callback("__nexus_base_url", move || -> String {
            base_state
                .lock()
                .ok()
                .map(|state| state.dom.base_url().as_str().to_owned())
                .unwrap_or_default()
        })
        .map_err(|error| error.to_string())?;

    let navigate_state = Arc::clone(state);
    context
        .add_callback("__nexus_navigate", move |value: String| -> bool {
            let Ok(mut state) = navigate_state.lock() else { return false };
            let base = state.dom.base_url();
            let Some(url) = resolve_url(&base, &value) else { return false };
            state.navigation_request = Some(url);
            true
        })
        .map_err(|error| error.to_string())?;

    let browser_state = network.browser_state();

    let storage_get_state = Arc::clone(state);
    let storage_get_browser = Arc::clone(&browser_state);
    let storage_get_session = session_storage.clone();
    context
        .add_callback(
            "__nexus_storage_get",
            move |kind: String, key: String| -> String {
                let origin = bridge_origin(&storage_get_state);
                let Some(origin) = origin else {
                    return json!({"found": false, "error": "opaque origin"}).to_string();
                };
                let result = if kind == "session" {
                    storage_get_session.get(&origin, &key)
                } else {
                    storage_get_browser.local_get(&origin, &key)
                };
                match result {
                    Ok(Some(value)) => json!({"found": true, "value": value}).to_string(),
                    Ok(None) => json!({"found": false}).to_string(),
                    Err(error) => json!({"found": false, "error": storage_error_name(error)}).to_string(),
                }
            },
        )
        .map_err(|error| error.to_string())?;

    let storage_set_state = Arc::clone(state);
    let storage_set_browser = Arc::clone(&browser_state);
    let storage_set_session = session_storage.clone();
    context
        .add_callback(
            "__nexus_storage_set",
            move |kind: String, key: String, value: String| -> String {
                let Some(origin) = bridge_origin(&storage_set_state) else {
                    return "SecurityError".to_owned();
                };
                let result = if kind == "session" {
                    storage_set_session.set(&origin, &key, &value)
                } else {
                    storage_set_browser.local_set(&origin, &key, &value)
                };
                result.err().map(storage_error_name).unwrap_or_default().to_owned()
            },
        )
        .map_err(|error| error.to_string())?;

    let storage_remove_state = Arc::clone(state);
    let storage_remove_browser = Arc::clone(&browser_state);
    let storage_remove_session = session_storage.clone();
    context
        .add_callback(
            "__nexus_storage_remove",
            move |kind: String, key: String| -> bool {
                let Some(origin) = bridge_origin(&storage_remove_state) else { return false };
                if kind == "session" {
                    storage_remove_session.remove(&origin, &key).is_ok()
                } else {
                    storage_remove_browser.local_remove(&origin, &key).is_ok()
                }
            },
        )
        .map_err(|error| error.to_string())?;

    let storage_clear_state = Arc::clone(state);
    let storage_clear_browser = Arc::clone(&browser_state);
    let storage_clear_session = session_storage.clone();
    context
        .add_callback("__nexus_storage_clear", move |kind: String| -> bool {
            let Some(origin) = bridge_origin(&storage_clear_state) else { return false };
            if kind == "session" {
                storage_clear_session.clear(&origin).is_ok()
            } else {
                storage_clear_browser.local_clear(&origin).is_ok()
            }
        })
        .map_err(|error| error.to_string())?;

    let storage_len_state = Arc::clone(state);
    let storage_len_browser = Arc::clone(&browser_state);
    let storage_len_session = session_storage.clone();
    context
        .add_callback("__nexus_storage_len", move |kind: String| -> i32 {
            let Some(origin) = bridge_origin(&storage_len_state) else { return 0 };
            let len = if kind == "session" {
                storage_len_session.len(&origin)
            } else {
                storage_len_browser.local_len(&origin)
            }
            .unwrap_or(0);
            i32::try_from(len).unwrap_or(i32::MAX)
        })
        .map_err(|error| error.to_string())?;

    let storage_key_state = Arc::clone(state);
    let storage_key_browser = Arc::clone(&browser_state);
    let storage_key_session = session_storage;
    context
        .add_callback(
            "__nexus_storage_key",
            move |kind: String, index: i32| -> String {
                let Some(origin) = bridge_origin(&storage_key_state) else { return String::new() };
                let index = usize::try_from(index.max(0)).unwrap_or(0);
                let value = if kind == "session" {
                    storage_key_session.key(&origin, index)
                } else {
                    storage_key_browser.local_key(&origin, index)
                };
                value.ok().flatten().unwrap_or_default()
            },
        )
        .map_err(|error| error.to_string())?;

    let fetch_state = Arc::clone(state);
    let fetch_network = Mutex::new(network.clone());
    context
        .add_callback(
            "__nexus_fetch",
            move |input: String,
                  method: String,
                  body: String,
                  content_type: String,
                  mode: String|
                  -> String {
                let (base, security) = match fetch_state.lock() {
                    Ok(state) => (state.dom.base_url(), state.security.clone()),
                    Err(_) => return json!({"ok": false, "error": "bridge state unavailable"}).to_string(),
                };
                let Some(url) = resolve_url(&base, &input) else {
                    return json!({"ok": false, "error": "invalid URL"}).to_string();
                };
                if let Ok(mut state) = fetch_state.lock() {
                    state.fetch_requests += 1;
                }
                let fetch_mode = match parse_fetch_mode(&mode) {
                    Ok(value) => value,
                    Err(error) => return json!({"ok": false, "error": error}).to_string(),
                };
                let credentials = match parse_credentials_mode("same-origin") {
                    Ok(value) => value,
                    Err(error) => return json!({"ok": false, "error": error}).to_string(),
                };
                let body_bytes = (!body.is_empty()).then_some(body.as_bytes());
                let content_type = (!content_type.is_empty()).then_some(content_type.as_str());
                let fetch_network = match fetch_network.lock() {
                    Ok(network) => network,
                    Err(_) => return json!({"ok": false, "error": "network client unavailable"}).to_string(),
                };
                match fetch_network.web_request(
                    &security,
                    &url,
                    &method,
                    "*/*",
                    body_bytes,
                    content_type,
                    fetch_mode,
                    credentials,
                ) {
                    Ok(response) => {
                        let text = String::from_utf8_lossy(&response.body).into_owned();
                        json!({
                            "ok": true,
                            "status": response.status,
                            "url": response.final_url.as_str(),
                            "contentType": response.content_type.unwrap_or_default(),
                            "body": text,
                        })
                        .to_string()
                    }
                    Err(error) => json!({"ok": false, "error": error.to_string()}).to_string(),
                }
            },
        )
        .map_err(|error| error.to_string())?;

    let ws_open_state = Arc::clone(state);
    let ws_browser_state = network.browser_state();
    context
        .add_callback("__nexus_ws_open", move |input: String, protocols_json: String| -> String {
            let protocols = serde_json::from_str::<Vec<String>>(&protocols_json).unwrap_or_default();
            let mut state = match ws_open_state.lock() {
                Ok(state) => state,
                Err(_) => return json!({"ok": false, "error": "bridge state unavailable"}).to_string(),
            };
            let page_url = state.security.document_url.clone();
            let mut request = match WebSocketRequest::new(&page_url, &input, protocols) {
                Ok(request) => request,
                Err(error) => return json!({"ok": false, "error": error}).to_string(),
            };
            if let Some(upgraded) = ws_browser_state.hsts_upgrade(&request.url) {
                request.url = upgraded;
            }
            if is_mixed_active_content(&page_url, &request.url) {
                return json!({"ok": false, "error": "mixed-content WebSocket blocked"}).to_string();
            }
            if !state.security.csp.allows_connect_url(&page_url, &request.url) {
                return json!({"ok": false, "error": "CSP connect-src blocked WebSocket"}).to_string();
            }
            let id = NEXT_WEBSOCKET_ID.fetch_add(1, Ordering::Relaxed);
            state.websocket_connections = state.websocket_connections.saturating_add(1);
            state.websocket_commands.push(WebSocketCommand::Open {
                id,
                url: request.url.clone(),
                origin: request.origin_header(),
                protocols: request.protocols.clone(),
            });
            json!({"ok": true, "id": id, "url": request.url.as_str()}).to_string()
        })
        .map_err(|error| error.to_string())?;

    let ws_send_state = Arc::clone(state);
    context
        .add_callback("__nexus_ws_send_text", move |id: i32, text: String| -> bool {
            let Ok(id) = u64::try_from(id) else { return false };
            if let Ok(mut state) = ws_send_state.lock() {
                state.websocket_commands.push(WebSocketCommand::SendText { id, text });
                true
            } else {
                false
            }
        })
        .map_err(|error| error.to_string())?;

    let ws_close_state = Arc::clone(state);
    context
        .add_callback("__nexus_ws_close", move |id: i32, code: i32, reason: String| -> bool {
            let (Ok(id), Ok(code)) = (u64::try_from(id), u16::try_from(code)) else { return false };
            if let Ok(mut state) = ws_close_state.lock() {
                state.websocket_commands.push(WebSocketCommand::Close { id, code, reason });
                true
            } else {
                false
            }
        })
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn node_id(id: i32) -> Option<NodeId> {
    usize::try_from(id).ok()
}

fn bridge_origin(state: &Arc<Mutex<BridgeState>>) -> Option<Origin> {
    state
        .lock()
        .ok()
        .map(|state| Origin::from_url(state.dom.document_url()))
        .filter(|origin| !matches!(origin, Origin::Opaque))
}

fn storage_error_name(error: StorageError) -> &'static str {
    match error {
        StorageError::QuotaExceeded => "QuotaExceededError",
        StorageError::OpaqueOrigin => "SecurityError",
    }
}

fn parse_fetch_mode(value: &str) -> Result<FetchMode, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "cors" => Ok(FetchMode::Cors),
        "same-origin" => Ok(FetchMode::SameOrigin),
        "no-cors" => Err("fetch mode 'no-cors' is not implemented in Nexus 0.20".to_owned()),
        other => Err(format!("unsupported fetch mode: {other}")),
    }
}

fn parse_credentials_mode(value: &str) -> Result<CredentialsMode, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "same-origin" => Ok(CredentialsMode::SameOrigin),
        "omit" => Ok(CredentialsMode::Omit),
        "include" => Ok(CredentialsMode::Include),
        other => Err(format!("unsupported credentials mode: {other}")),
    }
}

fn js_value_to_string(value: &OwnedJsValue) -> String {
    value
        .to_string()
        .or_else(|_| value.to_json_string(0))
        .unwrap_or_else(|_| "[unprintable]".to_owned())
}

fn load_script_source(
    script: &ScriptReference,
    network: &NetworkClient,
    security: &PageSecurityContext,
    report: &mut JavascriptReport,
) -> Option<(String, bool)> {
    if let Some(src) = &script.src {
        let Some(url) = &script.resolved_url else {
            report.warnings.push(format!(
                "script #{} has unresolved src='{}'",
                script.node_id, src
            ));
            return None;
        };
        match network.fetch_subresource(
            security,
            url,
            "text/javascript,application/javascript,application/ecmascript,text/ecmascript,*/*;q=0.1",
            SubresourceKind::Script,
        ) {
            Ok(response) => {
                if !(200..400).contains(&response.status) {
                    report.warnings.push(format!(
                        "external script {} returned HTTP {}",
                        response.final_url, response.status
                    ));
                    return None;
                }
                let decoded = decode_script(&response.body, response.content_type.as_deref());
                Some((decoded.text, true))
            }
            Err(error) => {
                report
                    .warnings
                    .push(format!("external script {url} failed: {error}"));
                None
            }
        }
    } else {
        Some((script.inline_code.clone(), false))
    }
}

fn is_classic_javascript_type(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        ""
            | "text/javascript"
            | "application/javascript"
            | "application/ecmascript"
            | "text/ecmascript"
            | "application/x-javascript"
            | "text/jscript"
            | "text/livescript"
    )
}

thread_local! {
    static JS_DEADLINE: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// QuickJS calls this hook periodically while executing bytecode. The handler
/// only reads thread-local state and never dereferences the opaque pointer.
#[allow(unsafe_code)]
pub(crate) unsafe extern "C" fn nexus_interrupt_handler(
    _runtime: *mut JSRuntime,
    _opaque: *mut c_void,
) -> c_int {
    JS_DEADLINE.with(|deadline| {
        if deadline
            .get()
            .is_some_and(|limit| Instant::now() >= limit)
        {
            1
        } else {
            0
        }
    })
}

pub(crate) fn with_js_deadline<T>(timeout: Duration, operation: impl FnOnce() -> T) -> T {
    struct ResetDeadline(Option<Instant>);
    impl Drop for ResetDeadline {
        fn drop(&mut self) {
            JS_DEADLINE.with(|deadline| deadline.set(self.0));
        }
    }

    let previous =
        JS_DEADLINE.with(|deadline| deadline.replace(Some(Instant::now() + timeout)));
    let _reset = ResetDeadline(previous);
    operation()
}

const WEB_API_BOOTSTRAP: &str = r#"
(() => {
  'use strict';

  const nodeCache = new Map();
  const documentListeners = new Map();
  const windowListeners = new Map();
  const elementListeners = new Map();
  let nextTimerId = 1;
  const timers = [];

  function listeners(map, type) {
    let list = map.get(type);
    if (!list) {
      list = [];
      map.set(type, list);
    }
    return list;
  }

  function addListener(map, type, callback) {
    if (typeof callback !== 'function') return;
    listeners(map, String(type)).push(callback);
  }

  function removeListener(map, type, callback) {
    const list = map.get(String(type));
    if (!list) return;
    const index = list.indexOf(callback);
    if (index >= 0) list.splice(index, 1);
  }

  function makeEvent(type, target) {
    return {
      type: String(type),
      target,
      currentTarget: target,
      bubbles: true,
      cancelable: true,
      defaultPrevented: false,
      propagationStopped: false,
      preventDefault() { this.defaultPrevented = true; },
      stopPropagation() { this.propagationStopped = true; },
    };
  }

  function runElementHandlers(element, event) {
    const type = String(event.type);
    const property = element['on' + type];
    if (typeof property === 'function') property.call(element, event);
    const inline = element.getAttribute('on' + type);
    if (inline) {
      try { Function('event', inline).call(element, event); }
      catch (error) { console.error(error); }
    }
    const map = elementListeners.get(element.__nexusNodeId);
    if (map) {
      for (const callback of [...(map.get(type) || [])]) {
        callback.call(element, event);
        if (event.propagationStopped) break;
      }
    }
  }

  class NexusElement {
    constructor(id) { this.__nexusNodeId = id; }
    get nodeType() { return 1; }
    get tagName() { return __nexus_get_tag(this.__nexusNodeId).toUpperCase(); }
    get parentElement() { return wrapNode(__nexus_parent_element(this.__nexusNodeId)); }
    get textContent() { return __nexus_get_text(this.__nexusNodeId); }
    set textContent(value) { __nexus_set_text(this.__nexusNodeId, String(value ?? '')); }
    get innerText() { return this.textContent; }
    set innerText(value) { this.textContent = value; }
    get id() { return __nexus_get_attr(this.__nexusNodeId, 'id'); }
    set id(value) { __nexus_set_attr(this.__nexusNodeId, 'id', String(value)); }
    get className() { return __nexus_get_attr(this.__nexusNodeId, 'class'); }
    set className(value) { __nexus_set_attr(this.__nexusNodeId, 'class', String(value)); }
    get href() { return __nexus_get_attr(this.__nexusNodeId, 'href'); }
    set href(value) { __nexus_set_attr(this.__nexusNodeId, 'href', String(value)); }
    get value() {
      if (this.tagName === 'TEXTAREA') return this.textContent;
      return __nexus_get_attr(this.__nexusNodeId, 'value');
    }
    set value(value) {
      value = String(value ?? '');
      if (this.tagName === 'TEXTAREA') this.textContent = value;
      else __nexus_set_attr(this.__nexusNodeId, 'value', value);
    }
    get checked() { return __nexus_has_attr(this.__nexusNodeId, 'checked'); }
    set checked(value) {
      if (value) __nexus_set_attr(this.__nexusNodeId, 'checked', '');
      else __nexus_remove_attr(this.__nexusNodeId, 'checked');
    }
    get selected() { return __nexus_has_attr(this.__nexusNodeId, 'selected'); }
    set selected(value) {
      if (value) __nexus_set_attr(this.__nexusNodeId, 'selected', '');
      else __nexus_remove_attr(this.__nexusNodeId, 'selected');
    }
    getAttribute(name) {
      name = String(name);
      return __nexus_has_attr(this.__nexusNodeId, name)
        ? __nexus_get_attr(this.__nexusNodeId, name)
        : null;
    }
    hasAttribute(name) { return __nexus_has_attr(this.__nexusNodeId, String(name)); }
    setAttribute(name, value) {
      __nexus_set_attr(this.__nexusNodeId, String(name), String(value));
    }
    removeAttribute(name) { __nexus_remove_attr(this.__nexusNodeId, String(name)); }
    appendChild(child) {
      if (!(child instanceof NexusElement)) throw new TypeError('appendChild requires a NexusElement');
      __nexus_append_child(this.__nexusNodeId, child.__nexusNodeId);
      return child;
    }
    removeChild(child) {
      if (!(child instanceof NexusElement)) throw new TypeError('removeChild requires a NexusElement');
      __nexus_remove_child(this.__nexusNodeId, child.__nexusNodeId);
      return child;
    }
    addEventListener(type, callback) {
      const id = this.__nexusNodeId;
      let map = elementListeners.get(id);
      if (!map) { map = new Map(); elementListeners.set(id, map); }
      addListener(map, type, callback);
    }
    removeEventListener(type, callback) {
      const map = elementListeners.get(this.__nexusNodeId);
      if (map) removeListener(map, type, callback);
    }
    dispatchEvent(event) {
      const type = typeof event === 'string' ? event : event?.type;
      if (!type) return true;
      return !globalThis.__nexusDispatchElementEvent(this.__nexusNodeId, String(type));
    }
    click() { return this.dispatchEvent('click'); }
    toString() { return `[object ${this.tagName || 'HTMLElement'}]`; }
  }

  function wrapNode(id) {
    if (typeof id !== 'number' || id < 0) return null;
    if (!nodeCache.has(id)) nodeCache.set(id, new NexusElement(id));
    return nodeCache.get(id);
  }

  const documentObject = {
    nodeType: 9,
    readyState: 'loading',
    querySelector(selector) { return wrapNode(__nexus_query(String(selector))); },
    getElementById(id) { return wrapNode(__nexus_get_element_by_id(String(id))); },
    addEventListener(type, callback) { addListener(documentListeners, type, callback); },
    removeEventListener(type, callback) { removeListener(documentListeners, type, callback); },
    createElement(tagName) { return wrapNode(__nexus_create_element(String(tagName))); },
  };

  Object.defineProperties(documentObject, {
    title: {
      enumerable: true,
      get: () => __nexus_get_title(),
      set: value => { __nexus_set_title(String(value)); },
    },
    URL: { enumerable: true, get: () => __nexus_document_url() },
    documentURI: { enumerable: true, get: () => __nexus_document_url() },
    baseURI: { enumerable: true, get: () => __nexus_base_url() },
    body: { enumerable: true, get: () => wrapNode(__nexus_first_element('body')) },
    head: { enumerable: true, get: () => wrapNode(__nexus_first_element('head')) },
    documentElement: { enumerable: true, get: () => wrapNode(__nexus_first_element('html')) },
  });

  const locationObject = {
    assign(value) { __nexus_navigate(String(value)); },
    replace(value) { __nexus_navigate(String(value)); },
    reload() { __nexus_navigate(__nexus_document_url()); },
    toString() { return this.href; },
  };
  Object.defineProperty(locationObject, 'href', {
    enumerable: true,
    get: () => __nexus_document_url(),
    set: value => { __nexus_navigate(String(value)); },
  });

  globalThis.window = globalThis;
  globalThis.self = globalThis;
  globalThis.document = documentObject;
  Object.defineProperty(globalThis, 'location', {
    configurable: false,
    enumerable: true,
    get: () => locationObject,
    set: value => { locationObject.href = value; },
  });
  documentObject.location = locationObject;

  globalThis.navigator = Object.freeze({
    userAgent: 'NexusEngine/0.20 QuickJS-ng',
    platform: 'Nexus',
    language: 'en-US',
  });
  globalThis.innerWidth = __nexus_inner_width;
  globalThis.innerHeight = __nexus_inner_height;

  globalThis.addEventListener = (type, callback) => addListener(windowListeners, type, callback);
  globalThis.removeEventListener = (type, callback) => removeListener(windowListeners, type, callback);

  globalThis.setTimeout = (callback, delay = 0, ...args) => {
    if (typeof callback !== 'function') throw new TypeError('setTimeout callback must be a function');
    const id = nextTimerId++;
    timers.push({ id, callback, args, cancelled: false, due: Date.now() + Math.max(0, Number(delay) || 0), interval: 0 });
    return id;
  };
  globalThis.clearTimeout = id => {
    const timer = timers.find(item => item.id === Number(id));
    if (timer) timer.cancelled = true;
  };
  globalThis.setInterval = (callback, delay = 0, ...args) => {
    if (typeof callback !== 'function') throw new TypeError('setInterval callback must be a function');
    const id = nextTimerId++;
    const interval = Math.max(1, Number(delay) || 1);
    timers.push({ id, callback, args, cancelled: false, due: Date.now() + interval, interval });
    return id;
  };
  globalThis.clearInterval = globalThis.clearTimeout;

  globalThis.__nexusDrainDueTimers = now => {
    let count = 0;
    let safety = 0;
    timers.sort((a, b) => a.due - b.due);
    while (timers.length && safety++ < 256) {
      const timer = timers[0];
      if (timer.due > Number(now)) break;
      timers.shift();
      if (timer.cancelled) continue;
      timer.callback(...timer.args);
      count++;
      if (timer.interval > 0 && !timer.cancelled) {
        timer.due = Number(now) + timer.interval;
        timers.push(timer);
        timers.sort((a, b) => a.due - b.due);
      }
    }
    return count;
  };

  globalThis.__nexusNextTimerDelay = now => {
    const active = timers.filter(timer => !timer.cancelled);
    if (!active.length) return -1;
    return Math.max(0, Math.min(...active.map(timer => timer.due)) - Number(now));
  };

  globalThis.__nexusDispatchElementEvent = (nodeId, type) => {
    const target = wrapNode(Number(nodeId));
    if (!target) return false;
    const event = makeEvent(String(type), target);
    let current = target;
    while (current) {
      event.currentTarget = current;
      runElementHandlers(current, event);
      if (event.propagationStopped) break;
      current = current.parentElement;
    }
    return event.defaultPrevented;
  };

  globalThis.__nexusSetControlValue = (nodeId, value) => {
    const element = wrapNode(Number(nodeId));
    if (element) element.value = value;
  };

  globalThis.__nexusSetControlChecked = (nodeId, checked) => {
    const element = wrapNode(Number(nodeId));
    if (element) element.checked = Boolean(checked);
  };

  globalThis.__nexusDispatchDocumentEvent = type => {
    const docEvent = makeEvent(type, documentObject);
    for (const callback of [...(documentListeners.get(String(type)) || [])]) {
      callback.call(documentObject, docEvent);
    }
    const winEvent = makeEvent(type, globalThis);
    for (const callback of [...(windowListeners.get(String(type)) || [])]) {
      callback.call(globalThis, winEvent);
    }
    if (type === 'DOMContentLoaded') documentObject.readyState = 'interactive';
    if (type === 'load') documentObject.readyState = 'complete';
  };

  class NexusStorage {
    constructor(kind) { this._kind = kind; }
    get length() { return __nexus_storage_len(this._kind); }
    key(index) {
      const value = __nexus_storage_key(this._kind, Number(index) | 0);
      return value === '' ? null : value;
    }
    getItem(key) {
      const payload = JSON.parse(__nexus_storage_get(this._kind, String(key)));
      if (payload.error === 'opaque origin') {
        const error = new Error('Web Storage is unavailable for opaque origins');
        error.name = 'SecurityError';
        throw error;
      }
      return payload.found ? String(payload.value) : null;
    }
    setItem(key, value) {
      const errorName = __nexus_storage_set(this._kind, String(key), String(value));
      if (errorName) {
        const error = new Error(errorName === 'QuotaExceededError' ? 'Web Storage quota exceeded' : 'Web Storage unavailable');
        error.name = errorName;
        throw error;
      }
    }
    removeItem(key) { __nexus_storage_remove(this._kind, String(key)); }
    clear() { __nexus_storage_clear(this._kind); }
  }

  Object.defineProperty(globalThis, 'localStorage', {
    value: new NexusStorage('local'), enumerable: true
  });
  Object.defineProperty(globalThis, 'sessionStorage', {
    value: new NexusStorage('session'), enumerable: true
  });

  class NexusResponse {
    constructor(payload) {
      this.ok = payload.status >= 200 && payload.status < 300;
      this.status = payload.status || 0;
      this.url = payload.url || '';
      this.headers = Object.freeze({ get(name) { return String(name).toLowerCase() === 'content-type' ? (payload.contentType || null) : null; } });
      this._body = payload.body || '';
    }
    text() { return Promise.resolve(this._body); }
    json() { return Promise.resolve(JSON.parse(this._body)); }
  }

  globalThis.fetch = (input, init = {}) => {
    return new Promise((resolve, reject) => {
      try {
        const method = String(init.method || 'GET').toUpperCase();
        const body = init.body == null ? '' : String(init.body);
        let contentType = '';
        if (init.headers && typeof init.headers === 'object') {
          contentType = String(init.headers['Content-Type'] || init.headers['content-type'] || '');
        }
        const mode = String(init.mode || 'cors');
        const credentials = String(init.credentials || 'same-origin');
        const payload = JSON.parse(__nexus_fetch(String(input), method, body, contentType, mode));
        if (!payload.ok && payload.error) {
          reject(new TypeError(payload.error));
          return;
        }
        resolve(new NexusResponse(payload));
      } catch (error) { reject(error); }
    });
  };

  const websocketObjects = new Map();
  class NexusWebSocket {
    static CONNECTING = 0;
    static OPEN = 1;
    static CLOSING = 2;
    static CLOSED = 3;

    constructor(url, protocols = []) {
      const list = Array.isArray(protocols) ? protocols.map(String) : (protocols ? [String(protocols)] : []);
      const payload = JSON.parse(__nexus_ws_open(String(url), JSON.stringify(list)));
      if (!payload.ok) { const error = new Error(payload.error || 'WebSocket blocked'); error.name = 'SecurityError'; throw error; }
      this._id = Number(payload.id);
      this.url = String(payload.url || url);
      this.protocol = '';
      this.extensions = '';
      this.readyState = NexusWebSocket.CONNECTING;
      this.bufferedAmount = 0;
      this.binaryType = 'blob';
      this.onopen = null;
      this.onmessage = null;
      this.onerror = null;
      this.onclose = null;
      this._listeners = new Map();
      websocketObjects.set(this._id, this);
    }

    addEventListener(type, callback) {
      if (typeof callback !== 'function') return;
      const key = String(type);
      const list = this._listeners.get(key) || [];
      list.push(callback);
      this._listeners.set(key, list);
    }
    removeEventListener(type, callback) {
      const key = String(type);
      const list = this._listeners.get(key) || [];
      this._listeners.set(key, list.filter(item => item !== callback));
    }
    _dispatch(type, event) {
      const handler = this['on' + type];
      if (typeof handler === 'function') handler.call(this, event);
      for (const callback of [...(this._listeners.get(type) || [])]) callback.call(this, event);
    }
    send(data) {
      if (this.readyState !== NexusWebSocket.OPEN) throw new Error('WebSocket is not open');
      if (!__nexus_ws_send_text(this._id, String(data))) throw new Error('WebSocket send failed');
    }
    close(code = 1000, reason = '') {
      if (this.readyState === NexusWebSocket.CLOSED || this.readyState === NexusWebSocket.CLOSING) return;
      this.readyState = NexusWebSocket.CLOSING;
      __nexus_ws_close(this._id, Number(code) | 0, String(reason));
    }
  }

  globalThis.__nexusWsDeliver = (id, type, payload = {}) => {
    const socket = websocketObjects.get(Number(id));
    if (!socket) return;
    if (type === 'open') {
      socket.readyState = NexusWebSocket.OPEN;
      socket.protocol = payload.protocol || '';
      socket._dispatch('open', { type: 'open', target: socket, currentTarget: socket });
      return;
    }
    if (type === 'message') {
      const data = payload.binary ? new Uint8Array(payload.data || []) : String(payload.data || '');
      socket._dispatch('message', { type: 'message', data, target: socket, currentTarget: socket });
      return;
    }
    if (type === 'error') {
      socket._dispatch('error', { type: 'error', message: String(payload.message || ''), target: socket, currentTarget: socket });
      return;
    }
    if (type === 'close') {
      socket.readyState = NexusWebSocket.CLOSED;
      socket._dispatch('close', { type: 'close', code: Number(payload.code || 1000), reason: String(payload.reason || ''), wasClean: Number(payload.code || 1000) !== 1006, target: socket, currentTarget: socket });
      websocketObjects.delete(Number(id));
    }
  };

  globalThis.WebSocket = NexusWebSocket;
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_html;

    #[test]
    fn classic_script_types_are_recognized() {
        assert!(is_classic_javascript_type(""));
        assert!(is_classic_javascript_type("text/javascript"));
        assert!(!is_classic_javascript_type("application/json"));
        assert!(!is_classic_javascript_type("module"));
    }

    #[test]
    fn bridge_state_tracks_dom_mutation() {
        let dom = parse_html(
            Url::parse("https://example.com/").unwrap(),
            r#"<p id="target">before</p>"#,
        );
        let mut state = BridgeState::new(dom.clone(), PageSecurityContext::permissive(dom.document_url().clone()));
        let id = state.dom.query_selector("#target").unwrap();
        let changed = state.dom.set_text_content(id, "after");
        assert!(state.mark_mutation(changed));
        assert_eq!(state.mutations, 1);
        assert_eq!(state.dom.text_content(id), "after");
    }
}
