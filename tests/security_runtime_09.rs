use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use nexus_engine::network::NetworkClient;
use nexus_engine::{
    BrowserEventLoop, BrowserState, CredentialsMode, CspPolicy, FetchMode, PageSecurityContext,
    ReferrerPolicy, WebSocketCommand, WebSocketEvent,
};
use url::Url;

#[test]
fn csp_blocks_external_connect_and_inline_script_without_unsafe_inline() {
    let page = Url::parse("https://app.example/").unwrap();
    let csp = CspPolicy::parse(Some("default-src 'self'; connect-src https://api.example; script-src 'self'"));
    assert!(!csp.allows_inline_script());
    assert!(csp.allows_connect_url(&page, &Url::parse("https://api.example/data").unwrap()));
    assert!(!csp.allows_connect_url(&page, &Url::parse("https://evil.example/data").unwrap()));
}

#[test]
fn referrer_policy_strips_path_cross_origin() {
    let source = Url::parse("https://app.example/account/private?q=1#token").unwrap();
    let target = Url::parse("https://api.example/data").unwrap();
    assert_eq!(
        ReferrerPolicy::StrictOriginWhenCrossOrigin.referer(&source, &target).as_deref(),
        Some("https://app.example/")
    );
}

#[test]
fn hsts_is_persisted_in_profile() {
    let dir = std::env::temp_dir().join(format!("nexus-09-hsts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    {
        let state = BrowserState::new(Some(dir.clone()));
        state.observe_hsts(&Url::parse("https://example.com/").unwrap(), "max-age=3600; includeSubDomains");
        assert!(state.hsts_upgrade(&Url::parse("http://www.example.com/a").unwrap()).is_some());
    }
    {
        let state = BrowserState::new(Some(dir.clone()));
        let upgraded = state.hsts_upgrade(&Url::parse("http://example.com/a").unwrap()).unwrap();
        assert_eq!(upgraded.scheme(), "https");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cors_json_post_performs_options_preflight() {
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let base = spawn_preflight_server(Arc::clone(&seen));
    let source_url = Url::parse("http://source.example/").unwrap();
    let security = PageSecurityContext::permissive(source_url);
    let client = NetworkClient::new(1024 * 1024).unwrap();
    let target = Url::parse(&format!("{base}/api")).unwrap();

    let response = client
        .web_request(
            &security,
            &target,
            "POST",
            "application/json",
            Some(br#"{"hello":"nexus"}"#),
            Some("application/json"),
            FetchMode::Cors,
            CredentialsMode::Omit,
        )
        .unwrap();
    assert_eq!(response.status, 200);
    let methods = seen.lock().unwrap().clone();
    assert_eq!(methods, vec!["OPTIONS", "POST"]);
}

fn spawn_preflight_server(seen: Arc<Mutex<Vec<String>>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    // Test source origin is deterministic; duplicate it here because the server
    // thread cannot read another thread's thread-local value.
    let allow_origin = "http://source.example".to_owned();
    thread::spawn(move || {
        for stream in listener.incoming().take(2) {
            let Ok(mut stream) = stream else { break };
            let request = read_request(&mut stream);
            let method = request.split_whitespace().next().unwrap_or("GET").to_owned();
            seen.lock().unwrap().push(method.clone());
            if method == "OPTIONS" {
                respond(&mut stream, 204, vec![
                    ("Access-Control-Allow-Origin", allow_origin.clone()),
                    ("Access-Control-Allow-Methods", "POST".to_owned()),
                    ("Access-Control-Allow-Headers", "content-type".to_owned()),
                ], "");
            } else {
                respond(&mut stream, 200, vec![
                    ("Access-Control-Allow-Origin", allow_origin.clone()),
                    ("Content-Type", "application/json".to_owned()),
                ], r#"{"ok":true}"#);
            }
        }
    });
    format!("http://{addr}")
}

fn read_request(stream: &mut TcpStream) -> String {
    let mut buffer = [0_u8; 16384];
    let size = stream.read(&mut buffer).unwrap_or(0);
    String::from_utf8_lossy(&buffer[..size]).into_owned()
}

fn respond(stream: &mut TcpStream, status: u16, headers: Vec<(&str, String)>, body: &str) {
    let reason = if status == 204 { "No Content" } else { "OK" };
    let mut response = format!("HTTP/1.1 {status} {reason}\r\n");
    for (name, value) in headers {
        response.push_str(&format!("{name}: {value}\r\n"));
    }
    response.push_str(&format!("Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()));
    stream.write_all(response.as_bytes()).unwrap();
}

#[test]
fn live_websocket_event_loop_echoes_text() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            if let Some(Ok(message)) = ws.next().await {
                ws.send(message).await.unwrap();
            }
        });
    });

    let mut event_loop = BrowserEventLoop::new().unwrap();
    event_loop
        .submit_websocket(WebSocketCommand::Open {
            id: 1,
            url: Url::parse(&format!("ws://{addr}/echo")).unwrap(),
            origin: "http://example.com".to_owned(),
            protocols: vec![],
        })
        .unwrap();

    wait_for_event(&mut event_loop, |event| matches!(event, WebSocketEvent::Open { id: 1, .. }));
    event_loop
        .submit_websocket(WebSocketCommand::SendText { id: 1, text: "nexus".to_owned() })
        .unwrap();
    let echoed = wait_for_event(&mut event_loop, |event| {
        matches!(event, WebSocketEvent::Text { id: 1, text } if text == "nexus")
    });
    assert!(matches!(echoed, WebSocketEvent::Text { .. }));
}

fn wait_for_event(
    event_loop: &mut BrowserEventLoop,
    predicate: impl Fn(&WebSocketEvent) -> bool,
) -> WebSocketEvent {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        for event in event_loop.drain_websocket_events(32) {
            if predicate(&event) {
                return event;
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("timed out waiting for Nexus WebSocket event")
}

#[test]
fn browser_session_websocket_event_updates_dom() {
    let ws_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    ws_listener.set_nonblocking(true).unwrap();
    let ws_addr = ws_listener.local_addr().unwrap();
    thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(ws_listener).unwrap();
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            if let Some(Ok(message)) = ws.next().await {
                ws.send(message).await.unwrap();
            }
        });
    });

    let http_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let http_addr = http_listener.local_addr().unwrap();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = http_listener.accept() {
            let _ = read_request(&mut stream);
            let body = format!(
                r#"<!doctype html><title>WS Session</title>
                <div id="status">waiting</div>
                <script>
                const socket = new WebSocket('ws://{ws_addr}/echo');
                socket.addEventListener('open', () => socket.send('nexus-live'));
                socket.addEventListener('message', event => {{
                    document.querySelector('#status').textContent = event.data;
                    socket.close();
                }});
                </script>"#
            );
            respond(
                &mut stream,
                200,
                vec![("Content-Type", "text/html; charset=utf-8".to_owned())],
                &body,
            );
        }
    });

    let engine = nexus_engine::NexusEngine::builder()
        .viewport(390.0, 500.0)
        .build()
        .unwrap();
    let mut session = nexus_engine::BrowserSession::new(engine);
    session.navigate(&format!("http://{http_addr}/")).unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut observed = false;
    while Instant::now() < deadline {
        session.tick().unwrap();
        if let Some(page) = session.current_page() {
            if let Some(node) = page.dom.query_selector("#status") {
                if page.dom.text_content(node) == "nexus-live" {
                    observed = true;
                    break;
                }
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(observed, "WebSocket message did not reach the live Nexus DOM");
}
