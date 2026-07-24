//! In-crate integration tests for the socket API server.

use crate::methods;
use crate::server::socket_path_from_override;

#[test]
fn test_socket_path_prefers_env_override() {
    let path = socket_path_from_override(Some("/tmp/custom-rmux.sock".into()));
    assert_eq!(path, std::path::PathBuf::from("/tmp/custom-rmux.sock"));
}

#[test]
fn test_socket_path_falls_back_when_override_absent_or_empty() {
    let expected = if cfg!(debug_assertions) { "/tmp/rmux-debug.sock" } else { "/tmp/rmux.sock" };
    assert_eq!(socket_path_from_override(None), std::path::PathBuf::from(expected));
    assert_eq!(socket_path_from_override(Some("".into())), std::path::PathBuf::from(expected));
}

#[test]
fn test_all_methods_lists_full_contract() {
    let all = methods::all_methods();
    assert_eq!(all.len(), 30);
    assert!(all.contains(&methods::SYSTEM_PING));
    assert!(all.contains(&methods::EVENTS_STREAM));
    assert!(all.contains(&methods::SIDEBAR_SET_PROGRESS));
    assert!(all.contains(&methods::WORKSPACE_RENAME));
    assert!(all.contains(&methods::SURFACE_CLOSE));
    assert!(all.contains(&methods::SURFACE_NEW));
    assert!(all.contains(&methods::BROWSER_OPEN));
    assert!(all.contains(&methods::APP_SET_THEME));
}

#[cfg(unix)]
mod unix {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use serde_json::{Value, json};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;
    use tokio::sync::{broadcast, mpsc};

    use crate::{ApiEvent, ApiRequestEnvelope, ApiServer, JsonRpcError};

