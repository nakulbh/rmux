//! End-to-end command dispatch against a fake socket server.
//!
//! Exercises hierarchical domains, Phase 3 aliases, and the `call` escape
//! hatch: each test asserts the wire method/params the CLI produces.
#![cfg(unix)]

mod common;

use rmux_api::methods;
use rmux_cli::commands::{
    self, AppCommand, BrowserCommand, CallCommand, Command, EventsCommand, NotificationCommand,
    SidebarCommand, SurfaceCommand, SystemCommand, WorkspaceCommand, aliases, sidebar, surface,
};
use rmux_cli::output::OutputOpts;
use serde_json::json;

use common::{ok_response, spawn_event_stream_server, spawn_one_shot_server, temp_socket_path};

fn run_ok(command: Command, path: &std::path::Path) {
    commands::run(command, path, OutputOpts::default()).expect("command succeeds");
}

fn run_json(command: Command, path: &std::path::Path) {
    commands::run(command, path, OutputOpts { json: true }).expect("command succeeds");
}

// --- system -----------------------------------------------------------------

#[test]
fn system_ping_sends_system_ping() {
    let path = temp_socket_path("sys-ping");
    let server = spawn_one_shot_server(&path, ok_response(json!({ "pong": true })));
    run_ok(Command::System(SystemCommand::Ping), &path);
    let req = server.join().unwrap();
    assert_eq!(req.method, methods::SYSTEM_PING);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn system_capabilities_sends_capabilities() {
    let path = temp_socket_path("sys-caps");
    let server = spawn_one_shot_server(
        &path,
        ok_response(json!({ "version": "0.1.0", "methods": ["system.ping"] })),
    );
    run_ok(Command::System(SystemCommand::Capabilities), &path);
    assert_eq!(server.join().unwrap().method, methods::SYSTEM_CAPABILITIES);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn system_identify_sends_identify() {
    let path = temp_socket_path("sys-id");
    let server = spawn_one_shot_server(
        &path,
        ok_response(json!({ "app": "rmux", "version": "0.1.0", "pid": 1 })),
    );
    run_ok(Command::System(SystemCommand::Identify), &path);
    assert_eq!(server.join().unwrap().method, methods::SYSTEM_IDENTIFY);
    let _ = std::fs::remove_file(&path);
}

// --- workspace --------------------------------------------------------------

#[test]
fn workspace_list_create_select_close_rename() {
    let cases: Vec<(Command, &str, serde_json::Value)> = vec![
        (Command::Workspace(WorkspaceCommand::List), methods::WORKSPACE_LIST, json!({})),
        (
            Command::Workspace(WorkspaceCommand::Create { name: Some("dev".into()) }),
            methods::WORKSPACE_CREATE,
            json!({ "name": "dev" }),
        ),
        (
            Command::Workspace(WorkspaceCommand::Select { index: 2 }),
            methods::WORKSPACE_SELECT,
            json!({ "index": 2 }),
        ),
        (
            Command::Workspace(WorkspaceCommand::Close { id: 9 }),
            methods::WORKSPACE_CLOSE,
            json!({ "id": 9 }),
        ),
        (
            Command::Workspace(WorkspaceCommand::Rename { id: 1, name: "main".into() }),
            methods::WORKSPACE_RENAME,
            json!({ "id": 1, "name": "main" }),
        ),
    ];

    for (i, (command, method, params)) in cases.into_iter().enumerate() {
        let path = temp_socket_path(&format!("ws-{i}"));
        let result = match method {
            m if m == methods::WORKSPACE_CREATE => json!({ "id": 1 }),
            m if m == methods::WORKSPACE_LIST => {
                json!({ "workspaces": [{ "id": 1, "name": "main", "pane_count": 1, "active": true }] })
            }
            _ => json!({}),
        };
        let server = spawn_one_shot_server(&path, ok_response(result));
        run_ok(command, &path);
        let req = server.join().unwrap();
        assert_eq!(req.method, method);
        assert_eq!(req.params, params);
        let _ = std::fs::remove_file(&path);
    }
}

// --- surface ----------------------------------------------------------------

#[test]
fn surface_commands_map_to_wire_methods() {
    let cases: Vec<(Command, &str, serde_json::Value)> = vec![
        (Command::Surface(SurfaceCommand::List), methods::SURFACE_LIST, json!({})),
        (
            Command::Surface(SurfaceCommand::Split { direction: surface::SplitDirection::Right }),
            methods::SURFACE_SPLIT,
            json!({ "direction": "right" }),
        ),
        (
            Command::Surface(SurfaceCommand::Split { direction: surface::SplitDirection::Down }),
            methods::SURFACE_SPLIT,
            json!({ "direction": "down" }),
        ),
        (
            Command::Surface(SurfaceCommand::Focus { pane_id: 7 }),
            methods::SURFACE_FOCUS,
            json!({ "pane_id": 7 }),
        ),
        (
            Command::Surface(SurfaceCommand::Close { pane_id: Some(3) }),
            methods::SURFACE_CLOSE,
            json!({ "pane_id": 3 }),
        ),
        (
            Command::Surface(SurfaceCommand::Close { pane_id: None }),
            methods::SURFACE_CLOSE,
            json!({ "pane_id": null }),
        ),
        (
            Command::Surface(SurfaceCommand::New { title: Some("shell".into()) }),
            methods::SURFACE_NEW,
            json!({ "title": "shell" }),
        ),
        (
            Command::Surface(SurfaceCommand::Send { text: "ls\\n".into() }),
            methods::SURFACE_SEND_TEXT,
            json!({ "text": "ls\n" }),
        ),
        (
            Command::Surface(SurfaceCommand::Key { key: "enter".into() }),
            methods::SURFACE_SEND_KEY,
            json!({ "key": "enter" }),
        ),
    ];

    for (i, (command, method, params)) in cases.into_iter().enumerate() {
        let path = temp_socket_path(&format!("surf-{i}"));
        let result = if method == methods::SURFACE_SPLIT || method == methods::SURFACE_NEW {
            json!({ "pane_id": 42 })
        } else if method == methods::SURFACE_LIST {
            json!({ "surfaces": [] })
        } else {
            json!({})
        };
        let server = spawn_one_shot_server(&path, ok_response(result));
        run_ok(command, &path);
        let req = server.join().unwrap();
        assert_eq!(req.method, method, "case {i}");
        assert_eq!(req.params, params, "case {i}");
        let _ = std::fs::remove_file(&path);
    }
}

// --- notification / sidebar -------------------------------------------------

#[test]
fn notification_and_sidebar_commands() {
    let cases: Vec<(Command, &str, serde_json::Value)> = vec![
        (
            Command::Notification(NotificationCommand::Create {
                title: "Build".into(),
                subtitle: Some("rmux".into()),
                body: Some("done".into()),
            }),
            methods::NOTIFICATION_CREATE,
            json!({ "title": "Build", "subtitle": "rmux", "body": "done" }),
        ),
        (Command::Notification(NotificationCommand::List), methods::NOTIFICATION_LIST, json!({})),
        (Command::Notification(NotificationCommand::Clear), methods::NOTIFICATION_CLEAR, json!({})),
        (
            Command::Sidebar(SidebarCommand::Status(sidebar::StatusCommand::Set {
                status: "ok".into(),
                workspace: Some(1),
            })),
            methods::SIDEBAR_SET_STATUS,
            json!({ "workspace_id": 1, "status": "ok" }),
        ),
        (
            Command::Sidebar(SidebarCommand::Status(sidebar::StatusCommand::Clear {
                workspace: None,
            })),
            methods::SIDEBAR_CLEAR_STATUS,
            json!({ "workspace_id": null }),
        ),
        (
            Command::Sidebar(SidebarCommand::Progress { value: 0.75 }),
            methods::SIDEBAR_SET_PROGRESS,
            json!({ "value": 0.75 }),
        ),
    ];

    for (i, (command, method, params)) in cases.into_iter().enumerate() {
        let path = temp_socket_path(&format!("ns-{i}"));
        let result = if method == methods::NOTIFICATION_CREATE {
            json!({ "id": 5 })
        } else if method == methods::NOTIFICATION_LIST {
            json!({ "notifications": [] })
        } else {
            json!({})
        };
        let server = spawn_one_shot_server(&path, ok_response(result));
        run_ok(command, &path);
        let req = server.join().unwrap();
        assert_eq!(req.method, method);
        assert_eq!(req.params, params);
        let _ = std::fs::remove_file(&path);
    }
}

// --- browser / app ----------------------------------------------------------

#[test]
fn browser_and_app_commands() {
    let cases: Vec<(Command, &str, serde_json::Value)> = vec![
        (
            Command::Browser(BrowserCommand::Open { url: Some("https://example.com".into()) }),
            methods::BROWSER_OPEN,
            json!({ "url": "https://example.com" }),
        ),
        (
            Command::Browser(BrowserCommand::Navigate { url: "https://x.ai".into() }),
            methods::BROWSER_NAVIGATE,
            json!({ "url": "https://x.ai" }),
        ),
        (Command::Browser(BrowserCommand::Back), methods::BROWSER_BACK, json!({})),
        (Command::Browser(BrowserCommand::Forward), methods::BROWSER_FORWARD, json!({})),
        (Command::Browser(BrowserCommand::Reload), methods::BROWSER_RELOAD, json!({})),
        (Command::Browser(BrowserCommand::Url), methods::BROWSER_URL, json!({})),
        (
            Command::App(AppCommand::FontSize { delta: Some(1.0), reset: false }),
            methods::APP_SET_FONT_SIZE,
            json!({ "delta": 1.0, "reset": false }),
        ),
        (
            Command::App(AppCommand::FontSize { delta: None, reset: true }),
            methods::APP_SET_FONT_SIZE,
            json!({ "delta": null, "reset": true }),
        ),
        (
            Command::App(AppCommand::Theme { name: "dracula".into() }),
            methods::APP_SET_THEME,
            json!({ "theme": "dracula" }),
        ),
    ];

    for (i, (command, method, params)) in cases.into_iter().enumerate() {
        let path = temp_socket_path(&format!("ba-{i}"));
        let result = match method {
            m if m == methods::BROWSER_OPEN => json!({ "pane_id": 99 }),
            m if m == methods::BROWSER_URL => json!({ "url": "https://example.com" }),
            m if m == methods::APP_SET_FONT_SIZE => json!({ "font_size": 15.0 }),
            _ => json!({}),
        };
        let server = spawn_one_shot_server(&path, ok_response(result));
        run_ok(command, &path);
        let req = server.join().unwrap();
        assert_eq!(req.method, method, "case {i}");
        assert_eq!(req.params, params, "case {i}");
        let _ = std::fs::remove_file(&path);
    }
}

// --- call / events / aliases ------------------------------------------------

#[test]
fn call_escape_hatch_sends_raw_method_and_params() {
    let path = temp_socket_path("call");
    let server = spawn_one_shot_server(&path, ok_response(json!({ "id": 3 })));
    run_ok(
        Command::Call(CallCommand {
            method: "workspace.create".into(),
            params: r#"{"name":"tmp"}"#.into(),
        }),
        &path,
    );
    let req = server.join().unwrap();
    assert_eq!(req.method, "workspace.create");
    assert_eq!(req.params, json!({ "name": "tmp" }));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn call_rejects_invalid_json_params() {
    let path = temp_socket_path("call-bad");
    let err = commands::run(
        Command::Call(CallCommand { method: "system.ping".into(), params: "not-json".into() }),
        &path,
        OutputOpts::default(),
    )
    .expect_err("invalid params");
    assert!(
        err.to_string().contains("params must be a valid JSON value")
            || format!("{err:#}").contains("JSON")
    );
    // No server needed — fails before connect when params parse fails.
    // Actually it parses params before connect, so no connect error.
    let _ = std::fs::remove_file(&path);
}

#[test]
fn events_stream_command_subscribes() {
    let path = temp_socket_path("events-cmd");
    let server = spawn_event_stream_server(
        &path,
        vec![json!({ "event": "notification", "data": { "id": 1 } })],
    );
    run_ok(Command::Events(EventsCommand::Stream), &path);
    let req = server.join().unwrap();
    assert_eq!(req.method, methods::EVENTS_STREAM);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn phase3_aliases_forward_to_same_methods() {
    let cases: Vec<(Command, &str)> = vec![
        (Command::Alias(aliases::AliasCommand::Ping), methods::SYSTEM_PING),
        (Command::Alias(aliases::AliasCommand::Capabilities), methods::SYSTEM_CAPABILITIES),
        (
            Command::Alias(aliases::AliasCommand::Notify {
                title: "t".into(),
                subtitle: None,
                body: None,
            }),
            methods::NOTIFICATION_CREATE,
        ),
        (
            Command::Alias(aliases::AliasCommand::NewWorkspace { name: Some("x".into()) }),
            methods::WORKSPACE_CREATE,
        ),
        (
            Command::Alias(aliases::AliasCommand::ListWorkspaces { json: true }),
            methods::WORKSPACE_LIST,
        ),
        (
            Command::Alias(aliases::AliasCommand::NewSplit {
                direction: aliases::AliasSplitDirection::Right,
            }),
            methods::SURFACE_SPLIT,
        ),
        (
            Command::Alias(aliases::AliasCommand::Send { text: "hi".into() }),
            methods::SURFACE_SEND_TEXT,
        ),
    ];

    for (i, (command, method)) in cases.into_iter().enumerate() {
        let path = temp_socket_path(&format!("alias-{i}"));
        let result = match method {
            m if m == methods::NOTIFICATION_CREATE || m == methods::WORKSPACE_CREATE => {
                json!({ "id": 1 })
            }
            m if m == methods::WORKSPACE_LIST => json!({ "workspaces": [] }),
            m if m == methods::SURFACE_SPLIT => json!({ "pane_id": 2 }),
            m if m == methods::SYSTEM_PING => json!({ "pong": true }),
            m if m == methods::SYSTEM_CAPABILITIES => {
                json!({ "version": "0", "methods": [] })
            }
            _ => json!({}),
        };
        let server = spawn_one_shot_server(&path, ok_response(result));
        run_ok(command, &path);
        assert_eq!(server.join().unwrap().method, method);
        let _ = std::fs::remove_file(&path);
    }
}

#[test]
fn json_output_mode_still_succeeds_for_list() {
    let path = temp_socket_path("json-list");
    let server = spawn_one_shot_server(
        &path,
        ok_response(json!({
            "workspaces": [{ "id": 1, "name": "main", "pane_count": 2, "active": true }]
        })),
    );
    run_json(Command::Workspace(WorkspaceCommand::List), &path);
    assert_eq!(server.join().unwrap().method, methods::WORKSPACE_LIST);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn command_surfaces_server_error() {
    use rmux_api::protocol::JsonRpcError;
    use rmux_api::protocol::JsonRpcResponse;

    let path = temp_socket_path("server-err");
    let response = JsonRpcResponse {
        id: json!(1),
        ok: false,
        result: None,
        error: Some(JsonRpcError {
            code: -32602,
            message: "workspace index out of range: 99".into(),
        }),
    };
    let server = spawn_one_shot_server(&path, response);
    let err = commands::run(
        Command::Workspace(WorkspaceCommand::Select { index: 99 }),
        &path,
        OutputOpts::default(),
    )
    .expect_err("should fail");
    let server_err = err.downcast_ref::<rmux_cli::socket::ServerError>().expect("ServerError");
    assert_eq!(server_err.code, -32602);
    let _ = server.join();
    let _ = std::fs::remove_file(&path);
}

#[test]
fn command_surfaces_connect_error() {
    let path = temp_socket_path("no-server");
    let err = commands::run(Command::System(SystemCommand::Ping), &path, OutputOpts::default())
        .expect_err("connect fails");
    assert!(err.downcast_ref::<rmux_cli::socket::ConnectError>().is_some());
}
