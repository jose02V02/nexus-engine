//! Shared memory, Atomics and bounded Worker messaging.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const MAX_SHARED_BUFFER_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConcurrencyError {
    CrossOriginIsolationRequired,
    InvalidByteLength,
    IndexOutOfBounds,
    WorkerClosed,
    WorkerBackpressure,
}

struct SharedMemory {
    cells: Vec<AtomicI32>,
    waits: Mutex<HashMap<usize, Arc<WaitCell>>>,
}

struct WaitCell {
    generation: Mutex<u64>,
    changed: Condvar,
    waiters: AtomicUsize,
}

#[derive(Clone)]
pub struct SharedArrayBuffer { inner: Arc<SharedMemory> }

impl std::fmt::Debug for SharedArrayBuffer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("SharedArrayBuffer").field("byte_length", &self.byte_length()).finish()
    }
}

impl SharedArrayBuffer {
    pub fn new(byte_length: usize, cross_origin_isolated: bool) -> Result<Self, ConcurrencyError> {
        if !cross_origin_isolated { return Err(ConcurrencyError::CrossOriginIsolationRequired); }
        if byte_length == 0 || byte_length > MAX_SHARED_BUFFER_BYTES || byte_length % 4 != 0 { return Err(ConcurrencyError::InvalidByteLength); }
        let cells = (0..byte_length / 4).map(|_| AtomicI32::new(0)).collect();
        Ok(Self { inner: Arc::new(SharedMemory { cells, waits: Mutex::new(HashMap::new()) }) })
    }

    #[must_use] pub fn byte_length(&self) -> usize { self.inner.cells.len() * 4 }
    #[must_use] pub fn len_i32(&self) -> usize { self.inner.cells.len() }
    pub fn waiter_count(&self, index: usize) -> Result<usize, ConcurrencyError> {
        self.cell(index)?;
        let waits = self.inner.waits.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        Ok(waits.get(&index).map_or(0, |wait| wait.waiters.load(Ordering::SeqCst)))
    }

    pub fn load_i32(&self, index: usize) -> Result<i32, ConcurrencyError> { Ok(self.cell(index)?.load(Ordering::SeqCst)) }
    pub fn store_i32(&self, index: usize, value: i32) -> Result<i32, ConcurrencyError> { self.cell(index)?.store(value, Ordering::SeqCst); Ok(value) }
    pub fn add_i32(&self, index: usize, value: i32) -> Result<i32, ConcurrencyError> { Ok(self.cell(index)?.fetch_add(value, Ordering::SeqCst)) }
    pub fn sub_i32(&self, index: usize, value: i32) -> Result<i32, ConcurrencyError> { Ok(self.cell(index)?.fetch_sub(value, Ordering::SeqCst)) }
    pub fn and_i32(&self, index: usize, value: i32) -> Result<i32, ConcurrencyError> { Ok(self.cell(index)?.fetch_and(value, Ordering::SeqCst)) }
    pub fn or_i32(&self, index: usize, value: i32) -> Result<i32, ConcurrencyError> { Ok(self.cell(index)?.fetch_or(value, Ordering::SeqCst)) }
    pub fn xor_i32(&self, index: usize, value: i32) -> Result<i32, ConcurrencyError> { Ok(self.cell(index)?.fetch_xor(value, Ordering::SeqCst)) }
    pub fn exchange_i32(&self, index: usize, value: i32) -> Result<i32, ConcurrencyError> { Ok(self.cell(index)?.swap(value, Ordering::SeqCst)) }
    pub fn compare_exchange_i32(&self, index: usize, expected: i32, replacement: i32) -> Result<i32, ConcurrencyError> {
        let cell = self.cell(index)?;
        Ok(cell.compare_exchange(expected, replacement, Ordering::SeqCst, Ordering::SeqCst).unwrap_or_else(|actual| actual))
    }

