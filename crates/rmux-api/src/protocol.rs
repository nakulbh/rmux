//! JSON-based line protocol types.
//!
//! Defines the request/response/error structures for the
//! newline-delimited JSON protocol used over the Unix socket.
//!
//! Will be fully implemented in Phase 3.

use serde::{Deserialize, Serialize};

/// A JSON-based request message.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// Request identifier (string or number).
    pub id: serde_json::Value,
    /// Method name to invoke.
    pub method: String,
    /// Method parameters.
    #[serde(default)]
    pub params: serde_json::Value,
}

/// A JSON-based response envelope.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// Matches the request `id`.
    pub id: serde_json::Value,
    /// Whether the call succeeded.
    pub ok: bool,
    /// Result on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error on failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// A JSON-based error object.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Numeric error code.
    pub code: i32,
    /// Human-readable error message.
    pub message: String,
}
