//! Socket API server bootstrap.
//!
//! Spawns a dedicated background thread running a Tokio runtime that
//! hosts the `rmux-api` Unix socket server. Parsed requests flow to the
//! egui main thread over an `mpsc` channel (drained each frame in
//! `RmuxApp::update`); application events flow back to `events.stream`
//! subscribers over a `broadcast` channel.

use rmux_api::{ApiEvent, ApiRequestEnvelope, ApiServer};
use tokio::sync::{broadcast, mpsc};

/// Capacity of the request channel from the server to the app.
const REQUEST_CHANNEL_CAPACITY: usize = 64;

/// Capacity of the event broadcast channel from the app to the server.
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// Channel endpoints the application keeps after starting the server.
pub struct ApiChannels {
    /// Receives parsed API requests; drained each frame in `update()`.
    pub request_rx: mpsc::Receiver<ApiRequestEnvelope>,
    /// Publishes application events to `events.stream` subscribers.
    pub event_tx: broadcast::Sender<ApiEvent>,
}

/// Start the socket API server on a background thread.
///
/// Failures (thread spawn, runtime construction, socket bind) are logged
/// and swallowed: the app keeps running without the socket API rather
/// than failing to start.
pub fn start_server() -> ApiChannels {
    let (request_tx, request_rx) = mpsc::channel(REQUEST_CHANNEL_CAPACITY);
    let (event_tx, _event_rx) = broadcast::channel(EVENT_CHANNEL_CAPACITY);

    let server_event_tx = event_tx.clone();
    let spawned = std::thread::Builder::new()
        .name("rmux-api-server".to_owned())
        .spawn(move || run_server(request_tx, server_event_tx));
    if let Err(err) = spawned {
        tracing::error!(error = %err, "failed to spawn API server thread; socket API disabled");
    }

    ApiChannels { request_rx, event_tx }
}

/// Blocking body of the server thread: bind the socket and serve forever.
fn run_server(request_tx: mpsc::Sender<ApiRequestEnvelope>, event_tx: broadcast::Sender<ApiEvent>) {
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => {
            tracing::error!(error = %err, "failed to build Tokio runtime; socket API disabled");
            return;
        }
    };

    runtime.block_on(async move {
        let socket_path = rmux_api::default_socket_path();
        let _server = match ApiServer::bind(socket_path.clone(), request_tx, event_tx).await {
            Ok(server) => server,
            Err(err) => {
                tracing::error!(
                    path = %socket_path.display(),
                    error = %err,
                    "failed to bind API socket; running without socket API"
                );
                return;
            }
        };
        tracing::info!(path = %socket_path.display(), "socket API available");
        // Keep the runtime (and the accept loop it drives) alive for the
        // lifetime of the process; the socket file is cleaned up by the
        // OS-level unlink on the next bind.
        std::future::pending::<()>().await;
    });
}
