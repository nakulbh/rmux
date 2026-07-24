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

/// Environment variable that overrides the default socket path
/// (honored by [`rmux_api::default_socket_path`]).
pub const SOCKET_PATH_ENV: &str = "RMUX_SOCKET_PATH";

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

/// Resolve the socket path, giving a CLI `--socket` flag top precedence.
///
/// Precedence: `flag` > `$RMUX_SOCKET_PATH` > built-in default. The
/// env-var and default handling is delegated to
/// [`rmux_api::default_socket_path`] so the CLI and the app always
/// agree on the socket location.
pub fn effective_socket_path(flag: Option<PathBuf>) -> PathBuf {
    flag.unwrap_or_else(rmux_api::default_socket_path)
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

/// Subscribe to `events.stream` and print each event as one NDJSON line on stdout.
///
/// # Errors
///
/// See [`stream_events_to`].
#[cfg(unix)]
pub fn stream_events(path: &Path) -> anyhow::Result<()> {
    stream_events_to(path, &mut std::io::stdout())
}

/// Subscribe to `events.stream` and write each event as one NDJSON line to `out`.
///
/// Sends the stream request, checks the ack, then reads event lines with
/// no read timeout until the server closes the connection or the client
/// is interrupted (Ctrl-C).
///
/// # Errors
///
/// - [`ConnectError`] when the socket cannot be connected to
/// - [`ServerError`] when the ack reports failure
/// - I/O errors after the stream starts (other than clean EOF)
#[cfg(unix)]
pub fn stream_events_to(path: &Path, out: &mut dyn std::io::Write) -> anyhow::Result<()> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    use anyhow::Context;
    use rmux_api::methods;
    use rmux_api::protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
    use serde_json::json;

    let stream = UnixStream::connect(path)
        .map_err(|source| ConnectError { path: path.to_path_buf(), source })?;
    // Only a write timeout: reads stay blocking so sparse events work.
    // (Changing read timeouts after I/O starts is EINVAL on some platforms.)
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .context("failed to set socket write timeout")?;

    let request = JsonRpcRequest {
        id: serde_json::Value::from(1),
        method: methods::EVENTS_STREAM.to_owned(),
        params: json!({}),
    };
    let mut line = serde_json::to_string(&request).context("failed to serialize request")?;
    line.push('\n');
    (&stream)
        .write_all(line.as_bytes())
        .with_context(|| format!("failed to send request to {}", path.display()))?;

    let mut reader = BufReader::new(&stream);
    let mut ack_line = String::new();
    reader
        .read_line(&mut ack_line)
        .with_context(|| format!("failed to read stream ack from {}", path.display()))?;
    anyhow::ensure!(!ack_line.trim().is_empty(), "rmux closed the connection without a stream ack");

    let ack: JsonRpcResponse =
        serde_json::from_str(ack_line.trim()).context("failed to parse stream ack JSON")?;
    if !ack.ok {
        let error = ack.error.unwrap_or(JsonRpcError {
            code: -32603,
            message: "server reported failure without an error object".to_owned(),
        });
        return Err(ServerError { code: error.code, message: error.message }.into());
    }

    let mut event_line = String::new();
    loop {
        event_line.clear();
        let n = reader
            .read_line(&mut event_line)
            .with_context(|| format!("failed to read event from {}", path.display()))?;
        if n == 0 {
            break; // clean disconnect
        }
        let trimmed = event_line.trim_end();
        if !trimmed.is_empty() {
            writeln!(out, "{trimmed}").context("failed to write event line")?;
        }
    }
    Ok(())
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

/// Stub for non-Unix targets.
///
/// # Errors
///
/// Always returns an error; the Unix-socket transport is not available.
#[cfg(not(unix))]
pub fn stream_events(_path: &Path) -> anyhow::Result<()> {
    anyhow::bail!("the rmux socket API is not supported on this platform yet")
}

/// Stub for non-Unix targets.
///
/// # Errors
///
/// Always returns an error; the Unix-socket transport is not available.
#[cfg(not(unix))]
pub fn stream_events_to(_path: &Path, _out: &mut dyn std::io::Write) -> anyhow::Result<()> {
    anyhow::bail!("the rmux socket API is not supported on this platform yet")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_takes_precedence() {
        let path = effective_socket_path(Some(PathBuf::from("/tmp/flag.sock")));
        assert_eq!(path, PathBuf::from("/tmp/flag.sock"));
    }

    #[test]
    fn no_flag_delegates_to_rmux_api_resolution() {
        // Env-var and build-profile precedence is covered by rmux-api's
        // own tests; here we only assert the CLI delegates to it.
        assert_eq!(effective_socket_path(None), rmux_api::default_socket_path());
    }
}
