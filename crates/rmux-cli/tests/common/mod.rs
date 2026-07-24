//! Shared fake Unix-socket helpers for `rmux-cli` integration tests.
#![cfg(unix)]
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use rmux_api::protocol::{JsonRpcRequest, JsonRpcResponse};
use serde_json::{Value, json};

/// Unique temp socket path for a named test case.
pub fn temp_socket_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("rmux-cli-test-{}-{name}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

/// Successful response envelope with the given result.
pub fn ok_response(result: Value) -> JsonRpcResponse {
    JsonRpcResponse { id: json!(1), ok: true, result: Some(result), error: None }
}

/// Bind a listener and answer exactly one request with `response`.
///
/// Returns a handle yielding the request the fake server received.
pub fn spawn_one_shot_server(path: &Path, response: JsonRpcResponse) -> JoinHandle<JsonRpcRequest> {
    let listener = UnixListener::bind(path).expect("bind test socket");
    thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept connection");
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line).expect("read request line");
        let request: JsonRpcRequest = serde_json::from_str(line.trim()).expect("parse request");
        let mut reply = serde_json::to_string(&response).expect("serialize response");
        reply.push('\n');
        (&stream).write_all(reply.as_bytes()).expect("write response");
        request
    })
}

/// Bind a listener and answer `count` sequential requests with the same canned success.
///
/// Records every request for later assertions.
pub fn spawn_multi_shot_server(
    path: &Path,
    count: usize,
    result: Value,
) -> (JoinHandle<()>, Arc<Mutex<Vec<JsonRpcRequest>>>) {
    let listener = UnixListener::bind(path).expect("bind multi-shot socket");
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let recorded_thread = Arc::clone(&recorded);
    let handle = thread::spawn(move || {
        for _ in 0..count {
            let (stream, _) = listener.accept().expect("accept connection");
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            reader.read_line(&mut line).expect("read request line");
            let request: JsonRpcRequest = serde_json::from_str(line.trim()).expect("parse request");
            recorded_thread.lock().expect("lock").push(request);
            let response = ok_response(result.clone());
            let mut reply = serde_json::to_string(&response).expect("serialize response");
            reply.push('\n');
            (&stream).write_all(reply.as_bytes()).expect("write response");
        }
    });
    (handle, recorded)
}

/// Event-stream fake server: ack + write each event line, then close.
pub fn spawn_event_stream_server(path: &Path, events: Vec<Value>) -> JoinHandle<JsonRpcRequest> {
    let listener = UnixListener::bind(path).expect("bind event stream socket");
    thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept stream connection");
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line).expect("read stream request");
        let request: JsonRpcRequest = serde_json::from_str(line.trim()).expect("parse request");

        let ack = ok_response(json!({ "streaming": true }));
        let mut reply = serde_json::to_string(&ack).expect("serialize ack");
        reply.push('\n');
        (&stream).write_all(reply.as_bytes()).expect("write ack");

        for event in events {
            let mut event_line = serde_json::to_string(&event).expect("serialize event");
            event_line.push('\n');
            (&stream).write_all(event_line.as_bytes()).expect("write event");
        }
        // Drop stream to signal EOF.
        request
    })
}

/// Event-stream fake server that returns a failed ack.
pub fn spawn_event_stream_error_server(path: &Path) -> JoinHandle<JsonRpcRequest> {
    use rmux_api::protocol::JsonRpcError;

    let listener = UnixListener::bind(path).expect("bind event error socket");
    thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line).expect("read request");
        let request: JsonRpcRequest = serde_json::from_str(line.trim()).expect("parse");
        let response = JsonRpcResponse {
            id: json!(1),
            ok: false,
            result: None,
            error: Some(JsonRpcError { code: -32000, message: "stream denied".to_owned() }),
        };
        let mut reply = serde_json::to_string(&response).expect("serialize");
        reply.push('\n');
        (&stream).write_all(reply.as_bytes()).expect("write error ack");
        request
    })
}
