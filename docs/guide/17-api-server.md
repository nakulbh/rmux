# 17. API server

rmux has socket API. CLI sends JSON-RPC. App answers on main thread.

Files: `rmux-api/src/server.rs`, `rmux-cli/src/commands.rs`, `rmux-app/src/app.rs`.

## Server job

Top comment:

```rust
//! Unix socket server module.
//!
//! Listens on a Unix domain socket, accepts client connections, and
//! speaks the newline-delimited JSON protocol from [`crate::protocol`].
//! Requests are forwarded to the application over an `mpsc` channel as
//! [`ApiRequestEnvelope`]s; the special `events.stream` method switches
//! a connection into streaming mode fed by a `broadcast` channel of
//! [`ApiEvent`]s.
```

Map:

```text
rmux-cli -> Unix socket -> rmux-api -> mpsc request -> RmuxApp -> response -> CLI
```

## Socket path

Default path comes from env var or `/tmp`.

```rust
pub fn default_socket_path() -> PathBuf {
    socket_path_from_override(std::env::var_os("RMUX_SOCKET_PATH"))
}

pub(crate) fn socket_path_from_override(override_path: Option<OsString>) -> PathBuf {
    match override_path {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ if cfg!(debug_assertions) => PathBuf::from("/tmp/rmux-debug.sock"),
        _ => PathBuf::from("/tmp/rmux.sock"),
    }
}
```

Debug and release use different default sockets. Safer during dev.

## ApiServer struct

```rust
pub struct ApiServer {
    socket_path: PathBuf,
    shutdown_tx: Option<oneshot::Sender<()>>,
    accept_task: Option<tokio::task::JoinHandle<()>>,
}
```

Server owns socket path, shutdown sender, accept task.

## Bind

`bind()` uses default timeout.

```rust
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

pub async fn bind(
    socket_path: impl Into<PathBuf>,
    request_tx: mpsc::Sender<ApiRequestEnvelope>,
    event_tx: broadcast::Sender<ApiEvent>,
) -> Result<Self, ApiError> {
    Self::bind_with_timeout(socket_path, request_tx, event_tx, DEFAULT_REQUEST_TIMEOUT).await
}
```

Unix bind removes stale socket, then starts accept loop.

```rust
let socket_path = socket_path.into();
remove_stale_socket(&socket_path);
let listener = tokio::net::UnixListener::bind(&socket_path)
    .map_err(|source| ApiError::Bind { path: socket_path.clone(), source })?;
tracing::info!(path = %socket_path.display(), "API server listening");

let (shutdown_tx, shutdown_rx) = oneshot::channel();
let accept_task =
    tokio::spawn(accept_loop(listener, shutdown_rx, request_tx, event_tx, request_timeout));
```

Non-Unix currently unsupported:

```rust
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
```

## App drains API requests

Server task can't mutate UI state directly. It sends request envelope to app.

```rust
fn process_api_requests(&mut self) {
    while let Ok(envelope) = self.api_request_rx.try_recv() {
        tracing::debug!(method = %envelope.method, "handling API request");
        let result = crate::api_dispatch::dispatch(self, &envelope.method, envelope.params);
        let _ = envelope.respond.send(result);
    }
}
```

Main thread owns workspace and UI state.

## CLI commands

Each CLI function builds method and params, calls socket, prints result.

Ping:

```rust
pub fn ping(socket_path: &Path) -> Result<()> {
    let (method, params) = ping_request();
    socket::call(socket_path, method, params)?;
    println!("pong");
    Ok(())
}
```

Notify:

```rust
pub fn notify(
    socket_path: &Path,
    title: &str,
    subtitle: Option<&str>,
    body: Option<&str>,
) -> Result<()> {
    let (method, params) = notify_request(title, subtitle, body);
    let result = socket::call(socket_path, method, params)?;
    println!("{}", extract_id(&result));
    Ok(())
}
```

Request builders are tiny and testable:

```rust
fn ping_request() -> (&'static str, Value) {
    ("system.ping", json!({}))
}

fn notify_request(
    title: &str,
    subtitle: Option<&str>,
    body: Option<&str>,
) -> (&'static str, Value) {
    ("notification.create", json!({ "title": title, "subtitle": subtitle, "body": body }))
}
```

Workspace and pane commands:

```rust
fn new_workspace_request(name: Option<&str>) -> (&'static str, Value) {
    ("workspace.create", json!({ "name": name }))
}

fn new_split_request(direction: &str) -> (&'static str, Value) {
    ("surface.split", json!({ "direction": direction }))
}

fn send_request(text: &str) -> (&'static str, Value) {
    ("surface.send_text", json!({ "text": interpret_escapes(text) }))
}
```

← **Prev: [16 — Notifications](16-notifications.md)**

→ **Next: [18 — Config](18-config.md)**
