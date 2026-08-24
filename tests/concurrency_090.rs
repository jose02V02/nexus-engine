use std::thread;
use std::time::{Duration, Instant};

use nexus_engine::{AtomicWaitResult, ConcurrentWorker, ConcurrentWorkerEvent, ConcurrencyError, SharedArrayBuffer, StructuredCloneValue};

fn next_event(worker: &ConcurrentWorker) -> ConcurrentWorkerEvent {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if let Some(event) = worker.try_event() { return event; }
        assert!(Instant::now() < deadline, "worker event timed out");
        thread::yield_now();
    }
}

#[test]
fn shared_memory_requires_cross_origin_isolation() {
    assert!(matches!(SharedArrayBuffer::new(16, false), Err(ConcurrencyError::CrossOriginIsolationRequired)));
    assert!(SharedArrayBuffer::new(16, true).is_ok());
}

#[test]
fn atomic_operations_return_previous_values() {
    let buffer = SharedArrayBuffer::new(16, true).unwrap();
    assert_eq!(buffer.store_i32(0, 10).unwrap(), 10);
    assert_eq!(buffer.add_i32(0, 5).unwrap(), 10);
    assert_eq!(buffer.sub_i32(0, 2).unwrap(), 15);
    assert_eq!(buffer.compare_exchange_i32(0, 13, 99).unwrap(), 13);
    assert_eq!(buffer.load_i32(0).unwrap(), 99);
}

#[test]
fn atomics_wait_reports_not_equal_and_timeout() {
    let buffer = SharedArrayBuffer::new(4, true).unwrap();
    assert_eq!(buffer.wait_i32(0, 1, Some(Duration::ZERO)).unwrap(), AtomicWaitResult::NotEqual);
    assert_eq!(buffer.wait_i32(0, 0, Some(Duration::ZERO)).unwrap(), AtomicWaitResult::TimedOut);
}

#[test]
fn atomics_notify_wakes_waiter_for_the_same_cell() {
    let buffer = SharedArrayBuffer::new(8, true).unwrap();
    let waiter_buffer = buffer.clone();
    let waiter = thread::spawn(move || waiter_buffer.wait_i32(1, 0, Some(Duration::from_secs(1))).unwrap());
    let deadline = Instant::now() + Duration::from_secs(1);
    while buffer.waiter_count(1).unwrap() == 0 { assert!(Instant::now() < deadline); thread::yield_now(); }
    assert_eq!(buffer.notify_i32(1, 1).unwrap(), 1);
    assert_eq!(waiter.join().unwrap(), AtomicWaitResult::Ok);
}

#[test]
fn atomics_indices_are_strictly_bounds_checked() {
    let buffer = SharedArrayBuffer::new(4, true).unwrap();
    assert_eq!(buffer.load_i32(1), Err(ConcurrencyError::IndexOutOfBounds));
    assert_eq!(buffer.notify_i32(1, 1), Err(ConcurrencyError::IndexOutOfBounds));
}

#[test]
fn worker_transports_structured_clone_messages() {
    let mut worker = ConcurrentWorker::spawn(4, |message| Some(message)).unwrap();
    assert!(matches!(next_event(&worker), ConcurrentWorkerEvent::Ready));
    worker.post_message(StructuredCloneValue::String("Nexus".into())).unwrap();
    match next_event(&worker) { ConcurrentWorkerEvent::Message(StructuredCloneValue::String(value)) => assert_eq!(value, "Nexus"), _ => panic!("unexpected worker event") }
    worker.shutdown();
    assert!(matches!(next_event(&worker), ConcurrentWorkerEvent::Closed));
}

#[test]
fn shared_buffer_identity_survives_worker_message_clone() {
    let buffer = SharedArrayBuffer::new(4, true).unwrap();
    let copy = buffer.clone();
    let mut worker = ConcurrentWorker::spawn(2, |message| {
        if let StructuredCloneValue::SharedBuffer(shared) = &message { shared.store_i32(0, 42).unwrap(); }
        Some(message)
    }).unwrap();
    let _ = next_event(&worker);
    worker.post_message(StructuredCloneValue::SharedBuffer(copy)).unwrap();
    let _ = next_event(&worker);
    assert_eq!(buffer.load_i32(0).unwrap(), 42);
}
