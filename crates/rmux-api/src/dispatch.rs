//! Bridge types between the socket server and the application.
//!
//! `rmux-api` never touches application state directly. Instead, each
//! parsed request is wrapped in an [`ApiRequestEnvelope`] and forwarded
//! over an `mpsc` channel to the application (which owns all workspace
//! and pane state); the application answers through the enclosed
//! `oneshot` channel. Application-side events flow the other way as
//! [`ApiEvent`] values on a `broadcast` channel, which the server fans
//! out to `events.stream` subscribers.

use serde::{Deserialize, Serialize};

use crate::protocol::JsonRpcError;

/// The outcome the application sends back for a single request.
pub type ApiResponseResult = Result<serde_json::Value, JsonRpcError>;

/// A parsed API request forwarded to the application for handling.
#[derive(Debug)]
pub struct ApiRequestEnvelope {
    /// Method name from the request, e.g. `"workspace.list"`.
    pub method: String,
    /// Raw JSON parameters from the request.
    pub params: serde_json::Value,
    /// The app sends the outcome back through this channel.
    ///
    /// Dropping the sender without answering makes the server report a
    /// timeout ([`crate::protocol::codes::TIMEOUT`]) to the client.
    pub respond: tokio::sync::oneshot::Sender<ApiResponseResult>,
}

/// An event published by the application, broadcast to streaming clients.
///
/// Serialized as one JSON line per event, e.g.
/// `{"event":"workspace.changed","data":{"id":3}}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiEvent {
    /// Event name, e.g. `"workspace.changed"`, `"pane.created"`,
    /// `"notification"`.
    pub event: String,
    /// Event payload.
    pub data: serde_json::Value,
}

impl ApiEvent {
    /// Create an event with the given name and payload.
    ///
    /// # Examples
    ///
    /// ```
    /// use rmux_api::ApiEvent;
    ///
    /// let event = ApiEvent::new("pane.created", serde_json::json!({"pane_id": 7}));
    /// assert_eq!(event.event, "pane.created");
    /// ```
    #[must_use]
    pub fn new(event: impl Into<String>, data: serde_json::Value) -> Self {
        Self { event: event.into(), data }
    }
}
