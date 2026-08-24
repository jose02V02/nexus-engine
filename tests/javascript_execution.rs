use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use nexus_engine::{BrowserSession, NexusEngine};

#[test]
fn inline_javascript_mutates_dom_then_reflows_layout() {
    let base = spawn_test_server();
    let engine = NexusEngine::builder()
        .viewport(390.0, 700.0)
        .build()
        .unwrap();

    let page = engine.load(&format!("{base}/inline")).unwrap();
    let target = page.dom.query_selector("#target").unwrap();

    assert_eq!(page.dom.title().as_deref(), Some("Nexus JS ready"));
    assert_eq!(page.dom.text_content(target), "after timer");
    assert_eq!(page.dom.attribute(target, "class"), Some("hot"));
    assert_eq!(page.dom.attribute(target, "data-ready"), Some("yes"));

    let target_box = page.layout.box_for(target).unwrap();
    assert!(target_box.width >= 230.0, "JS class mutation must affect CSS/layout");

    assert_eq!(page.javascript.scripts_found, 1);
    assert_eq!(page.javascript.scripts_executed, 1);
    assert_eq!(page.javascript.inline_scripts_executed, 1);
    assert!(page.javascript.dom_mutations >= 5);
    assert_eq!(page.javascript.timers_executed, 1);
    assert!(page
        .javascript
        .console
        .iter()
        .any(|entry| entry.message.contains("inline boot")));
}

#[test]
fn external_javascript_is_loaded_and_executed_in_page() {
    let base = spawn_test_server();
    let engine = NexusEngine::builder().viewport(390.0, 700.0).build().unwrap();
    let page = engine.load(&format!("{base}/external")).unwrap();

    let target = page.dom.query_selector("#external-target").unwrap();
    assert_eq!(page.dom.title().as_deref(), Some("External script ran"));
    assert_eq!(page.dom.text_content(target), "loaded ✓");
    assert_eq!(page.javascript.scripts_executed, 1);
    assert_eq!(page.javascript.external_scripts_loaded, 1);
    assert!(page.javascript.script_bytes_executed > 0);
}

#[test]
fn browser_session_follows_location_navigation_requested_by_script() {
    let base = spawn_test_server();
    let engine = NexusEngine::builder().viewport(390.0, 700.0).build().unwrap();
    let mut session = BrowserSession::new(engine);

    let nav = session.navigate(&format!("{base}/script-nav")).unwrap();
    assert!(nav.final_url.path().ends_with("/destination"));
    assert_eq!(nav.title, "Destination");
    assert_eq!(session.history().len(), 1);
    assert!(session
        .snapshot()
        .url
        .as_ref()
        .is_some_and(|url| url.path().ends_with("/destination")));
}

#[test]
fn javascript_can_be_disabled_by_builder() {
    let base = spawn_test_server();
    let engine = NexusEngine::builder()
        .javascript_enabled(false)
        .viewport(390.0, 700.0)
        .build()
        .unwrap();
    let page = engine.load(&format!("{base}/inline")).unwrap();

    assert!(!page.javascript.enabled);
    assert_eq!(page.dom.title().as_deref(), Some("Before"));
    let target = page.dom.query_selector("#target").unwrap();
    assert_eq!(page.dom.text_content(target), "before");
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
    let mut buffer = [0_u8; 8192];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    let (content_type, body) = match path {
        "/inline" => (
            "text/html; charset=utf-8",
            r#"<!doctype html>
<html>
<head>
  <title>Before</title>
  <style>
    #target { display:block; width:80px; height:40px; }
    .hot { width:240px; }
  </style>
</head>
<body>
  <div id="target">before</div>
  <script>
    const target = document.querySelector('#target');
    target.textContent = 'after';
    target.className = 'hot';
    target.setAttribute('style', 'height: 55px');
    document.title = 'Nexus JS ready';
    console.log('inline boot', target.textContent);
    document.addEventListener('DOMContentLoaded', () => target.setAttribute('data-ready', 'yes'));
    setTimeout(() => { target.textContent = target.textContent + ' timer'; }, 0);
  </script>
</body>
</html>"#,
        ),
        "/external" => (
            "text/html; charset=utf-8",
            r#"<!doctype html><html><head><title>External before</title></head><body>
            <p id="external-target">waiting</p><script src="/app.js"></script></body></html>"#,
        ),
        "/app.js" => (
            "text/javascript; charset=utf-8",
            "document.title = 'External script ran'; document.querySelector('#external-target').textContent = 'loaded ✓';",
        ),
        "/script-nav" => (
            "text/html; charset=utf-8",
            r#"<!doctype html><title>Leaving</title><script>location.href='/destination';</script>"#,
        ),
        "/destination" => (
            "text/html; charset=utf-8",
            r#"<!doctype html><title>Destination</title><h1>Arrived through JavaScript</h1>"#,
        ),
        _ => ("text/html; charset=utf-8", "<!doctype html><title>404</title>"),
    };

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}
