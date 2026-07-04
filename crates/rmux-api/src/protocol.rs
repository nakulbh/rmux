//! JSON-based line protocol types.
//!
//! Defines the request/response/error structures for the
//! newline-delimited JSON protocol used over the Unix socket.
//!
//! Each request and response is a single JSON object on its own line.
//! Requests carry an `id` that is echoed back on the matching response
//! so clients can pipeline calls over one connection.

use serde::{Deserialize, Serialize};

/// Well-known protocol error codes.
///
/// The parse/method-not-found codes follow the JSON-RPC 2.0 spec;
/// the `-32000` range is reserved for implementation-defined errors.
pub mod codes {
    /// The request line was not valid JSON or not a valid request object.
    pub const PARSE_ERROR: i32 = -32700;
    /// The requested method is not known to the application.
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// The server or application failed internally while handling the call.
    pub const INTERNAL_ERROR: i32 = -32000;
    /// The application did not answer the request in time.
    pub const TIMEOUT: i32 = -32001;
}

/// A JSON-based request message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

impl JsonRpcResponse {
    /// Build a successful (`ok: true`) response carrying `result`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rmux_api::protocol::JsonRpcResponse;
    ///
    /// let response = JsonRpcResponse::success(1.into(), serde_json::json!({"pong": true}));
    /// assert!(response.ok);
    /// ```
    #[must_use]
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self { id, ok: true, result: Some(result), error: None }
    }

    /// Build a failed (`ok: false`) response carrying `error`.
    #[must_use]
    pub fn failure(id: serde_json::Value, error: JsonRpcError) -> Self {
        Self { id, ok: false, result: None, error: Some(error) }
    }
}

/// A JSON-based error object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Numeric error code.
    pub code: i32,
    /// Human-readable error message.
    pub message: String,
}

impl JsonRpcError {
    /// Create an error with an arbitrary code and message.
    #[must_use]
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    /// Create a parse error ([`codes::PARSE_ERROR`]).
    #[must_use]
    pub fn parse_error(detail: impl Into<String>) -> Self {
        Self::new(codes::PARSE_ERROR, format!("parse error: {}", detail.into()))
    }

    /// Create a method-not-found error ([`codes::METHOD_NOT_FOUND`]).
    #[must_use]
    pub fn method_not_found(method: &str) -> Self {
        Self::new(codes::METHOD_NOT_FOUND, format!("method not found: {method}"))
    }

    /// Create an internal error ([`codes::INTERNAL_ERROR`]).
    #[must_use]
    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(codes::INTERNAL_ERROR, format!("internal error: {}", detail.into()))
    }

    /// Create a timeout error ([`codes::TIMEOUT`]).
    #[must_use]
    pub fn timeout() -> Self {
        Self::new(codes::TIMEOUT, "request timed out waiting for the application")
    }
}
