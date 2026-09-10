use crate::browser::BrowserOpener;
use crate::inject::INJECT_SCRIPT;
use crate::paths;
use crate::watch::WatchRegistry;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 52331;

fn mime_type(ext: &str) -> &'static str {
    match ext {
        ".html" | ".htm" => "text/html; charset=utf-8",
        ".css" => "text/css; charset=utf-8",
        ".js" | ".mjs" | ".cjs" => "application/javascript; charset=utf-8",
        ".json" => "application/json; charset=utf-8",
        ".png" => "image/png",
        ".jpg" | ".jpeg" => "image/jpeg",
        ".gif" => "image/gif",
        ".svg" => "image/svg+xml",
        ".webp" => "image/webp",
        ".ico" => "image/x-icon",
        ".woff" => "font/woff",
        ".woff2" => "font/woff2",
        ".ttf" => "font/ttf",
        ".otf" => "font/otf",
        ".mp4" => "video/mp4",
        ".webm" => "video/webm",
        ".mp3" => "audio/mpeg",
        ".wav" => "audio/wav",
        ".pdf" => "application/pdf",
        ".txt" => "text/plain; charset=utf-8",
        ".xml" => "application/xml; charset=utf-8",
        _ => "application/octet-stream",
    }
}

pub struct HttpConfig {
    pub host: String,
    pub port: u16,
    pub stop_exits_process: bool,
    pub browser: Arc<dyn BrowserOpener>,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
            stop_exits_process: true,
            browser: Arc::new(crate::browser::DefaultBrowser),
        }
    }
}

pub struct HttpServerHandle {
    pub port: u16,
    stop_flag: Arc<AtomicBool>,
    join_handle: Mutex<Option<thread::JoinHandle<()>>>,
    pub watch_registry: WatchRegistry,
}

impl HttpServerHandle {
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.join_handle.lock().unwrap().take() {
            let _ = handle.join();
        }
    }
}

