//! Application-side dispatcher for socket API requests.
//!
//! Runs on the egui main thread (called from `RmuxApp::update`), so it
//! has direct `&mut` access to the workspace and notification managers.
//! Method names and parameter shapes come from [`rmux_api::methods`].

use rmux_api::JsonRpcError;
use rmux_api::methods::{
    self, NotificationCreateParams, SidebarClearStatusParams, SidebarSetProgressParams,
    SidebarSetStatusParams, SurfaceFocusParams, SurfaceSendKeyParams, SurfaceSendTextParams,
    SurfaceSplitParams, WorkspaceCloseParams, WorkspaceCreateParams, WorkspaceSelectParams,
};
use serde_json::{Value, json};

use crate::app::RmuxApp;
use crate::workspace::model::{Workspace, WorkspaceId};
use crate::workspace::splits::{PaneId, SplitDirection};

/// JSON-RPC 2.0 error code for invalid method parameters.
const INVALID_PARAMS: i32 = -32602;

/// Handle one API request against the application state.
///
/// Returns the JSON result on success, or a [`JsonRpcError`] (including
/// code `-32601` for unknown methods) that the server relays verbatim.
pub fn dispatch(app: &mut RmuxApp, method: &str, params: Value) -> Result<Value, JsonRpcError> {
    match method {
        methods::SYSTEM_PING => Ok(json!({ "pong": true })),
        methods::SYSTEM_CAPABILITIES => Ok(json!({
            "version": env!("CARGO_PKG_VERSION"),
            "methods": methods::all_methods(),
        })),
        methods::SYSTEM_IDENTIFY => Ok(json!({
            "app": "rmux",
            "version": env!("CARGO_PKG_VERSION"),
            "pid": std::process::id(),
        })),
        methods::WORKSPACE_LIST => Ok(workspace_list(app)),
        methods::WORKSPACE_CREATE => workspace_create(app, params),
        methods::WORKSPACE_SELECT => workspace_select(app, params),
        methods::WORKSPACE_CLOSE => workspace_close(app, params),
        methods::SURFACE_LIST => Ok(surface_list(app)),
        methods::SURFACE_SPLIT => surface_split(app, params),
        methods::SURFACE_FOCUS => surface_focus(app, params),
        methods::SURFACE_SEND_TEXT => surface_send_text(app, params),
        methods::SURFACE_SEND_KEY => surface_send_key(app, params),
        methods::NOTIFICATION_CREATE => notification_create(app, params),
        methods::NOTIFICATION_LIST => Ok(notification_list(app)),
        methods::NOTIFICATION_CLEAR => {
            app.notifications.clear();
            Ok(json!({}))
        }
        methods::SIDEBAR_SET_STATUS => sidebar_set_status(app, params),
        methods::SIDEBAR_CLEAR_STATUS => sidebar_clear_status(app, params),
        methods::SIDEBAR_SET_PROGRESS => sidebar_set_progress(app, params),
        other => Err(JsonRpcError::method_not_found(other)),
    }
}

/// Deserialize `params` into `T`, mapping failures to `INVALID_PARAMS`.
fn parse_params<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, JsonRpcError> {
    serde_json::from_value(params)
        .map_err(|err| JsonRpcError::new(INVALID_PARAMS, format!("invalid params: {err}")))
}

/// `workspace.list` — id/name/pane_count/active for every workspace.
fn workspace_list(app: &RmuxApp) -> Value {
    let active_index = app.workspace_manager.active_index();
    let workspaces: Vec<Value> = app
        .workspace_manager
        .workspaces()
        .iter()
        .enumerate()
        .map(|(i, ws)| {
            json!({
                "id": ws.id.0,
                "name": ws.name,
                "pane_count": ws.pane_count(),
                "active": i == active_index,
            })
        })
        .collect();
    json!({ "workspaces": workspaces })
}

/// `workspace.create` — create a workspace with a live terminal pane.
fn workspace_create(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: WorkspaceCreateParams = parse_params(params)?;
    let name = params
        .name
        .unwrap_or_else(|| format!("Workspace {}", app.workspace_manager.workspace_count() + 1));
    let id = app.create_workspace_with_terminal(name);
    Ok(json!({ "id": id }))
}

/// `workspace.select` — switch to a workspace by zero-based index.
fn workspace_select(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: WorkspaceSelectParams = parse_params(params)?;
    if params.index >= app.workspace_manager.workspace_count() {
        return Err(JsonRpcError::new(
            INVALID_PARAMS,
            format!("workspace index out of range: {}", params.index),
        ));
    }
    app.workspace_manager.switch_to(params.index);
    Ok(json!({}))
}

/// `workspace.close` — close a workspace by id.
fn workspace_close(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: WorkspaceCloseParams = parse_params(params)?;
    app.workspace_manager
        .close_workspace(WorkspaceId(params.id))
        .map_err(|err| JsonRpcError::internal(err.to_string()))?;
    app.publish_event("workspace.closed", json!({ "id": params.id }));
    Ok(json!({}))
}

