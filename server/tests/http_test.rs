use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

fn start_test_server() -> (u16, zero_preview_server::http::HttpServerHandle) {
    let config = zero_preview_server::http::HttpConfig {
        host: "127.0.0.1".to_string(),
        port: 0,
        stop_exits_process: false,
        browser: std::sync::Arc::new(zero_preview_server::browser::DefaultBrowser),
    };
    let handle = zero_preview_server::http::start_server(config).expect("start server");
    (handle.port, handle)
}

fn http_get(port: u16, path: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).expect("connect");
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
        path,
        port
    );
    stream.write_all(request.as_bytes()).expect("write");
    stream.shutdown(std::net::Shutdown::Write).ok();

    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read");

    let status = response
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();

    (status, body)
}

#[test]
fn serves_html_with_injection_for_cjk_path() {
    let (port, server) = start_test_server();

    let base = std::env::temp_dir().join("zero-preview-test-cjk");
    let dir = base.join("測試 a#b");
    fs::create_dir_all(&dir).expect("create dir");
    let file = dir.join("100%.html");
    fs::write(&file, "<html><body>hi</body></html>").expect("write file");

    let path_str = file.to_string_lossy().replace('\\', "/");
    let encoded = zero_preview_server::paths::encode_path_for_url(&path_str);
    let (status, body) = http_get(port, &format!("/raw/{}", encoded));

    assert_eq!(status, 200);
    assert!(body.contains("hi"));
    assert!(body.contains("__ZED_LIVE_PREVIEW_INJECT__"));

    server.stop();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn sse_target_path_resolves_and_exists() {
    let base = std::env::temp_dir().join("zero-preview-test-sse-path");
    let dir = base.join("project");
    fs::create_dir_all(&dir).expect("create dir");
    let file = dir.join("index.html");
    fs::write(&file, "<html></html>").expect("write");

    let path_str = file.to_string_lossy().replace('\\', "/");
    let encoded = zero_preview_server::paths::encode_path_for_url(&path_str);
    let target = format!("/raw/{}", encoded);
    let raw = target.strip_prefix("/raw/").unwrap();
    let decoded = zero_preview_server::paths::raw_url_to_fs_path(raw, cfg!(windows));
    assert!(
        decoded.exists(),
        "decoded {:?} should exist (from target {})",
        decoded,
        target
    );

    let query = format!("target={}", url_encode(&target));
    let watch_dir =
        zero_preview_server::http::sse_watch_dir_from_query(&query, cfg!(windows));
    assert!(
        watch_dir.is_some(),
        "watch dir should resolve for query {}",
        query
    );

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn sse_emits_reload_on_html_change() {
    let (port, server) = start_test_server();

    let base = std::env::temp_dir().join("zero-preview-test-sse-reload");
    let dir = base.join("project");
    fs::create_dir_all(&dir).expect("create dir");
    let file = dir.join("index.html");
    fs::write(&file, "<html><body>v1</body></html>").expect("write");

    let path_str = file.to_string_lossy().replace('\\', "/");
    let encoded = zero_preview_server::paths::encode_path_for_url(&path_str);
    let target = format!("/raw/{}", encoded);
    let sse_path = format!("/__sse?target={}", url_encode(&target));

    let sse_port = port;
    let received = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let received_clone = received.clone();
    let connected = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let connected_clone = connected.clone();

    let reader = thread::spawn(move || {
        let mut stream =
            TcpStream::connect(format!("127.0.0.1:{}", sse_port)).expect("sse connect");
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: keep-alive\r\n\r\n",
            sse_path,
            sse_port
        );
        stream.write_all(request.as_bytes()).expect("sse write");
        let mut buf = [0u8; 4096];
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if let Ok(n) = stream.read(&mut buf) {
                if n > 0 {
                    let chunk = String::from_utf8_lossy(&buf[..n]);
                    received_clone.lock().unwrap().push_str(&chunk);
                    if chunk.contains(": connected") {
                        connected_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    if received_clone.lock().unwrap().contains("data: reload") {
                        return;
                    }
                }
            }
            thread::sleep(Duration::from_millis(50));
        }
    });

    let wait_deadline = std::time::Instant::now() + Duration::from_secs(2);
    while !connected.load(std::sync::atomic::Ordering::SeqCst)
        && std::time::Instant::now() < wait_deadline
    {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(connected.load(std::sync::atomic::Ordering::SeqCst), "SSE did not connect");

    let query = format!("target={}", url_encode(&target));
    let watch_dir =
        zero_preview_server::http::sse_watch_dir_from_query(&query, cfg!(windows)).unwrap();
    assert!(
        server.watch_registry.client_count(&watch_dir) > 0,
        "expected SSE client registered for {:?}",
        watch_dir
    );

    thread::sleep(Duration::from_millis(150));
    fs::write(&file, "<html><body>v2</body></html>").expect("rewrite");

    reader.join().expect("reader join");
    assert!(
        received.lock().unwrap().contains("data: reload"),
        "expected reload event, got: {}",
        received.lock().unwrap()
    );
    // SSE must stream: a Content-Length header would make browsers close the
    // EventSource after the first bytes and drop later events.
    assert!(
        !received.lock().unwrap().contains("Content-Length"),
        "SSE response must not carry Content-Length, got: {}",
        received.lock().unwrap()
    );

    server.stop();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn sse_emits_css_event_on_stylesheet_change() {
    let (port, server) = start_test_server();

    let base = std::env::temp_dir().join("zero-preview-test-sse-css");
    let dir = base.join("project");
    fs::create_dir_all(&dir).expect("create dir");
    let css = dir.join("style.css");
    fs::write(&css, "body { color: red; }").expect("write css");

    let path_str = css.to_string_lossy().replace('\\', "/");
    let encoded = zero_preview_server::paths::encode_path_for_url(&path_str);
    let target = format!("/raw/{}", encoded);
    let sse_path = format!("/__sse?target={}", url_encode(&target));

    let sse_port = port;
    let received = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let received_clone = received.clone();
    let connected = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let connected_clone = connected.clone();

    let reader = thread::spawn(move || {
        let mut stream =
            TcpStream::connect(format!("127.0.0.1:{}", sse_port)).expect("sse connect");
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: keep-alive\r\n\r\n",
            sse_path,
            sse_port
        );
        stream.write_all(request.as_bytes()).expect("sse write");
        let mut buf = [0u8; 4096];
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if let Ok(n) = stream.read(&mut buf) {
                if n > 0 {
                    let chunk = String::from_utf8_lossy(&buf[..n]);
                    received_clone.lock().unwrap().push_str(&chunk);
                    if chunk.contains(": connected") {
                        connected_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    if received_clone.lock().unwrap().contains("event: css") {
                        return;
                    }
                }
            }
            thread::sleep(Duration::from_millis(50));
        }
    });

    let wait_deadline = std::time::Instant::now() + Duration::from_secs(2);
    while !connected.load(std::sync::atomic::Ordering::SeqCst)
        && std::time::Instant::now() < wait_deadline
    {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(connected.load(std::sync::atomic::Ordering::SeqCst), "SSE did not connect");

    thread::sleep(Duration::from_millis(150));
    fs::write(&css, "body { color: blue; }").expect("rewrite css");

    reader.join().expect("reader join");
    let data = received.lock().unwrap().clone();
    assert!(data.contains("event: css"), "expected css event, got: {}", data);
    assert!(data.contains("style.css"), "expected filename in event, got: {}", data);

    server.stop();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn open_endpoint_invokes_browser_with_raw_url() {
    let mock = zero_preview_server::browser::MockBrowser::new();
    let browser: std::sync::Arc<zero_preview_server::browser::MockBrowser> =
        std::sync::Arc::new(mock);

    let config = zero_preview_server::http::HttpConfig {
        host: "127.0.0.1".to_string(),
        port: 0,
        stop_exits_process: false,
        browser: browser.clone(),
    };
    let server = zero_preview_server::http::start_server(config).expect("start");
    let port = server.port;

    let base = std::env::temp_dir().join("zero-preview-test-open");
    fs::create_dir_all(&base).expect("create dir");
    let file = base.join("page.html");
    fs::write(&file, "<html></html>").expect("write");

    let path_str = file.to_string_lossy().replace('\\', "/");
    let encoded_file = zero_preview_server::paths::encode_path_for_url(&path_str);
    let (status, body) = http_get(port, &format!("/__open?file={}", url_encode(&path_str)));

    assert_eq!(status, 200);
    assert_eq!(body.trim(), "opened");

    let urls = browser.urls.lock().unwrap().clone();
    assert_eq!(urls.len(), 1);
    assert!(urls[0].contains(&format!("/raw/{}", encoded_file)));

    server.stop();
    let _ = fs::remove_dir_all(&base);
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'/' {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}
