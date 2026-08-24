use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::thread;

use nexus_engine::network::NetworkClient;
use nexus_engine::{
    BrowserSession, CredentialsMode, FetchMode, NexusEngine, Origin, PageSecurityContext,
};
use url::Url;

#[test]
fn cookie_jar_survives_navigation() {
    let base = spawn_server(ServerMode::Cookies, Arc::new(AtomicUsize::new(0)));
    let engine = NexusEngine::builder().viewport(390.0, 500.0).build().unwrap();
    let mut session = BrowserSession::new(engine);
    session.navigate(&format!("{base}/set")).unwrap();
    assert!(session.snapshot().cookie_count >= 1);
    session.navigate(&format!("{base}/check")).unwrap();
    assert_eq!(session.snapshot().title.as_deref(), Some("Cookie OK"));
}

#[test]
fn local_and_session_storage_survive_same_session_navigation() {
    let base = spawn_server(ServerMode::Storage, Arc::new(AtomicUsize::new(0)));
    let engine = NexusEngine::builder().viewport(390.0, 500.0).build().unwrap();
    let mut session = BrowserSession::new(engine);
    session.navigate(&format!("{base}/storage-set")).unwrap();
    session.navigate(&format!("{base}/storage-get")).unwrap();
    let page = session.current_page().unwrap();
    let local = page.dom.query_selector("#local").unwrap();
    let session_node = page.dom.query_selector("#session").unwrap();
    assert_eq!(page.dom.text_content(local), "local-value");
    assert_eq!(page.dom.text_content(session_node), "session-value");
    assert_eq!(session.snapshot().local_storage_origins, 1);
}

#[test]
fn cors_blocks_cross_origin_without_allow_origin() {
    let base = spawn_server(ServerMode::CorsDenied, Arc::new(AtomicUsize::new(0)));
    let client = NetworkClient::new(1024 * 1024).unwrap();
    let source_url = Url::parse("http://source.example/").unwrap();
    let security = PageSecurityContext::permissive(source_url);
    let target = Url::parse(&format!("{base}/data")).unwrap();
    let error = client
        .web_request(
            &security,
            &target,
            "GET",
            "*/*",
            None,
            None,
            FetchMode::Cors,
            CredentialsMode::Omit,
        )
        .unwrap_err();
    assert!(error.to_string().contains("CORS"));
}

#[test]
fn cors_allows_explicit_origin() {
    let source_url = Url::parse("http://source.example/").unwrap();
    let source = Origin::from_url(&source_url);
    let security = PageSecurityContext::permissive(source_url);
    let base = spawn_server(ServerMode::CorsAllowed(source.serialize()), Arc::new(AtomicUsize::new(0)));
    let client = NetworkClient::new(1024 * 1024).unwrap();
    let target = Url::parse(&format!("{base}/data")).unwrap();
    let response = client
        .web_request(
            &security,
            &target,
            "GET",
            "*/*",
            None,
            None,
            FetchMode::Cors,
            CredentialsMode::Omit,
        )
        .unwrap();
    assert_eq!(response.status, 200);
}

