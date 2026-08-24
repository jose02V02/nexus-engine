use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use nexus_engine::{BrowserSession, NexusEngine};

#[test]
fn session_follows_links_and_restores_history_scroll() {
    let base = spawn_test_server();
    let engine = NexusEngine::builder().viewport(390.0, 500.0).build().unwrap();
    let mut session = BrowserSession::new(engine);

    session.navigate(&format!("{base}/one")).unwrap();
    assert_eq!(session.history().len(), 1);

    let saved_scroll = session.scroll_by(350.0);
    assert!(saved_scroll > 0.0);

    let (x, y) = {
        let page = session.current_page().unwrap();
        let link = page.dom.links().into_iter().next().unwrap();
        let box_ = page.layout.box_for(link.node_id).unwrap();
        (box_.x + 4.0, box_.y - session.scroll_y() + 4.0)
    };

    let nav = session.activate_at(x, y).unwrap().unwrap();
    assert!(nav.final_url.path().ends_with("/two"));
    assert_eq!(session.history().len(), 2);
    assert!(session.snapshot().can_go_back);

    session.go_back().unwrap().unwrap();
    assert!(session.snapshot().url.unwrap().path().ends_with("/one"));
    assert!((session.scroll_y() - saved_scroll).abs() < 0.5);
    assert!(session.snapshot().can_go_forward);

    session.go_forward().unwrap().unwrap();
    assert!(session.snapshot().url.unwrap().path().ends_with("/two"));
}

#[test]
fn new_navigation_discards_forward_history() {
    let base = spawn_test_server();
    let engine = NexusEngine::builder().viewport(390.0, 500.0).build().unwrap();
    let mut session = BrowserSession::new(engine);

    session.navigate(&format!("{base}/one")).unwrap();
    session.navigate(&format!("{base}/two")).unwrap();
    session.go_back().unwrap().unwrap();
    assert!(session.snapshot().can_go_forward);

    session.navigate(&format!("{base}/three")).unwrap();
    assert_eq!(session.history().len(), 2);
    assert!(!session.snapshot().can_go_forward);
    assert!(session.snapshot().url.unwrap().path().ends_with("/three"));
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
    let mut buffer = [0_u8; 4096];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    let body = match path {
        "/one" => r#"<!doctype html><html><head><title>One</title><style>
            body { margin: 0px; }
            a { display: block; width: 220px; height: 70px; background-color: #eeeeee; }
            .spacer { display: block; height: 1800px; }
        </style></head><body><a href="/two"><span>Go to page two</span></a><div class="spacer"></div></body></html>"#,
        "/two" => r#"<!doctype html><html><head><title>Two</title></head><body><h1>Second page</h1><a href="/one">Back by link</a></body></html>"#,
        "/three" => r#"<!doctype html><html><head><title>Three</title></head><body><h1>Third page</h1></body></html>"#,
        _ => "<!doctype html><title>404</title>",
    };

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}