/// `surface.list` — all panes across all workspaces.
fn surface_list(app: &RmuxApp) -> Value {
    let active_index = app.workspace_manager.active_index();
    let mut surfaces = Vec::new();
    for (i, ws) in app.workspace_manager.workspaces().iter().enumerate() {
        for pane in ws.pane_ids() {
            surfaces.push(json!({
                "pane_id": pane.0,
                "workspace_id": ws.id.0,
                "active": i == active_index && pane == ws.active_pane,
            }));
        }
    }
    json!({ "surfaces": surfaces })
}

/// `surface.split` — split the active pane and spawn a terminal in it.
fn surface_split(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: SurfaceSplitParams = parse_params(params)?;
    let direction = match params.direction {
        methods::SplitDirection::Right => SplitDirection::Horizontal,
        methods::SplitDirection::Down => SplitDirection::Vertical,
    };
    let pane_id = app
        .split_active_with_terminal(direction)
        .map_err(|err| JsonRpcError::internal(err.to_string()))?;
    Ok(json!({ "pane_id": pane_id }))
}

/// `surface.focus` — focus a pane anywhere, switching workspaces if needed.
fn surface_focus(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: SurfaceFocusParams = parse_params(params)?;
    if app.workspace_manager.focus_pane_global(PaneId(params.pane_id)) {
        Ok(json!({}))
    } else {
        Err(JsonRpcError::new(INVALID_PARAMS, format!("pane not found: {}", params.pane_id)))
    }
}

/// `surface.send_text` — type raw text into the active pane's PTY.
fn surface_send_text(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: SurfaceSendTextParams = parse_params(params)?;
    send_to_active_terminal(app, &params.text)
}

/// `surface.send_key` — send a named key to the active pane's PTY.
fn surface_send_key(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: SurfaceSendKeyParams = parse_params(params)?;
    let text = match params.key.as_str() {
        "enter" => "\r",
        "tab" => "\t",
        "escape" => "\u{1b}",
        "ctrl+c" => "\u{3}",
        "ctrl+d" => "\u{4}",
        other => {
            return Err(JsonRpcError::new(INVALID_PARAMS, format!("unsupported key: {other}")));
        }
    };
    send_to_active_terminal(app, text)
}

/// Write text to the active pane's terminal, erroring if there is none.
fn send_to_active_terminal(app: &mut RmuxApp, text: &str) -> Result<Value, JsonRpcError> {
    match app.workspace_manager.active_mut().active_terminal() {
        Some(terminal) => {
            terminal.send_text(text);
            Ok(json!({}))
        }
        None => Err(JsonRpcError::internal("active pane has no terminal")),
    }
}

/// `notification.create` — store an external notification.
///
/// A subtitle, if given, is folded into the body (the notification model
/// only carries title + body).
fn notification_create(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: NotificationCreateParams = parse_params(params)?;
    let body = match (params.subtitle, params.body) {
        (Some(subtitle), Some(body)) => Some(format!("{subtitle}\n{body}")),
        (Some(subtitle), None) => Some(subtitle),
        (None, body) => body,
    };
    let id = app.notifications.add(params.title.clone(), body.clone(), None, None);
    app.publish_event(
        "notification",
        json!({ "id": id, "title": params.title, "body": body, "pane_id": null, "workspace_id": null }),
    );
    Ok(json!({ "id": id }))
}

/// `notification.list` — all stored notifications, oldest first.
fn notification_list(app: &RmuxApp) -> Value {
    let notifications: Vec<Value> = app
        .notifications
        .list()
        .iter()
        .map(|n| {
            let timestamp =
                n.timestamp.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            json!({
                "id": n.id,
                "title": n.title,
                "body": n.body,
                "read": n.read,
                "pane_id": n.pane_id,
                "workspace_id": n.workspace_id,
                "timestamp": timestamp,
            })
        })
        .collect();
    json!({ "notifications": notifications })
}

/// `sidebar.set_status` — set the status text on a workspace tab.
fn sidebar_set_status(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: SidebarSetStatusParams = parse_params(params)?;
    let workspace = resolve_workspace_mut(app, params.workspace_id)?;
    workspace.status = Some(params.status);
    Ok(json!({}))
}

/// `sidebar.clear_status` — clear the status text (and progress bar).
fn sidebar_clear_status(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: SidebarClearStatusParams = parse_params(params)?;
    let workspace = resolve_workspace_mut(app, params.workspace_id)?;
    workspace.status = None;
    workspace.progress = None;
    Ok(json!({}))
}

/// `sidebar.set_progress` — set the active workspace's progress bar.
fn sidebar_set_progress(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: SidebarSetProgressParams = parse_params(params)?;
    let safe = if params.value.is_finite() { params.value.clamp(0.0, 1.0) } else { 0.0 };
    app.workspace_manager.active_mut().progress = Some(safe);
    Ok(json!({}))
}

/// Resolve an optional workspace id to a workspace (`None` = active).
fn resolve_workspace_mut(
    app: &mut RmuxApp,
    workspace_id: Option<u64>,
) -> Result<&mut Workspace, JsonRpcError> {
    match workspace_id {
        None => Ok(app.workspace_manager.active_mut()),
        Some(id) => app
            .workspace_manager
            .workspace_mut(WorkspaceId(id))
            .ok_or_else(|| JsonRpcError::new(INVALID_PARAMS, format!("workspace not found: {id}"))),
    }
}
