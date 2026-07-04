//! Unix socket server module.
//!
//! Listens on a Unix domain socket, accepts client connections, and
//! speaks the newline-delimited JSON protocol from [`crate::protocol`].
//! Requests are forwarded to the application over an `mpsc` channel as
//! [`ApiRequestEnvelope`]s; the special `events.stream` method switches
//! a connection into streaming mode fed by a `broadcast` channel of
//! [`ApiEvent`]s.

use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::sync::{broadcast, mpsc, oneshot};

use crate::dispatch::{ApiEvent, ApiRequestEnvelope};
use crate::error::ApiError;

/// How long the server waits for the application to answer a request
/// before reporting a timeout to the client.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Resolve the socket path: `$RMUX_SOCKET_PATH` override, else
/// `/tmp/rmux-debug.sock` for debug builds, `/tmp/rmux.sock` for release.
///
/// # Examples
///
/// ```
/// let path = rmux_api::default_socket_path();
/// assert!(path.extension().is_some() || path.is_absolute());
/// ```
#[must_use]
pub fn default_socket_path() -> PathBuf {
    socket_path_from_override(std::env::var_os("RMUX_SOCKET_PATH"))
}

/// Resolve the socket path from an explicit (possibly absent) override.
///
/// Split out from [`default_socket_path`] so the resolution logic is
/// testable without mutating process environment variables (which is
/// `unsafe` in edition 2024 and forbidden in this crate).
pub(crate) fn socket_path_from_override(override_path: Option<OsString>) -> PathBuf {
    match override_path {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ if cfg!(debug_assertions) => PathBuf::from("/tmp/rmux-debug.sock"),
        _ => PathBuf::from("/tmp/rmux.sock"),
    }
}

/// JSON-RPC server listening on a Unix domain socket.
///
/// The server owns an accept loop task; each accepted connection is
/// handled on its own task, so multiple concurrent clients are
/// supported. Dropping the server (or calling [`ApiServer::shutdown`])
/// cancels the accept loop and removes the socket file.
///
/// # Examples
///
/// ```no_run
/// use rmux_api::{ApiEvent, ApiRequestEnvelope, ApiServer};
/// use tokio::sync::{broadcast, mpsc};
///
/// # async fn run() -> Result<(), rmux_api::ApiError> {
/// let (request_tx, mut request_rx) = mpsc::channel::<ApiRequestEnvelope>(64);
/// let (event_tx, _) = broadcast::channel::<ApiEvent>(64);
/// let server = ApiServer::bind(rmux_api::default_socket_path(), request_tx, event_tx).await?;
/// while let Some(request) = request_rx.recv().await {
///     let _ = request.respond.send(Ok(serde_json::json!({"pong": true})));
/// }
/// server.shutdown();
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct ApiServer {
    socket_path: PathBuf,
    shutdown_tx: Option<oneshot::Sender<()>>,
    accept_task: Option<tokio::task::JoinHandle<()>>,
}

impl ApiServer {
    /// Bind the server with the default request timeout
    /// ([`DEFAULT_REQUEST_TIMEOUT`]).
    ///
    /// # Errors
    ///
    /// Returns [`ApiError::Bind`] if the socket cannot be bound, or
    /// [`ApiError::UnsupportedPlatform`] on non-Unix platforms.
    pub async fn bind(
        socket_path: impl Into<PathBuf>,
        request_tx: mpsc::Sender<ApiRequestEnvelope>,
        event_tx: broadcast::Sender<ApiEvent>,
    ) -> Result<Self, ApiError> {
        Self::bind_with_timeout(socket_path, request_tx, event_tx, DEFAULT_REQUEST_TIMEOUT).await
    }

