use serde_json::json;
use std::io::BufReader;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Regression test: an unknown request method must get a -32601 error
/// response. A previous version deadlocked here (re-locking the state mutex
/// while already holding it), freezing the whole LSP loop.
#[test]
fn unknown_method_gets_error_response_without_hanging() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_zero-preview-server"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn server binary");

    let mut stdin = child.stdin.take().expect("child stdin");
    let stdout = child.stdout.take().expect("child stdout");

    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        while let Some(msg) = zero_preview_server::lsp::read_message(&mut reader) {
            if tx.send(msg).is_err() {
                break;
            }
        }
    });

    // initialize (do NOT send `initialized`, so no HTTP port is bound in tests)
    zero_preview_server::lsp::write_message(
        &mut stdin,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    )
    .expect("write initialize");

    let init = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("no initialize response");
    assert_eq!(init["result"]["capabilities"]["codeActionProvider"], true);
    assert!(
        init["result"]["capabilities"].get("completionProvider").is_none(),
        "must not advertise completions"
    );

    // unknown request method must not hang the loop
    zero_preview_server::lsp::write_message(
        &mut stdin,
        &json!({"jsonrpc": "2.0", "id": 2, "method": "textDocument/hover", "params": {}}),
    )
    .expect("write unknown request");

    let resp = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("server hung on unknown method (deadlock regression)");
    assert_eq!(resp["id"], 2);
    assert_eq!(resp["error"]["code"], -32601);

    // clean shutdown
    zero_preview_server::lsp::write_message(
        &mut stdin,
        &json!({"jsonrpc": "2.0", "id": 3, "method": "shutdown"}),
    )
    .expect("write shutdown");
    let shutdown = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("no shutdown response");
    assert_eq!(shutdown["id"], 3);

    zero_preview_server::lsp::write_message(
        &mut stdin,
        &json!({"jsonrpc": "2.0", "method": "exit"}),
    )
    .expect("write exit");

    let status = child.wait().expect("wait for child");
    assert!(status.success(), "server should exit cleanly");
    reader.join().expect("reader thread");
}
