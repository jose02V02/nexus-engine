use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nexus_engine::{BrowserCore, BrowserCoreConfig, DownloadStatus, Viewport};

struct TestServer {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}

impl TestServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let join = thread::spawn(move || {
            while !worker_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => handle(stream),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        Self { addr, stop, join: Some(join) }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn handle(mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let mut buffer = [0_u8; 8192];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]);
    let path = request.split_whitespace().nth(1).unwrap_or("/");
    let cookie_seen = request.lines().any(|line| line.to_ascii_lowercase().starts_with("cookie:") && line.contains("nexus=1"));
    let (status, headers, body) = match path {
        "/cookie" => (
            "200 OK",
            "Set-Cookie: nexus=1; Path=/; Max-Age=3600\r\nContent-Type: text/html; charset=utf-8\r\n",
            "<html><head><title>cookie-set</title></head><body>set</body></html>".to_owned(),
        ),
        "/check" => (
            "200 OK",
            "Content-Type: text/html; charset=utf-8\r\n",
            format!("<html><head><title>{}</title></head><body>check</body></html>", if cookie_seen { "cookie-seen" } else { "cookie-missing" }),
        ),
        "/a" => ("200 OK", "Content-Type: text/html; charset=utf-8\r\n", "<html><head><title>A</title></head><body>A</body></html>".to_owned()),
        "/b" => ("200 OK", "Content-Type: text/html; charset=utf-8\r\n", "<html><head><title>B</title></head><body>B</body></html>".to_owned()),
        "/file.bin" => ("200 OK", "Content-Type: application/octet-stream\r\n", "nexus-download".to_owned()),
        _ => ("404 Not Found", "Content-Type: text/plain\r\n", "missing".to_owned()),
    };
    let response = format!(
        "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn temp_profile(label: &str) -> PathBuf {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    std::env::temp_dir().join(format!("nexus-010-{label}-{stamp}"))
}

fn config(profile: PathBuf) -> BrowserCoreConfig {
    BrowserCoreConfig {
        viewport: Viewport { width: 390.0, height: 700.0 },
        profile_dir: Some(profile),
        max_tabs: 8,
        restore_on_start: false,
    }
}

#[test]
fn tabs_share_cookie_state_but_keep_separate_sessions() {
    let server = TestServer::start();
    let profile = temp_profile("cookies");
    let mut browser = BrowserCore::new(config(profile.clone())).unwrap();
    browser.navigate_active(&server.url("/cookie")).unwrap();
    browser.new_tab(Some(&server.url("/check")), true).unwrap();
    assert_eq!(browser.active_snapshot().unwrap().title.as_deref(), Some("cookie-seen"));
    assert_eq!(browser.tab_count(), 2);
    drop(browser);
    let _ = std::fs::remove_dir_all(profile);
}

#[test]
fn restores_tab_urls_and_active_index_from_profile() {
    let server = TestServer::start();
    let profile = temp_profile("restore");
    {
        let mut browser = BrowserCore::new(config(profile.clone())).unwrap();
        browser.navigate_active(&server.url("/a")).unwrap();
        let second = browser.new_tab(Some(&server.url("/b")), true).unwrap();
        assert_eq!(browser.active_tab_id(), Some(second));
        browser.save_session().unwrap();
    }
    let restored = BrowserCore::new(BrowserCoreConfig {
        restore_on_start: true,
        ..config(profile.clone())
    }).unwrap();
    assert_eq!(restored.tab_count(), 2);
    assert_eq!(restored.active_snapshot().unwrap().title.as_deref(), Some("B"));
    drop(restored);
    let _ = std::fs::remove_dir_all(profile);
}

#[test]
fn download_manager_streams_into_profile_downloads() {
    let server = TestServer::start();
    let profile = temp_profile("download");
    let mut browser = BrowserCore::new(config(profile.clone())).unwrap();
    let url = url::Url::parse(&server.url("/file.bin")).unwrap();
    let item = browser.download_url(&url, Some("file.bin")).unwrap();
    assert_eq!(item.status, DownloadStatus::Completed);
    assert_eq!(std::fs::read(&item.path).unwrap(), b"nexus-download");
    drop(browser);
    let _ = std::fs::remove_dir_all(profile);
}