    /// Bind the server, waiting at most `request_timeout` for the
    /// application to answer each forwarded request.
    ///
    /// Removes a stale socket file at `socket_path` before binding and
    /// spawns the accept loop on the current Tokio runtime.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError::Bind`] if the socket cannot be bound, or
    /// [`ApiError::UnsupportedPlatform`] on non-Unix platforms.
    #[cfg(unix)]
    pub async fn bind_with_timeout(
        socket_path: impl Into<PathBuf>,
        request_tx: mpsc::Sender<ApiRequestEnvelope>,
        event_tx: broadcast::Sender<ApiEvent>,
        request_timeout: Duration,
    ) -> Result<Self, ApiError> {
        let socket_path = socket_path.into();
        remove_stale_socket(&socket_path);
        let listener = tokio::net::UnixListener::bind(&socket_path)
            .map_err(|source| ApiError::Bind { path: socket_path.clone(), source })?;
        tracing::info!(path = %socket_path.display(), "API server listening");

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let accept_task =
            tokio::spawn(accept_loop(listener, shutdown_rx, request_tx, event_tx, request_timeout));
        Ok(Self { socket_path, shutdown_tx: Some(shutdown_tx), accept_task: Some(accept_task) })
    }

    /// Bind the server (non-Unix stub).
    ///
    /// # Errors
    ///
    /// Always returns [`ApiError::UnsupportedPlatform`].
    #[cfg(not(unix))]
    pub async fn bind_with_timeout(
        socket_path: impl Into<PathBuf>,
        _request_tx: mpsc::Sender<ApiRequestEnvelope>,
        _event_tx: broadcast::Sender<ApiEvent>,
        _request_timeout: Duration,
    ) -> Result<Self, ApiError> {
        let _ = socket_path.into();
        Err(ApiError::UnsupportedPlatform)
    }

    /// The socket path this server is bound to.
    #[must_use]
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Stop accepting connections and remove the socket file.
    ///
    /// Existing connections are dropped along with the accept task.
    /// Dropping the server has the same effect.
    pub fn shutdown(mut self) {
        self.shutdown_impl();
    }

    fn shutdown_impl(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.accept_task.take() {
            task.abort();
        }
        if let Err(err) = std::fs::remove_file(&self.socket_path)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            tracing::debug!(
                path = %self.socket_path.display(),
                error = %err,
                "failed to remove socket file",
            );
        }
    }
}

impl Drop for ApiServer {
    fn drop(&mut self) {
        self.shutdown_impl();
    }
}

/// Remove a leftover socket file from a previous run, if any.
///
/// Only removes the path if it is an actual Unix socket to avoid
/// accidentally deleting a non-socket file the user may have pointed
/// `$RMUX_SOCKET_PATH` at (data file, symlink, etc.).
#[cfg(unix)]
fn remove_stale_socket(path: &Path) {
    // Stat the path; bail if metadata isn't available or it's not a socket.
    match std::fs::metadata(path) {
        Ok(meta) if meta.file_type().is_socket() => {}
        Ok(_) => {
            tracing::warn!(
                path = %path.display(),
                "socket path exists but is not a Unix socket; refusing to remove",
            );
            return;
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => {
            tracing::warn!(path = %path.display(), error = %err, "could not stat socket path");
            return;
        }
    }
    match std::fs::remove_file(path) {
        Ok(()) => tracing::debug!(path = %path.display(), "removed stale socket file"),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            tracing::warn!(path = %path.display(), error = %err, "could not remove stale socket");
        }
    }
}

/// Accept connections until shut down, spawning a handler task each.
#[cfg(unix)]
async fn accept_loop(
    listener: tokio::net::UnixListener,
    mut shutdown_rx: oneshot::Receiver<()>,
    request_tx: mpsc::Sender<ApiRequestEnvelope>,
    event_tx: broadcast::Sender<ApiEvent>,
    request_timeout: Duration,
) {
    loop {
        tokio::select! {
            _ = &mut shutdown_rx => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, _addr)) => {
                    tokio::spawn(crate::connection::handle_connection(
                        stream,
                        request_tx.clone(),
                        event_tx.clone(),
                        request_timeout,
                    ));
                }
                Err(err) => tracing::warn!(error = %err, "failed to accept connection"),
            },
        }
    }
    tracing::debug!("API server accept loop stopped");
}