pub fn start_server(config: HttpConfig) -> std::io::Result<HttpServerHandle> {
    let addr = format!("{}:{}", config.host, config.port);
    let listener = TcpListener::bind(&addr)?;
    let port = listener.local_addr()?.port();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_thread = stop_flag.clone();
    let watch_registry = WatchRegistry::new();
    let registry_for_thread = watch_registry.clone();
    let stop_exits = config.stop_exits_process;
    let browser = config.browser;

    let join_handle = thread::spawn(move || {
        listener
            .set_nonblocking(true)
            .expect("set_nonblocking should succeed");

        while !stop_flag_thread.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let registry = registry_for_thread.clone();
                    let browser = browser.clone();
                    thread::spawn(move || {
                        handle_connection(stream, &registry, browser, stop_exits);
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    Ok(HttpServerHandle {
        port,
        stop_flag,
        join_handle: Mutex::new(Some(join_handle)),
        watch_registry,
    })
}

pub fn try_bind(port: u16) -> std::io::Result<TcpListener> {
    TcpListener::bind(format!("{}:{}", DEFAULT_HOST, port))
}

pub fn ping_server(port: u16) -> bool {
    http_get(DEFAULT_HOST, port, "/__ping")
        .map(|(status, body)| status == 200 && body.trim() == "pong")
        .unwrap_or(false)
}

pub fn http_get(host: &str, port: u16, path: &str) -> std::io::Result<(u16, String)> {
    let mut stream = TcpStream::connect(format!("{}:{}", host, port))?;
    let request = format!("GET {} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\n\r\n", path, host, port);
    stream.write_all(request.as_bytes())?;
    stream.shutdown(std::net::Shutdown::Write)?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;

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

    Ok((status, body))
}

struct HttpRequest {
    path: String,
    query: String,
    headers: Vec<(String, String)>,
}

fn parse_request(raw: &str) -> Option<HttpRequest> {
    let lines: Vec<&str> = raw.split("\r\n").collect();
    if lines.is_empty() {
        return None;
    }

    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    if parts.len() < 2 || parts[0] != "GET" {
        return None;
    }

    let (path, query) = match parts[1].split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (parts[1].to_string(), String::new()),
    };

    let headers = lines
        .iter()
        .skip(1)
        .filter_map(|line| {
            line.split_once(':').map(|(k, v)| {
                (k.trim().to_ascii_lowercase(), v.trim().to_string())
            })
        })
        .collect();

    Some(HttpRequest {
        path,
        query,
        headers,
    })
}

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    status_text: &str,
    headers: &[(&str, &str)],
    body: &[u8],
    keep_alive: bool,
) -> std::io::Result<()> {
    let mut response = format!("HTTP/1.1 {} {}\r\n", status, status_text);
    if !keep_alive {
        response.push_str("Connection: close\r\n");
    }
    for (k, v) in headers {
        response.push_str(&format!("{}: {}\r\n", k, v));
    }
    response.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
    stream.write_all(response.as_bytes())?;
    if !body.is_empty() {
        stream.write_all(body)?;
    }
    stream.flush()?;
    Ok(())
}

fn write_response_text(
    stream: &mut TcpStream,
    status: u16,
    status_text: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> std::io::Result<()> {
    write_response(stream, status, status_text, headers, body.as_bytes(), false)
}

fn handle_connection(
    mut stream: TcpStream,
    watch_registry: &WatchRegistry,
    browser: Arc<dyn BrowserOpener>,
    stop_exits_process: bool,
) {
    let mut buf = [0u8; 8192];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let raw = String::from_utf8_lossy(&buf[..n]);
    let req = match parse_request(&raw) {
        Some(r) => r,
        None => {
            write_response_text(&mut stream, 405, "Method Not Allowed", &[], "Method Not Allowed")
                .ok();
            return;
        }
    };

    let is_windows = cfg!(windows);

    if req.path == "/__ping" {
        write_response_text(
            &mut stream,
            200,
            "OK",
            &[("Content-Type", "text/plain")],
            "pong",
        )
        .ok();
        return;
    }

    if req.path == "/__stop" {
        write_response_text(
            &mut stream,
            200,
            "OK",
            &[("Content-Type", "text/plain")],
            "stopping",
        )
        .ok();
        if stop_exits_process {
            thread::spawn(|| {
                thread::sleep(Duration::from_millis(150));
                std::process::exit(0);
            });
        }
        return;
    }

    if req.path == "/__sse" {
        handle_sse(stream, &req.query, watch_registry);
        return;
    }

    if req.path == "/__open" {
        handle_open(&mut stream, &req.query, &browser);
        return;
    }

    if req.path.starts_with("/raw/") {
        let raw_part = req.path.strip_prefix("/raw/").unwrap_or("");
        serve_raw_path(&mut stream, raw_part, &req.path, &req.query, is_windows);
        return;
    }

    if try_referer_fallback(&mut stream, &req, is_windows) {
        return;
    }

    write_response_text(
        &mut stream,
        200,
        "OK",
        &[("Content-Type", "text/html; charset=utf-8")],
        "<h3>Zed Universal Live Preview Server</h3><p>Status: Running</p>",
    )
    .ok();
}

fn handle_open(stream: &mut TcpStream, query: &str, browser: &Arc<dyn BrowserOpener>) {
    let file_param = parse_query_param(query, "file").unwrap_or_default();
    if file_param.is_empty() {
        write_response_text(stream, 400, "Bad Request", &[], "missing file parameter").ok();
        return;
    }

    let is_windows = cfg!(windows);
    let file_path = paths::raw_url_to_fs_path(&file_param, is_windows);

    if !file_path.exists() || file_path.is_dir() {
        write_response_text(stream, 404, "Not Found", &[], "file not found").ok();
        return;
    }

    let path_str = file_path.to_string_lossy().replace('\\', "/");
    let encoded = paths::encode_path_for_url(&path_str);
    let url = format!("http://{}:{}/raw/{}", DEFAULT_HOST, DEFAULT_PORT, encoded);
    browser.open(&url);

    write_response_text(stream, 200, "OK", &[("Content-Type", "text/plain")], "opened").ok();
}

fn parse_query_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return Some(paths::decode_path_component(v));
            }
        }
    }
    None
}

