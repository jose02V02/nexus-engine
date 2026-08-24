use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use nexus_engine::{BrowserSession, NexusEngine};

#[test]
fn click_handlers_and_globals_persist_across_interactions() {
    let base = spawn_test_server();
    let engine = NexusEngine::builder().viewport(390.0, 600.0).build().unwrap();
    let mut session = BrowserSession::new(engine);
    session.navigate(&format!("{base}/click")).unwrap();

    let (x, y) = center_of(&session, "#button");
    session.interact_at(x, y).unwrap();
    session.interact_at(x, y).unwrap();

    let page = session.current_page().unwrap();
    let output = page.dom.query_selector("#output").unwrap();
    assert_eq!(page.dom.text_content(output), "count=2");
    assert!(page.javascript.persistent_realm);
    assert!(page.javascript.events_dispatched >= 2);
    let created = page.dom.query_selector("#created").unwrap();
    assert_eq!(page.dom.text_content(created), "created dynamically");
}

#[test]
fn delayed_timer_mutates_dom_on_later_tick() {
    let base = spawn_test_server();
    let engine = NexusEngine::builder().viewport(390.0, 600.0).build().unwrap();
    let mut session = BrowserSession::new(engine);
    session.navigate(&format!("{base}/timer")).unwrap();

    let target = session.current_page().unwrap().dom.query_selector("#target").unwrap();
    assert_eq!(session.current_page().unwrap().dom.text_content(target), "waiting");
    thread::sleep(Duration::from_millis(90));
    let activity = session.tick().unwrap();
    assert!(activity.dirty);
    assert_eq!(session.current_page().unwrap().dom.text_content(target), "timer fired");
    assert!(session.snapshot().js_timers_executed >= 1);
}

#[test]
fn fetch_promise_updates_the_live_document() {
    let base = spawn_test_server();
    let engine = NexusEngine::builder().viewport(390.0, 600.0).build().unwrap();
    let mut session = BrowserSession::new(engine);
    session.navigate(&format!("{base}/fetch")).unwrap();

    let (x, y) = center_of(&session, "#load");
    session.interact_at(x, y).unwrap();

    let page = session.current_page().unwrap();
    let target = page.dom.query_selector("#result").unwrap();
    assert_eq!(page.dom.text_content(target), "hello from Nexus fetch");
    assert_eq!(page.javascript.fetch_requests, 1);
}


#[test]
fn prevent_default_blocks_link_navigation() {
    let base = spawn_test_server();
    let engine = NexusEngine::builder().viewport(390.0, 600.0).build().unwrap();
    let mut session = BrowserSession::new(engine);
    session.navigate(&format!("{base}/prevent")).unwrap();

    let (x, y) = center_of(&session, "#blocked");
    let interaction = session.interact_at(x, y).unwrap();
    assert!(interaction.default_prevented);
    assert!(interaction.navigation.is_none());
    assert!(session.snapshot().url.unwrap().path().ends_with("/prevent"));
    let page = session.current_page().unwrap();
    let status = page.dom.query_selector("#prevent-status").unwrap();
    assert_eq!(page.dom.text_content(status), "prevented");
}

#[test]
fn focused_input_dispatches_events_and_get_form_navigates() {
    let base = spawn_test_server();
    let engine = NexusEngine::builder().viewport(390.0, 600.0).build().unwrap();
    let mut session = BrowserSession::new(engine);
    session.navigate(&format!("{base}/form")).unwrap();

    let (x, y) = center_of(&session, "#q");
    let interaction = session.interact_at(x, y).unwrap();
    assert!(interaction.focused_node.is_some());

    session.set_focused_input_value("nexus browser").unwrap();
    let page = session.current_page().unwrap();
    let mirror = page.dom.query_selector("#mirror").unwrap();
    assert_eq!(page.dom.text_content(mirror), "nexus browser");

    let submitted = session.submit_focused_form().unwrap();
    assert!(submitted.navigation.is_some());
    let snapshot = session.snapshot();
    assert!(snapshot.url.unwrap().as_str().contains("q=nexus+browser"));
    assert_eq!(snapshot.title.as_deref(), Some("Form Result"));
}

