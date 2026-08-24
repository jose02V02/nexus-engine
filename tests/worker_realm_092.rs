use std::thread;
use std::time::{Duration, Instant};

use nexus_engine::{JavaScriptWorker, WorkerRealmEvent};
use serde_json::json;

fn next_event(worker: &JavaScriptWorker) -> WorkerRealmEvent {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(event) = worker.try_event() { return event; }
        assert!(Instant::now() < deadline, "JavaScript Worker event timed out");
        thread::yield_now();
    }
}

#[test]
fn worker_runs_script_in_independent_quickjs_realm() {
    let worker = JavaScriptWorker::spawn("onmessage = event => postMessage({answer: event.data.value * 2});", 4).unwrap();
    assert_eq!(next_event(&worker), WorkerRealmEvent::Ready);
    worker.post_message(json!({"value": 21})).unwrap();
    assert_eq!(next_event(&worker), WorkerRealmEvent::Message(json!({"answer": 42})));
}

#[test]
fn separate_workers_do_not_share_javascript_globals() {
    let source = "let count = 0; onmessage = () => postMessage(++count);";
    let first = JavaScriptWorker::spawn(source, 2).unwrap();
    let second = JavaScriptWorker::spawn(source, 2).unwrap();
    let _ = next_event(&first); let _ = next_event(&second);
    first.post_message(json!(null)).unwrap(); second.post_message(json!(null)).unwrap();
    assert_eq!(next_event(&first), WorkerRealmEvent::Message(json!(1)));
    assert_eq!(next_event(&second), WorkerRealmEvent::Message(json!(1)));
}

#[test]
fn promise_jobs_run_at_message_microtask_checkpoint() {
    let worker = JavaScriptWorker::spawn("onmessage = event => Promise.resolve(event.data).then(value => postMessage(value + 1));", 2).unwrap();
    let _ = next_event(&worker);
    worker.post_message(json!(9)).unwrap();
    assert_eq!(next_event(&worker), WorkerRealmEvent::Message(json!(10)));
}

#[test]
fn timer_callbacks_run_only_when_worker_is_checkpointed() {
    let worker = JavaScriptWorker::spawn("setTimeout(() => postMessage('timer'), 0);", 2).unwrap();
    let _ = next_event(&worker);
    worker.checkpoint().unwrap();
    let first = next_event(&worker);
    let second = next_event(&worker);
    assert!(matches!((first, second), (WorkerRealmEvent::Checkpoint(_), WorkerRealmEvent::Message(value)) if value == json!("timer")));
}

#[test]
fn startup_exception_is_isolated_and_closes_only_worker() {
    let worker = JavaScriptWorker::spawn("throw new Error('boom');", 2).unwrap();
    assert!(matches!(next_event(&worker), WorkerRealmEvent::Error(message) if message.contains("boom")));
    assert_eq!(next_event(&worker), WorkerRealmEvent::Closed);
}

#[test]
fn explicit_termination_joins_realm_thread() {
    let mut worker = JavaScriptWorker::spawn("onmessage = () => {};", 2).unwrap();
    let _ = next_event(&worker);
    worker.terminate();
    assert_eq!(next_event(&worker), WorkerRealmEvent::Closed);
}