fn handle_sse(mut stream: TcpStream, query: &str, watch_registry: &WatchRegistry) {
    let target_param = parse_query_param(query, "target").unwrap_or_default();
    let is_windows = cfg!(windows);

    let target_path = if target_param.starts_with("/raw/") {
        let raw = target_param.strip_prefix("/raw/").unwrap_or("");
        paths::raw_url_to_fs_path(raw, is_windows)
    } else {
        PathBuf::new()
    };

    // SSE responses must NOT carry Content-Length: the stream stays open and
    // the browser reads events until the connection closes. A Content-Length
    // header would make EventSource treat the response as complete after the
    // first bytes and drop all subsequent reload/css events.
    let headers = [
        ("Content-Type", "text/event-stream"),
        ("Cache-Control", "no-cache, no-transform"),
        ("Connection", "keep-alive"),
        ("Access-Control-Allow-Origin", "*"),
    ];
    let mut response = String::from("HTTP/1.1 200 OK\r\n");
    for (k, v) in &headers {
        response.push_str(&format!("{}: {}\r\n", k, v));
    }
    response.push_str("\r\n: connected\n\n");
    if stream
        .write_all(response.as_bytes())
        .and_then(|_| stream.flush())
        .is_err()
    {
        return;
    }

    let watch_dir = resolve_watch_dir(&target_path);
    if let Some(dir) = watch_dir {
        watch_registry.add_client(dir, stream);
    }
}

/// Test helper: parse a raw HTTP request buffer.
pub fn parse_raw_request(raw: &str) -> Option<(String, String)> {
    parse_request(raw).map(|r| (r.path, r.query))
}

/// Test helper: resolve SSE watch directory from a query string.
pub fn sse_watch_dir_from_query(query: &str, is_windows: bool) -> Option<PathBuf> {
    let target_param = parse_query_param(query, "target").unwrap_or_default();
    let target_path = if target_param.starts_with("/raw/") {
        let raw = target_param.strip_prefix("/raw/").unwrap_or("");
        paths::raw_url_to_fs_path(raw, is_windows)
    } else {
        PathBuf::new()
    };
    resolve_watch_dir(&target_path)
}

fn resolve_watch_dir(target_path: &Path) -> Option<PathBuf> {
    if !target_path.exists() {
        return None;
    }
    if target_path.is_dir() {
        Some(target_path.to_path_buf())
    } else {
        target_path.parent().map(|p| p.to_path_buf())
    }
}

fn try_referer_fallback(stream: &mut TcpStream, req: &HttpRequest, is_windows: bool) -> bool {
    let referer = match header_value(&req.headers, "referer") {
        Some(r) => r,
        None => return false,
    };
    let ref_path = referer
        .split("://")
        .nth(1)
        .and_then(|rest| rest.splitn(2, '/').nth(1))
        .map(|p| format!("/{}", p.split('?').next().unwrap_or(p)));

    let ref_path = match ref_path {
        Some(p) if p.starts_with("/raw/") => p,
        _ => return false,
    };

    let ref_raw = ref_path.strip_prefix("/raw/").unwrap_or("");
    let ref_fs = paths::raw_url_to_fs_path(ref_raw, is_windows);
    let ref_dir = ref_fs.parent().unwrap_or(Path::new(""));

    let sub_path = paths::decode_path_component(&req.path.trim_start_matches('/'));
    let candidate = ref_dir.join(&sub_path);

    if candidate.exists() && !candidate.is_dir() {
        serve_file(stream, &candidate);
        return true;
    }

    false
}

