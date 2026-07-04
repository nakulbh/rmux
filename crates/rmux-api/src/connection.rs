//! Per-connection protocol handling (Unix only).
//!
//! Each accepted connection runs [`handle_connection`] on its own task:
//! read a JSON line, forward it to the application, write the JSON
//! response line. The `events.stream` method is intercepted here and
//! turns the connection into a one-way event feed.

#![cfg(unix)]

use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::net::unix::OwnedWriteHalf;
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::dispatch::{ApiEvent, ApiRequestEnvelope};
use crate::methods;
use crate::protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};

/// Serve one client connection until it disconnects.
pub(crate) async fn handle_connection(
    stream: UnixStream,
    request_tx: mpsc::Sender<ApiRequestEnvelope>,
    event_tx: broadcast::Sender<ApiEvent>,
    request_timeout: Duration,
) {
    let (read_half, mut write_half) = stream.into_split();
    let mut lines = BufReader::new(read_half).lines();

    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(err) => {
                tracing::debug!(error = %err, "connection read failed");
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(err) => {
                // Report the parse error but keep the connection open so
                // the client can retry with a well-formed line.
                let response = JsonRpcResponse::failure(
                    serde_json::Value::Null,
                    JsonRpcError::parse_error(err.to_string()),
                );
                if write_response(&mut write_half, &response).await.is_err() {
                    break;
                }
                continue;
            }
        };

        if request.method == methods::EVENTS_STREAM {
            // Subscribe BEFORE acking so no event published after the
            // client observes the ack can be missed.
            stream_events(write_half, event_tx.subscribe(), request.id).await;
            return;
        }

        let response = dispatch_request(request, &request_tx, request_timeout).await;
        if write_response(&mut write_half, &response).await.is_err() {
            break;
        }
    }
}

/// Forward one request to the application and await its outcome.
async fn dispatch_request(
    request: JsonRpcRequest,
    request_tx: &mpsc::Sender<ApiRequestEnvelope>,
    request_timeout: Duration,
) -> JsonRpcResponse {
    let JsonRpcRequest { id, method, params } = request;
    let (respond, response_rx) = oneshot::channel();
    let envelope = ApiRequestEnvelope { method, params, respond };

    if request_tx.send(envelope).await.is_err() {
        return JsonRpcResponse::failure(id, JsonRpcError::internal("application unavailable"));
    }

    match tokio::time::timeout(request_timeout, response_rx).await {
        Ok(Ok(Ok(result))) => JsonRpcResponse::success(id, result),
        Ok(Ok(Err(error))) => JsonRpcResponse::failure(id, error),
        // The app dropped the responder without answering, or never
        // answered in time — either way the client sees a timeout.
        Ok(Err(_)) | Err(_) => JsonRpcResponse::failure(id, JsonRpcError::timeout()),
    }
}

/// Ack the `events.stream` request, then forward broadcast events as
/// JSON lines until the client disconnects or the channel closes.
async fn stream_events(
    mut write_half: OwnedWriteHalf,
    mut events: broadcast::Receiver<ApiEvent>,
    request_id: serde_json::Value,
) {
    let ack = JsonRpcResponse::success(request_id, serde_json::json!({"streaming": true}));
    if write_response(&mut write_half, &ack).await.is_err() {
        return;
    }

    loop {
        match events.recv().await {
            Ok(event) => {
                if write_json_line(&mut write_half, &event).await.is_err() {
                    break; // client disconnected
                }
            }
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                tracing::warn!(skipped, "event stream lagged; skipping missed events");
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

/// Write a response as one newline-terminated JSON line.
async fn write_response(
    write_half: &mut OwnedWriteHalf,
    response: &JsonRpcResponse,
) -> std::io::Result<()> {
    write_json_line(write_half, response).await
}

/// Serialize `value` and write it as one newline-terminated JSON line.
async fn write_json_line<T: serde::Serialize>(
    write_half: &mut OwnedWriteHalf,
    value: &T,
) -> std::io::Result<()> {
    let mut line = serde_json::to_string(value).map_err(std::io::Error::other)?;
    line.push('\n');
    write_half.write_all(line.as_bytes()).await?;
    write_half.flush().await
}
