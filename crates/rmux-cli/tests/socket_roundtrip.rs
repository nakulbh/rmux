//! Integration tests for the blocking socket client.
//!
//! Spawns a `UnixListener` on a temp path that answers canned
//! `JsonRpcResponse`s, then drives `rmux_cli::socket::{call,stream_events_to}`.
#![cfg(unix)]

mod common;

use rmux_api::methods;
use rmux_api::protocol::{JsonRpcError, JsonRpcResponse};
use rmux_cli::socket;
use serde_json::json;

use common::{
    ok_response, spawn_event_stream_error_server, spawn_event_stream_server, spawn_one_shot_server,
    temp_socket_path,
};

#[test]
fn call_roundtrips_a_successful_response() {
    let path = temp_socket_path("ok");
    let server = spawn_one_shot_server(&path, ok_response(json!({ "pong": true })));

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
    let server = spawn_one_shot_server(&path, ok_response(json!({ "id": "p2" })));

    let params = json!({ "text": "ls\n" });
    let result = socket::call(&path, "surface.send_text", params.clone()).expect("call succeeds");
    assert_eq!(result, json!({ "id": "p2" }));

    let request = server.join().expect("server thread");
    assert_eq!(request.method, "surface.send_text");
    assert_eq!(request.params, params);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn call_treats_missing_result_as_null() {
    let path = temp_socket_path("null-result");
    let response = JsonRpcResponse { id: json!(1), ok: true, result: None, error: None };
    let server = spawn_one_shot_server(&path, response);

    let result = socket::call(&path, "workspace.select", json!({ "index": 0 })).expect("ok");
    assert!(result.is_null());
    let _ = server.join();
    let _ = std::fs::remove_file(&path);
}

#[test]
fn stream_events_to_reads_ack_and_event_lines() {
    let path = temp_socket_path("stream-ok");
    let events = vec![
        json!({ "event": "pane.created", "data": { "pane_id": 1 } }),
        json!({ "event": "workspace.changed", "data": { "id": 2 } }),
    ];
    let server = spawn_event_stream_server(&path, events.clone());

    let mut buf = Vec::new();
    socket::stream_events_to(&path, &mut buf).expect("stream completes");

    let request = server.join().expect("server thread");
    assert_eq!(request.method, methods::EVENTS_STREAM);
    assert_eq!(request.params, json!({}));

    let output = String::from_utf8(buf).expect("utf8");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], events[0].to_string());
    assert_eq!(lines[1], events[1].to_string());

    let _ = std::fs::remove_file(&path);
}

#[test]
fn stream_events_to_propagates_failed_ack() {
    let path = temp_socket_path("stream-err");
    let server = spawn_event_stream_error_server(&path);

    let mut buf = Vec::new();
    let err = socket::stream_events_to(&path, &mut buf).expect_err("failed ack");
    let server_err = err.downcast_ref::<socket::ServerError>().expect("ServerError");
    assert_eq!(server_err.code, -32000);
    assert_eq!(server_err.message, "stream denied");
    assert!(buf.is_empty());

    let request = server.join().expect("server");
    assert_eq!(request.method, methods::EVENTS_STREAM);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn stream_events_to_reports_connect_error() {
    let path = temp_socket_path("stream-missing");
    let mut buf = Vec::new();
    let err = socket::stream_events_to(&path, &mut buf).expect_err("no listener");
    let connect = err.downcast_ref::<socket::ConnectError>().expect("ConnectError");
    assert_eq!(connect.path, path);
}
