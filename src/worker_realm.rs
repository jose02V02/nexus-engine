//! Independent QuickJS Worker realms with bounded structured messaging.

use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use quickjs_rusty::Context;
use serde_json::Value;

use crate::javascript::{nexus_interrupt_handler, with_js_deadline};

#[derive(Debug, Clone, PartialEq)]
pub enum WorkerRealmEvent {
    Ready,
    Message(Value),
    Error(String),
    Checkpoint(WorkerRealmStats),
    Closed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkerRealmStats {
    pub messages_received: usize,
    pub messages_posted: usize,
    pub timer_callbacks: usize,
    pub microtask_checkpoints: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerRealmError { SpawnFailed, Closed, Backpressure }

enum WorkerRealmCommand { Message(Value), Checkpoint, Terminate }

pub struct JavaScriptWorker {
    commands: SyncSender<WorkerRealmCommand>,
    events: Receiver<WorkerRealmEvent>,
    thread: Option<JoinHandle<()>>,
}

impl JavaScriptWorker {
    pub fn spawn(source: impl Into<String>, capacity: usize) -> Result<Self, WorkerRealmError> {
        Self::spawn_with_limits(source, capacity, 16 * 1024 * 1024, 1024 * 1024, Duration::from_millis(500))
    }

    pub fn spawn_with_limits(
        source: impl Into<String>,
        capacity: usize,
        memory_limit: usize,
        stack_limit: usize,
        execution_timeout: Duration,
    ) -> Result<Self, WorkerRealmError> {
        let source = source.into();
        let (command_tx, command_rx) = mpsc::sync_channel(capacity.max(1));
        let (event_tx, event_rx) = mpsc::channel();
        let thread = thread::Builder::new().name("nexus-js-worker".to_owned()).spawn(move || {
            run_worker_realm(source, command_rx, event_tx, memory_limit.max(1024 * 1024), stack_limit.max(256 * 1024), execution_timeout.max(Duration::from_millis(10)));
        }).map_err(|_| WorkerRealmError::SpawnFailed)?;
        Ok(Self { commands: command_tx, events: event_rx, thread: Some(thread) })
    }

    pub fn post_message(&self, message: Value) -> Result<(), WorkerRealmError> {
        self.send(WorkerRealmCommand::Message(message))
    }

    pub fn checkpoint(&self) -> Result<(), WorkerRealmError> { self.send(WorkerRealmCommand::Checkpoint) }

    pub fn try_event(&self) -> Option<WorkerRealmEvent> {
        match self.events.try_recv() { Ok(event) => Some(event), Err(TryRecvError::Empty | TryRecvError::Disconnected) => None }
    }

    pub fn terminate(&mut self) {
        let _ = self.commands.send(WorkerRealmCommand::Terminate);
        if let Some(thread) = self.thread.take() { let _ = thread.join(); }
    }

    fn send(&self, command: WorkerRealmCommand) -> Result<(), WorkerRealmError> {
        self.commands.try_send(command).map_err(|error| match error { TrySendError::Full(_) => WorkerRealmError::Backpressure, TrySendError::Disconnected(_) => WorkerRealmError::Closed })
    }
}

impl Drop for JavaScriptWorker { fn drop(&mut self) { self.terminate(); } }

fn run_worker_realm(
    source: String,
    commands: Receiver<WorkerRealmCommand>,
    events: mpsc::Sender<WorkerRealmEvent>,
    memory_limit: usize,
    stack_limit: usize,
    execution_timeout: Duration,
) {
    let outgoing = Arc::new(Mutex::new(Vec::<String>::new()));
    let callback_outgoing = Arc::clone(&outgoing);
    let context = match Context::builder().memory_limit(memory_limit).build() {
        Ok(context) => context,
        Err(error) => { let _ = events.send(WorkerRealmEvent::Error(error.to_string())); let _ = events.send(WorkerRealmEvent::Closed); return; }
    };
    context.set_max_stack_size(stack_limit);
    context.set_interrupt_handler(Some(nexus_interrupt_handler), std::ptr::null_mut());
    if let Err(error) = context.add_callback("__nexusWorkerPost", move |message: String| -> bool {
        callback_outgoing.lock().map(|mut queue| { queue.push(message); true }).unwrap_or(false)
    }) {
        let _ = events.send(WorkerRealmEvent::Error(error.to_string())); let _ = events.send(WorkerRealmEvent::Closed); return;
    }
    context.update_stack_top();
    if let Err(error) = with_js_deadline(execution_timeout, || context.eval(WORKER_BOOTSTRAP, false)) {
        let _ = events.send(WorkerRealmEvent::Error(error.to_string())); let _ = events.send(WorkerRealmEvent::Closed); return;
    }
    if let Err(error) = with_js_deadline(execution_timeout, || context.eval(&source, false)) {
        let _ = events.send(WorkerRealmEvent::Error(error.to_string())); let _ = events.send(WorkerRealmEvent::Closed); return;
    }
    let mut stats = WorkerRealmStats::default();
    let _ = events.send(WorkerRealmEvent::Ready);
    drain_posted_messages(&outgoing, &events, &mut stats);
    while let Ok(command) = commands.recv() {
        match command {
            WorkerRealmCommand::Message(message) => {
                stats.messages_received = stats.messages_received.saturating_add(1);
                let code = format!("globalThis.__nexusWorkerDeliver({message});");
                if let Err(error) = with_js_deadline(execution_timeout, || context.eval(&code, false)) { let _ = events.send(WorkerRealmEvent::Error(error.to_string())); }
                run_pending_job(&context, execution_timeout, &events, &mut stats);
            }
            WorkerRealmCommand::Checkpoint => {
                match with_js_deadline(execution_timeout, || context.eval("globalThis.__nexusWorkerDrainTimers(Date.now())", false)) {
                    Ok(value) => if let Ok(count) = value.to_int() { stats.timer_callbacks = stats.timer_callbacks.saturating_add(usize::try_from(count.max(0)).unwrap_or(0)); },
                    Err(error) => { let _ = events.send(WorkerRealmEvent::Error(error.to_string())); }
                }
                run_pending_job(&context, execution_timeout, &events, &mut stats);
                let _ = events.send(WorkerRealmEvent::Checkpoint(stats.clone()));
            }
            WorkerRealmCommand::Terminate => break,
        }
        drain_posted_messages(&outgoing, &events, &mut stats);
    }
    let _ = events.send(WorkerRealmEvent::Closed);
}

fn run_pending_job(context: &Context, timeout: Duration, events: &mpsc::Sender<WorkerRealmEvent>, stats: &mut WorkerRealmStats) {
    match with_js_deadline(timeout, || context.execute_pending_job()) {
        Ok(()) => stats.microtask_checkpoints = stats.microtask_checkpoints.saturating_add(1),
        Err(error) => { let _ = events.send(WorkerRealmEvent::Error(error.to_string())); }
    }
}

fn drain_posted_messages(outgoing: &Arc<Mutex<Vec<String>>>, events: &mpsc::Sender<WorkerRealmEvent>, stats: &mut WorkerRealmStats) {
    let messages = outgoing.lock().map(|mut queue| std::mem::take(&mut *queue)).unwrap_or_default();
    for message in messages {
        match serde_json::from_str(&message) {
            Ok(value) => { stats.messages_posted = stats.messages_posted.saturating_add(1); let _ = events.send(WorkerRealmEvent::Message(value)); }
            Err(error) => { let _ = events.send(WorkerRealmEvent::Error(format!("worker postMessage serialization failed: {error}"))); }
        }
    }
}

const WORKER_BOOTSTRAP: &str = r#"
(() => {
  'use strict';
  const listeners = [];
  const timers = [];
  let nextTimerId = 1;
  globalThis.self = globalThis;
  globalThis.onmessage = null;
  globalThis.postMessage = value => __nexusWorkerPost(JSON.stringify(value));
  globalThis.addEventListener = (type, callback) => {
    if (type === 'message' && typeof callback === 'function') listeners.push(callback);
  };
  globalThis.removeEventListener = (type, callback) => {
    if (type !== 'message') return;
    const index = listeners.indexOf(callback);
    if (index >= 0) listeners.splice(index, 1);
  };
  globalThis.setTimeout = (callback, delay = 0, ...args) => {
    if (typeof callback !== 'function') throw new TypeError('callback must be a function');
    const id = nextTimerId++;
    timers.push({ id, callback, args, due: Date.now() + Math.max(0, Number(delay) || 0), cancelled: false });
    return id;
  };
  globalThis.clearTimeout = id => {
    const timer = timers.find(candidate => candidate.id === Number(id));
    if (timer) timer.cancelled = true;
  };
  globalThis.__nexusWorkerDeliver = data => {
    const event = Object.freeze({ type: 'message', data });
    if (typeof globalThis.onmessage === 'function') globalThis.onmessage(event);
    for (const callback of [...listeners]) callback(event);
  };
  globalThis.__nexusWorkerDrainTimers = now => {
    let count = 0;
    for (const timer of timers) {
      if (!timer.cancelled && timer.due <= now) { timer.cancelled = true; timer.callback(...timer.args); count++; }
    }
    return count;
  };
})();
"#;