fn serve_raw_path(
    stream: &mut TcpStream,
    raw_part: &str,
    request_path: &str,
    query: &str,
    is_windows: bool,
) {
    let file_path = paths::raw_url_to_fs_path(raw_part, is_windows);

    if !file_path.exists() {
        let body = format!(
            "<h3>404 Not Found</h3><p>File not found: {}</p>",
            file_path.display()
        );
        write_response_text(
            stream,
            404,
            "Not Found",
            &[("Content-Type", "text/html; charset=utf-8")],
            &body,
        )
        .ok();
        return;
    }

    if file_path.is_dir() {
        if !request_path.ends_with('/') {
            let location = format!("{}{}", request_path, '/');
            let location = if query.is_empty() {
                location
            } else {
                format!("{}?{}", location, query)
            };
            write_response_text(stream, 301, "Moved Permanently", &[("Location", &location)], "")
                .ok();
            return;
        }

        let index_html = file_path.join("index.html");
        let index_htm = file_path.join("index.htm");
        if index_html.exists() {
            serve_file(stream, &index_html);
        } else if index_htm.exists() {
            serve_file(stream, &index_htm);
        } else {
            serve_directory_listing(stream, &file_path);
        }
        return;
    }

    serve_file(stream, &file_path);
}

fn serve_directory_listing(stream: &mut TcpStream, dir_path: &Path) {
    let entries = match std::fs::read_dir(dir_path) {
        Ok(entries) => entries,
        Err(_) => {
            let _ = write_response_text(stream, 500, "Internal Server Error", &[], "Read Error");
            return;
        }
    };

    let mut links = String::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let full = dir_path.join(&name);
        let full_str = full.to_string_lossy().replace('\\', "/");
        let href = format!("/raw/{}", paths::encode_path_for_url(&full_str));
        links.push_str(&format!("<a href=\"{}\">{}</a>\n", href, name));
    }

    let body = format!(
        r#"<!DOCTYPE html>
<html>
<head><title>Index of {dir}</title><style>body{{font-family:sans-serif;padding:2rem;}}a{{display:block;margin:6px 0;}}</style></head>
<body>
  <h2>📁 Index of {dir}</h2>
  <hr/>
  {links}
</body>
</html>"#,
        dir = dir_path.display(),
        links = links
    );

    write_response_text(
        stream,
        200,
        "OK",
        &[("Content-Type", "text/html; charset=utf-8")],
        &body,
    )
    .ok();
}

fn serve_file(stream: &mut TcpStream, file_path: &Path) {
    let ext = file_path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    let content_type = mime_type(&ext);

    if ext == ".html" || ext == ".htm" {
        match std::fs::read_to_string(file_path) {
            Ok(data) => {
                let content = if data.contains("</body>") {
                    data.replace("</body>", &format!("{}\n</body>", INJECT_SCRIPT))
                } else {
                    format!("{}{}", data, INJECT_SCRIPT)
                };
                write_response_text(
                    stream,
                    200,
                    "OK",
                    &[
                        ("Content-Type", content_type),
                        ("Access-Control-Allow-Origin", "*"),
                    ],
                    &content,
                )
                .ok();
            }
            Err(err) => {
                write_response_text(
                    stream,
                    500,
                    "Internal Server Error",
                    &[("Content-Type", "text/plain")],
                    &format!("Read Error: {}", err),
                )
                .ok();
            }
        }
    } else {
        match std::fs::File::open(file_path) {
            Ok(mut file) => {
                let headers = [
                    ("Content-Type", content_type),
                    ("Access-Control-Allow-Origin", "*"),
                ];
                let mut response = format!("HTTP/1.1 200 OK\r\nConnection: close\r\n");
                for (k, v) in &headers {
                    response.push_str(&format!("{}: {}\r\n", k, v));
                }
                if let Ok(meta) = file.metadata() {
                    response.push_str(&format!("Content-Length: {}\r\n\r\n", meta.len()));
                } else {
                    response.push_str("\r\n");
                }
                stream.write_all(response.as_bytes()).ok();
                std::io::copy(&mut file, stream).ok();
                stream.flush().ok();
            }
            Err(err) => {
                write_response_text(
                    stream,
                    500,
                    "Internal Server Error",
                    &[("Content-Type", "text/plain")],
                    &format!("Read Error: {}", err),
                )
                .ok();
            }
        }
    }
}
