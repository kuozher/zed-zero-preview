use crate::browser::{BrowserOpener, DefaultBrowser};
use crate::http::{self, HttpConfig, HttpServerHandle};
use crate::paths;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};

pub struct LspState {
    pub client_mode: bool,
    pub http_server: Option<HttpServerHandle>,
    pub browser: Arc<dyn BrowserOpener>,
    writer: Mutex<Option<std::io::Stdout>>,
}

impl LspState {
    pub fn new(browser: Arc<dyn BrowserOpener>) -> Self {
        Self {
            client_mode: false,
            http_server: None,
            browser,
            writer: Mutex::new(None),
        }
    }

    pub fn set_writer(&self, writer: std::io::Stdout) {
        *self.writer.lock().unwrap() = Some(writer);
    }

    fn show_message(&self, message: &str) {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "window/showMessage",
            "params": {
                "type": 1,
                "message": message
            }
        });
        if let Some(writer) = self.writer.lock().unwrap().as_mut() {
            write_message(writer, &notification).ok();
        }
    }

    pub fn start_http_server(&mut self) {
        if self.http_server.is_some() || self.client_mode {
            return;
        }

        let config = HttpConfig {
            host: http::DEFAULT_HOST.to_string(),
            port: http::DEFAULT_PORT,
            stop_exits_process: true,
            browser: self.browser.clone(),
        };

        match http::start_server(config) {
            Ok(handle) => {
                self.http_server = Some(handle);
            }
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
                if http::ping_server(http::DEFAULT_PORT) {
                    self.client_mode = true;
                } else {
                    self.show_message(
                        "Zero Preview: port 52331 is in use by another program. Stop it or close other Zed windows using preview.",
                    );
                }
            }
            Err(err) => {
                self.show_message(&format!(
                    "Zero Preview: failed to start HTTP server: {}",
                    err
                ));
            }
        }
    }

    pub fn handle_code_action(&self, params: &Value) -> Value {
        let uri = params
            .get("textDocument")
            .and_then(|d| d.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("");

        json!([
            {
                "title": "Zero Preview: Open in Browser",
                "kind": "",
                "command": {
                    "title": "open",
                    "command": "zero-preview.open",
                    "arguments": [uri]
                }
            },
            {
                "title": "Zero Preview: Stop Preview Server",
                "command": {
                    "title": "stop",
                    "command": "zero-preview.stop",
                    "arguments": []
                }
            }
        ])
    }

    pub fn handle_execute_command(&mut self, params: &Value) -> Value {
        let command = params
            .get("command")
            .and_then(|c| c.as_str())
            .unwrap_or("");
        let arguments = params.get("arguments").and_then(|a| a.as_array());

        match command {
            "zero-preview.open" => {
                let uri = arguments
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                self.open_preview(uri);
            }
            "zero-preview.stop" => {
                self.stop_preview();
            }
            _ => {}
        }
        Value::Null
    }

    fn open_preview(&mut self, file_uri: &str) {
        if !self.client_mode && self.http_server.is_none() {
            self.start_http_server();
        }

        let is_windows = cfg!(windows);
        let file_path = paths::file_uri_to_path(file_uri, is_windows);
        let path_str = file_path.to_string_lossy().replace('\\', "/");
        let encoded = paths::encode_path_for_url(&path_str);

        if self.client_mode {
            let query = format!(
                "/__open?file={}",
                percent_encode_query(&path_str)
            );
            let _ = http::http_get(http::DEFAULT_HOST, http::DEFAULT_PORT, &query);
        } else {
            let url = format!(
                "http://{}:{}/raw/{}",
                http::DEFAULT_HOST,
                http::DEFAULT_PORT,
                encoded
            );
            self.browser.open(&url);
        }
    }

    fn stop_preview(&mut self) {
        if self.client_mode {
            self.show_message(
                "Zero Preview: preview server is shared with another Zed window; stop it from the window that started the server.",
            );
        } else if let Some(server) = self.http_server.take() {
            server.stop();
        }
    }
}