    pub fn wait_i32(&self, index: usize, expected: i32, timeout: Option<Duration>) -> Result<AtomicWaitResult, ConcurrencyError> {
        let cell = self.cell(index)?;
        if cell.load(Ordering::SeqCst) != expected { return Ok(AtomicWaitResult::NotEqual); }
        let wait = self.wait_cell(index);
        wait.waiters.fetch_add(1, Ordering::SeqCst);
        let guard = wait.generation.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let observed = *guard;
        if cell.load(Ordering::SeqCst) != expected {
            wait.waiters.fetch_sub(1, Ordering::SeqCst);
            return Ok(AtomicWaitResult::NotEqual);
        }
        let timed_out = if let Some(timeout) = timeout {
            let (_, status) = wait.changed.wait_timeout_while(guard, timeout, |generation| *generation == observed).unwrap_or_else(|poisoned| poisoned.into_inner());
            status.timed_out()
        } else {
            drop(wait.changed.wait_while(guard, |generation| *generation == observed).unwrap_or_else(|poisoned| poisoned.into_inner()));
            false
        };
        wait.waiters.fetch_sub(1, Ordering::SeqCst);
        Ok(if timed_out { AtomicWaitResult::TimedOut } else { AtomicWaitResult::Ok })
    }

    pub fn notify_i32(&self, index: usize, count: usize) -> Result<usize, ConcurrencyError> {
        self.cell(index)?;
        if count == 0 { return Ok(0); }
        let wait = self.wait_cell(index);
        let waiting = wait.waiters.load(Ordering::SeqCst);
        if waiting == 0 { return Ok(0); }
        let mut generation = wait.generation.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        *generation = generation.wrapping_add(1);
        drop(generation);
        let awakened = waiting.min(count);
        for _ in 0..awakened { wait.changed.notify_one(); }
        Ok(awakened)
    }

    fn cell(&self, index: usize) -> Result<&AtomicI32, ConcurrencyError> { self.inner.cells.get(index).ok_or(ConcurrencyError::IndexOutOfBounds) }

    fn wait_cell(&self, index: usize) -> Arc<WaitCell> {
        let mut waits = self.inner.waits.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        waits.entry(index).or_insert_with(|| Arc::new(WaitCell { generation: Mutex::new(0), changed: Condvar::new(), waiters: AtomicUsize::new(0) })).clone()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicWaitResult { Ok, NotEqual, TimedOut }

#[derive(Debug, Clone)]
pub enum StructuredCloneValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<StructuredCloneValue>),
    Object(BTreeMap<String, StructuredCloneValue>),
    SharedBuffer(SharedArrayBuffer),
}

#[derive(Debug, Clone)]
enum WorkerCommand { Message(StructuredCloneValue), Shutdown }

#[derive(Debug, Clone)]
pub enum ConcurrentWorkerEvent { Ready, Message(StructuredCloneValue), Closed }

pub struct ConcurrentWorker {
    commands: SyncSender<WorkerCommand>,
    events: Receiver<ConcurrentWorkerEvent>,
    thread: Option<JoinHandle<()>>,
}

impl ConcurrentWorker {
    pub fn spawn<F>(capacity: usize, mut handler: F) -> Result<Self, ConcurrencyError>
    where F: FnMut(StructuredCloneValue) -> Option<StructuredCloneValue> + Send + 'static {
        let (command_tx, command_rx) = mpsc::sync_channel(capacity.max(1));
        let (event_tx, event_rx) = mpsc::channel();
        let thread = thread::Builder::new().name("nexus-concurrent-worker".to_owned()).spawn(move || {
            let _ = event_tx.send(ConcurrentWorkerEvent::Ready);
            while let Ok(command) = command_rx.recv() {
                match command {
                    WorkerCommand::Message(message) => if let Some(response) = handler(message) { let _ = event_tx.send(ConcurrentWorkerEvent::Message(response)); },
                    WorkerCommand::Shutdown => break,
                }
            }
            let _ = event_tx.send(ConcurrentWorkerEvent::Closed);
        }).map_err(|_| ConcurrencyError::WorkerClosed)?;
        Ok(Self { commands: command_tx, events: event_rx, thread: Some(thread) })
    }

    pub fn post_message(&self, message: StructuredCloneValue) -> Result<(), ConcurrencyError> {
        self.commands.try_send(WorkerCommand::Message(message)).map_err(|error| match error { TrySendError::Full(_) => ConcurrencyError::WorkerBackpressure, TrySendError::Disconnected(_) => ConcurrencyError::WorkerClosed })
    }

    pub fn try_event(&self) -> Option<ConcurrentWorkerEvent> {
        match self.events.try_recv() { Ok(event) => Some(event), Err(TryRecvError::Empty | TryRecvError::Disconnected) => None }
    }

    pub fn shutdown(&mut self) {
        let _ = self.commands.send(WorkerCommand::Shutdown);
        if let Some(thread) = self.thread.take() { let _ = thread.join(); }
    }
}

impl Drop for ConcurrentWorker { fn drop(&mut self) { self.shutdown(); } }
