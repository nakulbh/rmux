#![forbid(unsafe_code)]
//! Socket API server for rmux.
//!
//! Provides a JSON-RPC line protocol over a Unix domain socket
//! for external control of the terminal multiplexer. Supports
//! method dispatch, event streaming, and cmux-compatible commands.
//!
//! # Modules
//!
//! - `server` — Unix socket listener and connection handler
//! - `methods` — Method handler registry and dispatch
//! - `protocol` — JSON-RPC request/response types
//!
//! Will be fully implemented in Phase 3.

pub mod methods;
pub mod protocol;
pub mod server;
