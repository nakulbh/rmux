#![forbid(unsafe_code)]
//! Socket API server for rmux.
//!
//! Provides a JSON-RPC line protocol over a Unix domain socket
//! for external control of the terminal multiplexer. Supports
//! method dispatch, event streaming, and cmux-compatible commands.
//!
//! # Modules
//!
//! - [`server`] — Unix socket listener, accept loop, and lifecycle
//! - [`dispatch`] — Bridge types between the server and the application
//! - [`methods`] — Method-name constants and typed param/result structs
//! - [`protocol`] — JSON-RPC request/response types and error codes
//!
//! # Architecture
//!
//! This crate never touches application state: parsed requests are
//! forwarded as [`ApiRequestEnvelope`]s over an `mpsc` channel to the
//! application, which answers through a `oneshot` channel. Application
//! events fan out to `events.stream` clients via a `broadcast` channel
//! of [`ApiEvent`]s.

mod connection;
pub mod dispatch;
mod error;
pub mod methods;
pub mod protocol;
pub mod server;

pub use dispatch::{ApiEvent, ApiRequestEnvelope, ApiResponseResult};
pub use error::ApiError;
pub use protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
pub use server::{ApiServer, DEFAULT_REQUEST_TIMEOUT, default_socket_path};

#[cfg(test)]
mod tests;
