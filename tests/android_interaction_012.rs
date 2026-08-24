use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use nexus_engine::{BrowserCore, BrowserCoreConfig, Viewport};

#[test]
fn zoom_reflows_and_keeps_zoom_in_snapshot() {
    let base = spawn_test_server();
    let mut browser = BrowserCore::new(BrowserCoreConfig {
        viewport: Viewport { width: 400.0, height: 800.0 },
        profile_dir: None,
        max_tabs: 4,
        restore_on_start: false,
    }).unwrap();
    browser.navigate_active(&base).unwrap();
    let before = browser.active_snapshot().unwrap();
    assert_eq!(before.zoom_factor, 1.0);

    let zoom = browser.set_active_zoom(2.0, 200.0, 300.0).unwrap();
    assert!((zoom - 2.0).abs() < 0.001);
    let after = browser.active_snapshot().unwrap();
    assert!((after.zoom_factor - 2.0).abs() < 0.001);
    assert!(after.max_scroll_y >= before.max_scroll_y);
}

#[test]
fn long_press_selection_returns_text_and_link_context() {
    let base = spawn_test_server();
    let mut browser = BrowserCore::new(BrowserCoreConfig {
        viewport: Viewport { width: 400.0, height: 800.0 },
        profile_dir: None,
        max_tabs: 4,
        restore_on_start: false,
    }).unwrap();
    browser.navigate_active(&base).unwrap();
    let page = browser.active_snapshot().unwrap();
    assert!(page.url.is_some());

    // The test link is the first visible block after body defaults. Scan a small
    // grid because this test is intentionally independent from internal layout IDs.
    let mut found = None;
    'outer: for y in (0..300).step_by(10) {
        for x in (0..300).step_by(10) {
            if let Some(info) = browser.select_active_at(x as f32, y as f32).unwrap() {
                if info.link_url.is_some() && info.text.contains("Nexus selection") {
                    found = Some(info);
                    break 'outer;
                }
            }
        }
    }
    let info = found.expect("link selection should be discoverable");
    assert_eq!(info.link_url.unwrap().path(), "/next");
    assert!(info.text.contains("Nexus selection"));

    browser.clear_active_selection().unwrap();
    assert!(browser.active_snapshot().unwrap().selected_text.is_none());
}

#[test]
fn physical_scroll_is_scaled_by_zoom() {
    let base = spawn_test_server();
    let mut browser = BrowserCore::new(BrowserCoreConfig {
        viewport: Viewport { width: 400.0, height: 300.0 },
        profile_dir: None,
        max_tabs: 4,
        restore_on_start: false,
    }).unwrap();
    browser.navigate_active(&base).unwrap();
    browser.set_active_zoom(2.0, 200.0, 150.0).unwrap();
    let before = browser.active_snapshot().unwrap().scroll_y;
    let after = browser.scroll_active_by_pixels(100.0).unwrap();
    assert!((after - before - 50.0).abs() < 0.5 || after == browser.active_snapshot().unwrap().max_scroll_y);
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
    format!("http://{address}/")
}

fn serve(stream: &mut TcpStream) {
    let mut buffer = [0_u8; 4096];
    let _ = stream.read(&mut buffer);
    let body = r#"<!doctype html>
<html><head><style>
body{margin:0} a{display:block;width:260px;height:70px;font-size:20px}
.spacer{height:1800px}
</style></head><body>
<a href="/next"><span>Nexus selection link</span></a>
<div class="spacer">Long scrolling page for zoom tests</div>
</body></html>"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(), body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}
