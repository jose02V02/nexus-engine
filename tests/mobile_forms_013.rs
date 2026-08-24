use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread;

use nexus_engine::{BrowserSession, NexusEngine};

#[test]
fn select_control_exposes_options_and_submits_selected_value() {
    let base = spawn_server();
    let engine = NexusEngine::builder().viewport(390.0, 700.0).build().unwrap();
    let mut session = BrowserSession::new(engine);
    session.navigate(&format!("{base}/controls")).unwrap();

    let (x, y) = center_of(&session, "#color");
    session.interact_at(x, y).unwrap();
    let info = session.focused_control().expect("select descriptor");
    assert_eq!(info.tag, "select");
    assert_eq!(info.options.len(), 3);
    assert_eq!(info.options[0].value, "red");

    session.set_focused_select_indices(&[2]).unwrap();
    let selected = session.focused_control().unwrap();
    assert_eq!(selected.value, "blue");
    let submitted = session.submit_focused_form().unwrap();
    assert!(submitted.navigation.is_some());
    assert!(session.snapshot().url.unwrap().as_str().contains("color=blue"));
}

#[test]
fn checkbox_default_action_toggles_and_is_serialized() {
    let base = spawn_server();
    let engine = NexusEngine::builder().viewport(390.0, 700.0).build().unwrap();
    let mut session = BrowserSession::new(engine);
    session.navigate(&format!("{base}/checkbox")).unwrap();

    let (x, y) = center_of(&session, "#agree");
    session.interact_at(x, y).unwrap();
    let info = session.focused_control().unwrap();
    assert!(info.checked);

    let submitted = session.submit_focused_form().unwrap();
    assert!(submitted.navigation.is_some());
    assert!(session.snapshot().url.unwrap().as_str().contains("agree=yes"));
}

#[test]
fn required_email_validation_blocks_invalid_submission() {
    let base = spawn_server();
    let engine = NexusEngine::builder().viewport(390.0, 700.0).build().unwrap();
    let mut session = BrowserSession::new(engine);
    session.navigate(&format!("{base}/email")).unwrap();

    let (x, y) = center_of(&session, "#mail");
    session.interact_at(x, y).unwrap();
    session.set_focused_input_value("not-an-email").unwrap();
    assert!(session.submit_focused_form().is_err());

    session.set_focused_input_value("nexus@example.com").unwrap();
    assert!(session.submit_focused_form().unwrap().navigation.is_some());
}

#[test]
fn file_input_posts_real_multipart_bytes() {
    let base = spawn_server();
    let engine = NexusEngine::builder().viewport(390.0, 700.0).build().unwrap();
    let mut session = BrowserSession::new(engine);
    session.navigate(&format!("{base}/upload")).unwrap();

    let (x, y) = center_of(&session, "#attachment");
    session.interact_at(x, y).unwrap();
    let path = temp_upload_file();
    session
        .add_focused_file(path.clone(), "note.txt".to_owned(), "text/plain".to_owned(), false)
        .unwrap();
    let info = session.focused_control().unwrap();
    assert!(info.value.contains("note.txt"));

    let submitted = session.submit_focused_form().unwrap();
    assert!(submitted.navigation.is_some());
    assert_eq!(session.snapshot().title.as_deref(), Some("Upload OK"));
    let _ = std::fs::remove_file(path);
}

fn temp_upload_file() -> PathBuf {
    let path = std::env::temp_dir().join(format!("nexus-upload-{}-note.txt", std::process::id()));
    std::fs::write(&path, b"hello-upload").unwrap();
    path
}

fn center_of(session: &BrowserSession, selector: &str) -> (f32, f32) {
    let page = session.current_page().unwrap();
    let node = page.dom.query_selector(selector).unwrap();
    let box_ = page.layout.box_for(node).unwrap();
    (box_.x + box_.width.max(10.0) / 2.0, box_.y - session.scroll_y() + box_.height.max(10.0) / 2.0)
}

fn spawn_server() -> String {
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
    let request = read_request(stream);
    let first = request.lines().next().unwrap_or("GET / HTTP/1.1");
    let path = first.split_whitespace().nth(1).unwrap_or("/");
    let body = if path == "/controls" {
        r#"<!doctype html><title>Controls</title><style>#color{display:block;width:220px;height:52px}</style>
        <form action='/result' method='get'><select id='color' name='color'><option value='red'>Red</option><option value='green'>Green</option><option value='blue'>Blue</option></select></form>"#.to_owned()
    } else if path == "/checkbox" {
        r#"<!doctype html><title>Checkbox</title><style>#agree{display:block;width:80px;height:52px}</style>
        <form action='/result' method='get'><input id='agree' type='checkbox' name='agree' value='yes'></form>"#.to_owned()
    } else if path == "/email" {
        r#"<!doctype html><title>Email</title><style>#mail{display:block;width:220px;height:52px}</style>
        <form action='/result' method='get'><input id='mail' type='email' name='mail' required></form>"#.to_owned()
    } else if path == "/upload" {
        r#"<!doctype html><title>Upload</title><style>#attachment{display:block;width:220px;height:52px}</style>
        <form action='/upload-target' method='post' enctype='multipart/form-data'><input id='attachment' type='file' name='attachment' required accept='text/plain'></form>"#.to_owned()
    } else if path == "/upload-target" {
        let good = request.to_ascii_lowercase().contains("content-type: multipart/form-data; boundary=")
            && request.contains("filename=\"note.txt\"")
            && request.contains("hello-upload");
        if good { "<!doctype html><title>Upload OK</title>".to_owned() }
        else { "<!doctype html><title>Upload Bad</title>".to_owned() }
    } else if path.starts_with("/result?") {
        "<!doctype html><title>Result</title>".to_owned()
    } else {
        "<!doctype html><title>Not Found</title>".to_owned()
    };
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(), body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn read_request(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut expected = None;
    loop {
        let read = stream.read(&mut buffer).unwrap_or(0);
        if read == 0 { break; }
        bytes.extend_from_slice(&buffer[..read]);
        if expected.is_none() {
            if let Some(index) = find_header_end(&bytes) {
                let headers = String::from_utf8_lossy(&bytes[..index]);
                let content_length = headers.lines().find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse::<usize>().ok()).flatten()
                }).unwrap_or(0);
                expected = Some(index + 4 + content_length);
            }
        }
        if expected.is_some_and(|length| bytes.len() >= length) { break; }
        if bytes.len() > 40 * 1024 * 1024 { break; }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}
