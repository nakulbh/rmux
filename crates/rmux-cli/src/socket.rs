//! Socket path resolution and the blocking line-protocol client.
//!
//! Speaks the newline-delimited JSON protocol defined in
//! [`rmux_api::protocol`]: one serialized [`JsonRpcRequest`] line out,
//! one [`JsonRpcResponse`] line back.
//!
//! [`JsonRpcRequest`]: rmux_api::protocol::JsonRpcRequest
//! [`JsonRpcResponse`]: rmux_api::protocol::JsonRpcResponse

use std::fmt;
use std::path::{Path, PathBuf};

/// Environment variable that overrides the default socket path.
pub const SOCKET_PATH_ENV: &str = "RMUX_SOCKET_PATH";

/// Default socket path for debug builds.
const DEBUG_SOCKET_PATH: &str = "/tmp/rmux-debug.sock";
/// Default socket path for release builds.
const RELEASE_SOCKET_PATH: &str = "/tmp/rmux.sock";

/// Error raised when the client cannot connect to the rmux socket.
///
/// The CLI maps this to exit code 2 with a "is rmux running?" hint.
#[derive(Debug)]
pub struct ConnectError {
    /// The socket path the connection was attempted against.
    pub path: PathBuf,
    /// The underlying I/O error (e.g. connection refused, not found).
    pub source: std::io::Error,
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot connect to rmux at {}: {}", self.path.display(), self.source)
    }
}

impl std::error::Error for ConnectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Error raised when the server answers with `ok: false`.
///
/// Carries the JSON-RPC error code and message; the CLI maps this to
/// exit code 1 and prints `error [<code>]: <message>`.
#[derive(Debug)]
pub struct ServerError {
    /// Numeric JSON-RPC error code (e.g. `-32601` for unknown method).
    pub code: i32,
    /// Human-readable error message from the server.
    pub message: String,
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "server error [{}]: {}", self.code, self.message)
    }
}

impl std::error::Error for ServerError {}

/// Resolve the rmux control socket path.
///
/// Resolution order: the `$RMUX_SOCKET_PATH` environment variable if set
/// and non-empty, else `/tmp/rmux-debug.sock` in debug builds or
/// `/tmp/rmux.sock` in release builds.
// TODO: unify with rmux_api::default_socket_path once merged
pub fn resolve_socket_path() -> PathBuf {
    resolve_from(None, std::env::var(SOCKET_PATH_ENV).ok())
}

/// Resolve the socket path, giving a CLI `--socket` flag top precedence.
///
/// Precedence: `flag` > `$RMUX_SOCKET_PATH` > built-in default.
pub fn effective_socket_path(flag: Option<PathBuf>) -> PathBuf {
    resolve_from(flag, std::env::var(SOCKET_PATH_ENV).ok())
}

/// Pure resolution helper so precedence is testable without touching
/// process environment (mutating env vars is unsafe in edition 2024).
fn resolve_from(flag: Option<PathBuf>, env_value: Option<String>) -> PathBuf {
    if let Some(path) = flag {
        return path;
    }
    if let Some(value) = env_value
        && !value.is_empty()
    {
        return PathBuf::from(value);
    }
    if cfg!(debug_assertions) {
        PathBuf::from(DEBUG_SOCKET_PATH)
    } else {
        PathBuf::from(RELEASE_SOCKET_PATH)
    }
}

/// Perform a single blocking request/response roundtrip over the socket.
///
/// Serializes a [`rmux_api::protocol::JsonRpcRequest`] with a fixed id of
/// `1`, writes it as one newline-terminated line, reads one response line
/// and parses it as a [`rmux_api::protocol::JsonRpcResponse`]. Read and
/// write timeouts are set to 5 seconds.
///
/// # Errors
///
/// - [`ConnectError`] when the socket cannot be connected to
/// - [`ServerError`] when the server responds with `ok: false`
/// - Any other I/O or (de)serialization failure, with context
#[cfg(unix)]
pub fn call(
    path: &Path,
    method: &str,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    use anyhow::Context;
    use rmux_api::protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};

    let stream = UnixStream::connect(path)
        .map_err(|source| ConnectError { path: path.to_path_buf(), source })?;
    let timeout = Some(Duration::from_secs(5));
    stream.set_read_timeout(timeout).context("failed to set socket read timeout")?;
    stream.set_write_timeout(timeout).context("failed to set socket write timeout")?;

    let request =
        JsonRpcRequest { id: serde_json::Value::from(1), method: method.to_owned(), params };
    let mut line = serde_json::to_string(&request).context("failed to serialize request")?;
    line.push('\n');

    (&stream)
        .write_all(line.as_bytes())
        .with_context(|| format!("failed to send request to {}", path.display()))?;

    let mut reader = BufReader::new(&stream);
    let mut response_line = String::new();
    reader
        .read_line(&mut response_line)
        .with_context(|| format!("failed to read response from {}", path.display()))?;
    anyhow::ensure!(
        !response_line.trim().is_empty(),
        "rmux closed the connection without a response"
    );

    let response: JsonRpcResponse =
        serde_json::from_str(response_line.trim()).context("failed to parse response JSON")?;
    if response.ok {
        Ok(response.result.unwrap_or(serde_json::Value::Null))
    } else {
        let error = response.error.unwrap_or(JsonRpcError {
            code: -32603,
            message: "server reported failure without an error object".to_owned(),
        });
        Err(ServerError { code: error.code, message: error.message }.into())
    }
}

/// Stub for non-Unix targets: the socket API always errors.
///
/// # Errors
///
/// Always returns an error; the Unix-socket transport is not available.
#[cfg(not(unix))]
pub fn call(
    _path: &Path,
    _method: &str,
    _params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    anyhow::bail!("the rmux socket API is not supported on this platform yet")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_takes_precedence_over_env() {
        let path =
            resolve_from(Some(PathBuf::from("/tmp/flag.sock")), Some("/tmp/env.sock".to_owned()));
        assert_eq!(path, PathBuf::from("/tmp/flag.sock"));
    }

    #[test]
    fn env_takes_precedence_over_default() {
        let path = resolve_from(None, Some("/tmp/env.sock".to_owned()));
        assert_eq!(path, PathBuf::from("/tmp/env.sock"));
    }

    #[test]
    fn empty_env_falls_back_to_default() {
        let path = resolve_from(None, Some(String::new()));
        assert_eq!(path, resolve_from(None, None));
    }

    #[test]
    fn default_matches_build_profile() {
        let expected = if cfg!(debug_assertions) { DEBUG_SOCKET_PATH } else { RELEASE_SOCKET_PATH };
        assert_eq!(resolve_from(None, None), PathBuf::from(expected));
    }
}