    /// Unique-per-test socket path in the system temp directory.
    fn test_socket_path() -> std::path::PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("rmux-api-test-{}-{n}.sock", std::process::id()))
    }

    /// Spawn a fake application handler that answers `system.ping`,
    /// drops the responder for `test.drop`, and reports method-not-found
    /// for everything else.
    fn spawn_fake_handler(mut request_rx: mpsc::Receiver<ApiRequestEnvelope>) {
        tokio::spawn(async move {
            while let Some(envelope) = request_rx.recv().await {
                match envelope.method.as_str() {
                    "system.ping" => {
                        let _ = envelope.respond.send(Ok(json!({"pong": true})));
                    }
                    "test.drop" => drop(envelope.respond),
                    other => {
                        let _ = envelope.respond.send(Err(JsonRpcError::method_not_found(other)));
                    }
                }
            }
        });
    }

    /// Bind a test server with a fake handler; returns the server, its
    /// socket path, and the event broadcast sender.
    async fn start_test_server(
        timeout: Duration,
    ) -> (ApiServer, std::path::PathBuf, broadcast::Sender<ApiEvent>) {
        let path = test_socket_path();
        let (request_tx, request_rx) = mpsc::channel(16);
        let (event_tx, _) = broadcast::channel(16);
        spawn_fake_handler(request_rx);
        let server = ApiServer::bind_with_timeout(&path, request_tx, event_tx.clone(), timeout)
            .await
            .expect("bind test server");
        (server, path, event_tx)
    }

    /// A line-oriented test client.
    struct TestClient {
        reader: tokio::io::Lines<BufReader<tokio::net::unix::OwnedReadHalf>>,
        writer: tokio::net::unix::OwnedWriteHalf,
    }

    impl TestClient {
        async fn connect(path: &std::path::Path) -> Self {
            let stream = UnixStream::connect(path).await.expect("connect to test socket");
            let (read_half, writer) = stream.into_split();
            Self { reader: BufReader::new(read_half).lines(), writer }
        }

        async fn send_line(&mut self, line: &str) {
            self.writer.write_all(line.as_bytes()).await.unwrap();
            self.writer.write_all(b"\n").await.unwrap();
        }

        async fn read_json(&mut self) -> Value {
            let line = self.reader.next_line().await.unwrap().expect("server closed connection");
            serde_json::from_str(&line).expect("response is valid JSON")
        }

        async fn call(&mut self, id: u64, method: &str) -> Value {
            self.send_line(&json!({"id": id, "method": method}).to_string()).await;
            self.read_json().await
        }
    }

    #[tokio::test]
    async fn test_ping_round_trip() {
        let (server, path, _event_tx) = start_test_server(Duration::from_secs(5)).await;
        let mut client = TestClient::connect(&path).await;

        let response = client.call(1, "system.ping").await;
        assert_eq!(response["id"], 1);
        assert_eq!(response["ok"], true);
        assert_eq!(response["result"]["pong"], true);

        server.shutdown();
        assert!(!path.exists(), "socket file removed on shutdown");
    }

    #[tokio::test]
    async fn test_unknown_method_returns_handler_error() {
        let (_server, path, _event_tx) = start_test_server(Duration::from_secs(5)).await;
        let mut client = TestClient::connect(&path).await;

        let response = client.call(2, "no.such.method").await;
        assert_eq!(response["ok"], false);
        assert_eq!(response["error"]["code"], crate::protocol::codes::METHOD_NOT_FOUND);
        assert!(response["error"]["message"].as_str().unwrap().contains("no.such.method"));
    }

    #[tokio::test]
    async fn test_dropped_responder_reports_timeout_error() {
        let (_server, path, _event_tx) = start_test_server(Duration::from_millis(200)).await;
        let mut client = TestClient::connect(&path).await;

        let response = client.call(3, "test.drop").await;
        assert_eq!(response["ok"], false);
        assert_eq!(response["error"]["code"], crate::protocol::codes::TIMEOUT);
    }

    #[tokio::test]
    async fn test_events_stream_receives_published_events() {
        let (_server, path, event_tx) = start_test_server(Duration::from_secs(5)).await;
        let mut client = TestClient::connect(&path).await;

        let ack = client.call(4, "events.stream").await;
        assert_eq!(ack["ok"], true);
        assert_eq!(ack["result"]["streaming"], true);

        event_tx.send(ApiEvent::new("pane.created", json!({"pane_id": 1}))).unwrap();
        event_tx.send(ApiEvent::new("workspace.changed", json!({"id": 2}))).unwrap();

        let first = client.read_json().await;
        assert_eq!(first["event"], "pane.created");
        assert_eq!(first["data"]["pane_id"], 1);

        let second = client.read_json().await;
        assert_eq!(second["event"], "workspace.changed");
        assert_eq!(second["data"]["id"], 2);
    }

    #[tokio::test]
    async fn test_malformed_json_keeps_connection_usable() {
        let (_server, path, _event_tx) = start_test_server(Duration::from_secs(5)).await;
        let mut client = TestClient::connect(&path).await;

        client.send_line("{not json at all").await;
        let error_response = client.read_json().await;
        assert_eq!(error_response["ok"], false);
        assert_eq!(error_response["id"], Value::Null);
        assert_eq!(error_response["error"]["code"], crate::protocol::codes::PARSE_ERROR);

        // Same connection still serves valid requests.
        let response = client.call(5, "system.ping").await;
        assert_eq!(response["ok"], true);
        assert_eq!(response["result"]["pong"], true);
    }

    #[tokio::test]
    async fn test_multiple_concurrent_clients() {
        let (_server, path, _event_tx) = start_test_server(Duration::from_secs(5)).await;

        let mut handles = Vec::new();
        for id in 0..8u64 {
            let path = path.clone();
            handles.push(tokio::spawn(async move {
                let mut client = TestClient::connect(&path).await;
                let response = client.call(id, "system.ping").await;
                assert_eq!(response["id"], id);
                assert_eq!(response["ok"], true);
            }));
        }
        for handle in handles {
            handle.await.expect("client task succeeded");
        }
    }

    #[tokio::test]
    async fn test_bind_removes_stale_socket_file() {
        let path = test_socket_path();

        // First server creates the socket, then is leaked without
        // shutdown so the file stays behind like after a crash.
        let (request_tx, request_rx) = mpsc::channel(16);
        let (event_tx, _) = broadcast::channel::<ApiEvent>(16);
        spawn_fake_handler(request_rx);
        let stale = ApiServer::bind(&path, request_tx.clone(), event_tx.clone())
            .await
            .expect("bind first server");
        std::mem::forget(stale);
        assert!(path.exists());

        // Second bind on the same path must succeed.
        let server = ApiServer::bind(&path, request_tx, event_tx)
            .await
            .expect("rebinding over stale socket succeeds");
        let mut client = TestClient::connect(&path).await;
        let response = client.call(6, "system.ping").await;
        assert_eq!(response["ok"], true);
        server.shutdown();
    }
}