#[test]
fn fresh_http_cache_avoids_second_network_request() {
    let counter = Arc::new(AtomicUsize::new(0));
    let base = spawn_server(ServerMode::FreshCache, Arc::clone(&counter));
    let client = NetworkClient::new(1024 * 1024).unwrap();
    let url = Url::parse(&format!("{base}/cache")).unwrap();
    let first = client.fetch(&url).unwrap();
    let second = client.fetch(&url).unwrap();
    assert!(!first.from_http_cache);
    assert!(second.from_http_cache);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn stale_etag_entry_revalidates_with_304() {
    let counter = Arc::new(AtomicUsize::new(0));
    let base = spawn_server(ServerMode::Revalidate, Arc::clone(&counter));
    let client = NetworkClient::new(1024 * 1024).unwrap();
    let url = Url::parse(&format!("{base}/etag")).unwrap();
    let first = client.fetch(&url).unwrap();
    let second = client.fetch(&url).unwrap();
    assert_eq!(first.body, b"cached-body");
    assert_eq!(second.body, b"cached-body");
    assert!(second.from_http_cache);
    assert!(second.revalidated);
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[test]
fn profile_local_storage_reopens_from_disk() {
    let dir = std::env::temp_dir().join(format!(
        "nexus-engine-0.9-storage-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let origin = Origin::from_url(&Url::parse("https://profile.example/").unwrap());
    {
        let state = nexus_engine::BrowserState::new(Some(dir.clone()));
        state.local_set(&origin, "persist", "yes").unwrap();
    }
    {
        let state = nexus_engine::BrowserState::new(Some(dir.clone()));
        assert_eq!(state.local_get(&origin, "persist").unwrap().as_deref(), Some("yes"));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn persistent_cookie_reopens_from_profile() {
    let base = spawn_server(ServerMode::PersistentCookies, Arc::new(AtomicUsize::new(0)));
    let dir = std::env::temp_dir().join(format!(
        "nexus-engine-0.9-cookies-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    {
        let engine = NexusEngine::builder()
            .viewport(390.0, 500.0)
            .profile_dir(dir.clone())
            .build()
            .unwrap();
        let mut session = BrowserSession::new(engine);
        session.navigate(&format!("{base}/set")).unwrap();
        assert!(session.snapshot().cookie_count >= 1);
    }
    {
        let engine = NexusEngine::builder()
            .viewport(390.0, 500.0)
            .profile_dir(dir.clone())
            .build()
            .unwrap();
        let mut session = BrowserSession::new(engine);
        session.navigate(&format!("{base}/check")).unwrap();
        assert_eq!(session.snapshot().title.as_deref(), Some("Persistent Cookie OK"));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[derive(Clone)]
enum ServerMode {
    Cookies,
    PersistentCookies,
    Storage,
    CorsDenied,
    CorsAllowed(String),
    FreshCache,
    Revalidate,
}

fn spawn_server(mode: ServerMode, counter: Arc<AtomicUsize>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            serve(&mut stream, &mode, &counter);
        }
    });
    format!("http://{address}")
}

fn serve(stream: &mut TcpStream, mode: &ServerMode, counter: &AtomicUsize) {
    let mut buffer = [0_u8; 16_384];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]);
    let first = request.lines().next().unwrap_or("GET / HTTP/1.1");
    let path = first.split_whitespace().nth(1).unwrap_or("/");
    counter.fetch_add(1, Ordering::SeqCst);

    match mode {
        ServerMode::Cookies => {
            if path == "/set" {
                respond(stream, 200, "OK", vec![
                    ("Content-Type", "text/html; charset=utf-8".to_owned()),
                    ("Set-Cookie", "nexus_session=ready; Path=/; HttpOnly; SameSite=Lax".to_owned()),
                ], "<!doctype html><title>Set Cookie</title>");
            } else {
                let has_cookie = request.lines().any(|line| {
                    line.to_ascii_lowercase().starts_with("cookie:") && line.contains("nexus_session=ready")
                });
                let title = if has_cookie { "Cookie OK" } else { "Cookie Missing" };
                respond(stream, 200, "OK", vec![("Content-Type", "text/html; charset=utf-8".to_owned())], &format!("<!doctype html><title>{title}</title>"));
            }
        }
        ServerMode::PersistentCookies => {
            if path == "/set" {
                respond(stream, 200, "OK", vec![
                    ("Content-Type", "text/html; charset=utf-8".to_owned()),
                    ("Set-Cookie", "nexus_persistent=ready; Max-Age=3600; Path=/; HttpOnly; SameSite=Lax".to_owned()),
                ], "<!doctype html><title>Persistent Cookie Set</title>");
            } else {
                let has_cookie = request.lines().any(|line| {
                    line.to_ascii_lowercase().starts_with("cookie:") && line.contains("nexus_persistent=ready")
                });
                let title = if has_cookie { "Persistent Cookie OK" } else { "Persistent Cookie Missing" };
                respond(stream, 200, "OK", vec![("Content-Type", "text/html; charset=utf-8".to_owned())], &format!("<!doctype html><title>{title}</title>"));
            }
        }
        ServerMode::Storage => {
            let body = if path == "/storage-set" {
                r#"<!doctype html><title>Storage Set</title><script>
                localStorage.setItem('local-key', 'local-value');
                sessionStorage.setItem('session-key', 'session-value');
                </script>"#
            } else {
                r#"<!doctype html><title>Storage Get</title>
                <div id="local"></div><div id="session"></div><script>
                document.querySelector('#local').textContent = localStorage.getItem('local-key') || 'missing';
                document.querySelector('#session').textContent = sessionStorage.getItem('session-key') || 'missing';
                </script>"#
            };
            respond(stream, 200, "OK", vec![("Content-Type", "text/html; charset=utf-8".to_owned())], body);
        }
        ServerMode::CorsDenied => {
            respond(stream, 200, "OK", vec![("Content-Type", "application/json".to_owned())], r#"{"ok":true}"#);
        }
        ServerMode::CorsAllowed(origin) => {
            respond(stream, 200, "OK", vec![
                ("Content-Type", "application/json".to_owned()),
                ("Access-Control-Allow-Origin", origin.clone()),
            ], r#"{"ok":true}"#);
        }
        ServerMode::FreshCache => {
            respond(stream, 200, "OK", vec![
                ("Content-Type", "text/plain".to_owned()),
                ("Cache-Control", "public, max-age=120".to_owned()),
            ], "fresh-body");
        }
        ServerMode::Revalidate => {
            let validated = request.lines().any(|line| {
                line.to_ascii_lowercase().starts_with("if-none-match:") && line.contains("nexus-etag")
            });
            if validated {
                respond(stream, 304, "Not Modified", vec![
                    ("ETag", "\"nexus-etag\"".to_owned()),
                    ("Cache-Control", "public, max-age=60".to_owned()),
                ], "");
            } else {
                respond(stream, 200, "OK", vec![
                    ("Content-Type", "text/plain".to_owned()),
                    ("ETag", "\"nexus-etag\"".to_owned()),
                    ("Cache-Control", "public, max-age=0".to_owned()),
                ], "cached-body");
            }
        }
    }
}

fn respond(stream: &mut TcpStream, status: u16, reason: &str, headers: Vec<(&str, String)>, body: &str) {
    let mut response = format!("HTTP/1.1 {status} {reason}\r\n");
    for (name, value) in headers {
        response.push_str(name);
        response.push_str(": ");
        response.push_str(&value);
        response.push_str("\r\n");
    }
    response.push_str(&format!("Content-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body));
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}
