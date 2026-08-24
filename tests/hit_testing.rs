use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use nexus_engine::{hit_test_page, NexusEngine};

#[test]
fn text_inside_anchor_resolves_to_anchor_url() {
    let base = spawn_test_server();
    let engine = NexusEngine::builder().viewport(400.0, 400.0).build().unwrap();
    let page = engine.load(&format!("{base}/")).unwrap();
    let anchor = page.dom.links().into_iter().next().unwrap();
    let anchor_box = page.layout.box_for(anchor.node_id).unwrap();

    let hit = hit_test_page(&page, anchor_box.x + 3.0, anchor_box.y + 3.0, 0.0).unwrap();
    assert!(hit.is_link());
    assert_eq!(hit.link_url.unwrap().path(), "/next");
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
    let mut buffer = [0_u8; 2048];
    let _ = stream.read(&mut buffer);
    let body = r#"<!doctype html><html><head><style>a{display:block;width:200px;height:60px}</style></head><body><a href="/next"><span>Nested link text</span></a></body></html>"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(), body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}
