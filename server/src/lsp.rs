use serde_json::{json, Value};
use std::io::{BufRead, Write};

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
    let stdin = std::io::stdin();
    let mut reader = std::io::BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();

    while let Some(msg) = read_message(&mut reader) {
        let method = msg.get("method").and_then(|m| m.as_str());

        if msg.get("id").is_some() {
            let id = msg.get("id").cloned().unwrap_or(Value::Null);
            let method = method.unwrap_or("");

            let response = match method {
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
                "textDocument/codeAction" => Value::Array(vec![]),
                "workspace/executeCommand" => Value::Null,
                _ => {
                    let err = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32601,
                            "message": "method not found"
                        }
                    });
                    write_message(&mut writer, &err).ok();
                    continue;
                }
            };

            let resp = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": response
            });
            write_message(&mut writer, &resp).ok();
            continue;
        }

        match method {
            Some("initialized") => {
                std::thread::spawn(|| {
                    // HTTP server starts here in Task 5.
                });
            }
            Some("exit") => std::process::exit(0),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn parses_content_length_frame() {
        let raw = b"Content-Length: 17\r\n\r\n{\"jsonrpc\":\"2.0\"}X";
        let mut cursor = Cursor::new(&raw[..]);
        let msg = read_message(&mut cursor).unwrap();
        assert_eq!(msg["jsonrpc"], "2.0");
    }
}
