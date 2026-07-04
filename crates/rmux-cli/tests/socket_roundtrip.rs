//! Integration tests for the blocking socket client.
//!
//! Spawns a `UnixListener` on a temp path in a background thread that
//! answers one request with a canned `JsonRpcResponse`, then drives
//! `rmux_cli::socket::call` against it and asserts the roundtrip.
#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

use rmux_api::protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use rmux_cli::socket;
use serde_json::json;

/// Build a unique socket path in the system temp directory.
fn temp_socket_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("rmux-cli-test-{}-{name}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

/// Bind a listener and answer exactly one request with `response`.
///
/// Returns a handle yielding the request the fake server received.
fn spawn_one_shot_server(path: &Path, response: JsonRpcResponse) -> JoinHandle<JsonRpcRequest> {
    let listener = UnixListener::bind(path).expect("bind test socket");
    std::thread::spawn(move || {
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

#[test]
fn call_roundtrips_a_successful_response() {
    let path = temp_socket_path("ok");
    let response = JsonRpcResponse {
        id: json!(1),
        ok: true,
        result: Some(json!({ "pong": true })),
        error: None,
    };
    let server = spawn_one_shot_server(&path, response);

    let result = socket::call(&path, "system.ping", json!({})).expect("call succeeds");
    assert_eq!(result, json!({ "pong": true }));

    let request = server.join().expect("server thread");
    assert_eq!(request.id, json!(1));
    assert_eq!(request.method, "system.ping");
    assert_eq!(request.params, json!({}));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn call_converts_a_server_error_into_server_error() {
    let path = temp_socket_path("err");
    let response = JsonRpcResponse {
        id: json!(1),
        ok: false,
        result: None,
        error: Some(JsonRpcError { code: -32601, message: "method not found".to_owned() }),
    };
    let server = spawn_one_shot_server(&path, response);

    let err = socket::call(&path, "system.nope", json!({})).expect_err("call fails");
    let server_err = err.downcast_ref::<socket::ServerError>().expect("is ServerError");
    assert_eq!(server_err.code, -32601);
    assert_eq!(server_err.message, "method not found");

    let request = server.join().expect("server thread");
    assert_eq!(request.method, "system.nope");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn call_reports_connect_error_when_socket_is_missing() {
    let path = temp_socket_path("missing");

    let err = socket::call(&path, "system.ping", json!({})).expect_err("no server listening");
    let connect = err.downcast_ref::<socket::ConnectError>().expect("is ConnectError");
    assert_eq!(connect.path, path);
}

#[test]
fn call_sends_params_through_unchanged() {
    let path = temp_socket_path("params");
    let response = JsonRpcResponse {
        id: json!(1),
        ok: true,
        result: Some(json!({ "id": "p2" })),
        error: None,
    };
    let server = spawn_one_shot_server(&path, response);

    let params = json!({ "text": "ls\n" });
    let result = socket::call(&path, "surface.send_text", params.clone()).expect("call succeeds");
    assert_eq!(result, json!({ "id": "p2" }));

    let request = server.join().expect("server thread");
    assert_eq!(request.method, "surface.send_text");
    assert_eq!(request.params, params);

    let _ = std::fs::remove_file(&path);
}
