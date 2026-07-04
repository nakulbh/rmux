//! Error types for the socket API server.

use std::path::PathBuf;

use thiserror::Error;

/// Errors produced by the socket API server.
#[derive(Debug, Error)]
pub enum ApiError {
    /// Binding the Unix listener to the socket path failed.
    #[error("failed to bind socket at {path}: {source}")]
    Bind {
        /// The socket path the server tried to bind.
        path: PathBuf,
        /// The underlying OS error.
        source: std::io::Error,
    },

    /// A socket I/O operation failed.
    #[error("socket I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Unix domain sockets are not available on this platform.
    #[error("Unix domain sockets are not supported on this platform")]
    UnsupportedPlatform,
}