fn center_of(session: &BrowserSession, selector: &str) -> (f32, f32) {
    let page = session.current_page().unwrap();
    let node = page.dom.query_selector(selector).unwrap();
    let box_ = page.layout.box_for(node).unwrap();
    (
        box_.x + box_.width.max(8.0) / 2.0,
        box_.y - session.scroll_y() + box_.height.max(8.0) / 2.0,
    )
}

fn spawn_test_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            serve(&mut stream);
        }
    });
    format!("http://{address}")
}

fn serve(stream: &mut TcpStream) {
    let mut buffer = [0_u8; 16_384];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]);
    let first = request.lines().next().unwrap_or("GET / HTTP/1.1");
    let path = first.split_whitespace().nth(1).unwrap_or("/");

    let (content_type, body) = if path == "/click" {
        (
            "text/html; charset=utf-8",
            r#"<!doctype html><title>Persistent Click</title><style>
            #button { display:block; width:180px; height:60px; }
            #output { display:block; width:220px; height:40px; }
            </style><button id="button">tap</button><div id="output">count=0</div>
            <script>
            let count = 0;
            const button = document.querySelector('#button');
            const output = document.querySelector('#output');
            const created = document.createElement('div');
            created.id = 'created';
            created.textContent = 'created dynamically';
            document.body.appendChild(created);
            button.addEventListener('click', () => { count++; output.textContent = 'count=' + count; });
            </script>"#,
        )
    } else if path == "/timer" {
        (
            "text/html; charset=utf-8",
            r#"<!doctype html><title>Timer</title><div id="target">waiting</div>
            <script>setTimeout(() => { document.querySelector('#target').textContent = 'timer fired'; }, 60);</script>"#,
        )
    } else if path == "/fetch" {
        (
            "text/html; charset=utf-8",
            r#"<!doctype html><title>Fetch</title><style>#load{display:block;width:160px;height:60px}</style>
            <button id="load">load</button><div id="result">empty</div>
            <script>
            document.querySelector('#load').addEventListener('click', () => {
              fetch('/data').then(r => r.json()).then(data => {
                document.querySelector('#result').textContent = data.message;
              });
            });
            </script>"#,
        )
    } else if path == "/data" {
        ("application/json; charset=utf-8", r#"{"message":"hello from Nexus fetch"}"#)
    } else if path == "/prevent" {
        (
            "text/html; charset=utf-8",
            r#"<!doctype html><title>Prevent</title><style>#blocked{display:block;width:180px;height:60px}</style>
            <a id="blocked" href="/destination">blocked link</a><div id="prevent-status">before</div>
            <script>document.querySelector('#blocked').addEventListener('click', event => {
              event.preventDefault(); document.querySelector('#prevent-status').textContent = 'prevented';
            });</script>"#,
        )
    } else if path == "/destination" {
        ("text/html; charset=utf-8", "<!doctype html><title>Destination</title>")
    } else if path == "/form" {
        (
            "text/html; charset=utf-8",
            r#"<!doctype html><title>Form</title><style>
            #q { display:block; width:220px; height:48px; }
            #send { display:block; width:140px; height:48px; }
            </style>
            <form id="form" action="/result" method="get">
              <input id="q" name="q" value="old">
              <button id="send" type="submit">Search</button>
            </form>
            <div id="mirror">old</div>
            <script>
            const q = document.querySelector('#q');
            q.addEventListener('input', () => document.querySelector('#mirror').textContent = q.value);
            </script>"#,
        )
    } else if path.starts_with("/result?") {
        ("text/html; charset=utf-8", "<!doctype html><title>Form Result</title><h1>submitted</h1>")
    } else {
        ("text/html; charset=utf-8", "<!doctype html><title>404</title>")
    };

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}