fn percent_encode_query(s: &str) -> String {
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

pub fn read_message<R: BufRead>(reader: &mut R) -> Option<Value> {
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => return None,
            Ok(_) => {}
            Err(_) => return None,
        }

        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }

        if let Some(len_str) = line.strip_prefix("Content-Length: ") {
            content_length = len_str.trim().parse().ok();
        }
    }

    let len = content_length?;
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

pub fn write_message<W: Write>(writer: &mut W, value: &Value) -> std::io::Result<()> {
    let json = serde_json::to_string(value).expect("json serialization should not fail");
    write!(writer, "Content-Length: {}\r\n\r\n{}", json.len(), json)?;
    writer.flush()?;
    Ok(())
}

pub fn run_lsp() {
    let browser: Arc<dyn BrowserOpener> = Arc::new(DefaultBrowser);
    let state = Arc::new(Mutex::new(LspState::new(browser)));

    let stdin = std::io::stdin();
    let mut reader = std::io::BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    state.lock().unwrap().set_writer(stdout);

    while let Some(msg) = read_message(&mut reader) {
        let method = msg.get("method").and_then(|m| m.as_str());

        if msg.get("id").is_some() {
            let id = msg.get("id").cloned().unwrap_or(Value::Null);
            let method = method.unwrap_or("");

            let response = {
                let mut guard = state.lock().unwrap();
                match method {
                    "initialize" => json!({
                        "capabilities": {
                            "codeActionProvider": true,
                            "executeCommandProvider": {
                                "commands": ["zero-preview.open", "zero-preview.stop"]
                            },
                            "textDocumentSync": 0
                        },
                        "serverInfo": {
                            "name": "zero-preview",
                            "version": "0.1.0"
                        }
                    }),
                    "shutdown" => Value::Null,
                    "textDocument/codeAction" => {
                        guard.handle_code_action(msg.get("params").unwrap_or(&Value::Null))
                    }
                    "workspace/executeCommand" => guard.handle_execute_command(
                        msg.get("params").unwrap_or(&Value::Null),
                    ),
                    _ => {
                        let err = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": {
                                "code": -32601,
                                "message": "method not found"
                            }
                        });
                        // Reuse `guard`: locking `state` again here would
                        // deadlock (std Mutex is not reentrant).
                        if let Some(writer) = guard.writer.lock().unwrap().as_mut() {
                            write_message(writer, &err).ok();
                        }
                        continue;
                    }
                }
            };

            let resp = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": response
            });
            if let Some(writer) = state.lock().unwrap().writer.lock().unwrap().as_mut() {
                write_message(writer, &resp).ok();
            }
            continue;
        }

        match method {
            Some("initialized") => {
                let mut guard = state.lock().unwrap();
                guard.start_http_server();
            }
            Some("exit") => std::process::exit(0),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::MockBrowser;
    use std::io::Cursor;

    #[test]
    fn parses_content_length_frame() {
        let raw = b"Content-Length: 17\r\n\r\n{\"jsonrpc\":\"2.0\"}X";
        let mut cursor = Cursor::new(&raw[..]);
        let msg = read_message(&mut cursor).unwrap();
        assert_eq!(msg["jsonrpc"], "2.0");
    }

    #[test]
    fn code_action_returns_open_and_stop() {
        let browser = Arc::new(MockBrowser::new());
        let state = LspState::new(browser);
        let actions = state.handle_code_action(&json!({
            "textDocument": { "uri": "file:///tmp/test.html" }
        }));
        let arr = actions.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["command"]["command"], "zero-preview.open");
        assert_eq!(arr[1]["command"]["command"], "zero-preview.stop");
    }

    #[test]
    fn execute_open_uses_browser_in_server_mode() {
        let browser = Arc::new(MockBrowser::new());
        let mut state = LspState::new(browser.clone());
        state.handle_execute_command(&json!({
            "command": "zero-preview.open",
            "arguments": ["file:///tmp/page.html"]
        }));
        let urls = browser.urls.lock().unwrap();
        assert_eq!(urls.len(), 1);
        assert!(urls[0].contains("/raw/"));
        assert!(urls[0].contains("page.html"));
    }
}
