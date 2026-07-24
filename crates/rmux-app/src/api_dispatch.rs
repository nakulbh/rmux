//! Application-side dispatcher for socket API requests.
//!
//! Runs on the egui main thread (called from `RmuxApp::update`), so it
//! has direct `&mut` access to the workspace and notification managers.
//! Method names and parameter shapes come from [`rmux_api::methods`].

use rmux_api::JsonRpcError;
use rmux_api::methods::{
    self, BrowserEvalParams, BrowserFillParams, BrowserNavigateParams, BrowserOpenParams,
    BrowserPaneParams, BrowserPressParams, BrowserScreenshotParams, BrowserSelectorParams,
    BrowserSnapshotParams, BrowserTypeParams, NotificationCreateParams, SidebarClearStatusParams,
    SidebarSetProgressParams, SidebarSetStatusParams, SurfaceFocusParams, SurfaceSendKeyParams,
    SurfaceSendTextParams, SurfaceSplitParams, WorkspaceCloseParams, WorkspaceCreateParams,
    WorkspaceSelectParams,
};
use serde_json::{Value, json};

use crate::app::RmuxApp;
use crate::browser::BrowserPane;
use crate::browser::automation;
use crate::workspace::model::{Workspace, WorkspaceId};
use crate::workspace::splits::{PaneId, SplitDirection};

/// JSON-RPC 2.0 error code for invalid method parameters.
const INVALID_PARAMS: i32 = -32602;

/// Outcome of dispatching one method.
///
/// Most methods finish immediately. Browser `eval` / `snapshot` (and similar)
/// schedule JS on the webview and complete later via [`PendingJsRpc`].
pub enum DispatchOutcome {
    /// Answer the client now.
    Immediate(Result<Value, JsonRpcError>),
    /// Wait for a wry JS callback, then map the string into a JSON result.
    PendingJs { result_rx: std::sync::mpsc::Receiver<String>, map: PendingJsMap },
}

/// How to interpret a page-returned JSON string for a deferred RPC.
#[derive(Debug, Clone, Copy)]
pub enum PendingJsMap {
    /// `browser.eval` — expect `{ok,value}` or `{ok:false,error}`.
    Eval,
    /// `browser.snapshot` — expect full snapshot object with `ok`.
    Snapshot,
    /// Action scripts (`click`/`fill`/…) — expect `{ok:true}` or error.
    Action,
}

/// A deferred browser JS call held until the wry callback fires.
pub struct PendingJsRpc {
    pub result_rx: std::sync::mpsc::Receiver<String>,
    pub map: PendingJsMap,
    pub respond: tokio::sync::oneshot::Sender<rmux_api::ApiResponseResult>,
}

/// Handle one API request against the application state.
pub fn dispatch(app: &mut RmuxApp, method: &str, params: Value) -> DispatchOutcome {
    match method {
        methods::SYSTEM_PING => Immediate(Ok(json!({ "pong": true }))),
        methods::SYSTEM_CAPABILITIES => Immediate(Ok(json!({
            "version": env!("CARGO_PKG_VERSION"),
            "methods": methods::all_methods(),
        }))),
        methods::SYSTEM_IDENTIFY => Immediate(Ok(json!({
            "app": "rmux",
            "version": env!("CARGO_PKG_VERSION"),
            "pid": std::process::id(),
        }))),
        methods::WORKSPACE_LIST => Immediate(Ok(workspace_list(app))),
        methods::WORKSPACE_CREATE => Immediate(workspace_create(app, params)),
        methods::WORKSPACE_SELECT => Immediate(workspace_select(app, params)),
        methods::WORKSPACE_CLOSE => Immediate(workspace_close(app, params)),
        methods::SURFACE_LIST => Immediate(Ok(surface_list(app))),
        methods::SURFACE_SPLIT => Immediate(surface_split(app, params)),
        methods::SURFACE_FOCUS => Immediate(surface_focus(app, params)),
        methods::SURFACE_SEND_TEXT => Immediate(surface_send_text(app, params)),
        methods::SURFACE_SEND_KEY => Immediate(surface_send_key(app, params)),
        methods::NOTIFICATION_CREATE => Immediate(notification_create(app, params)),
        methods::NOTIFICATION_LIST => Immediate(Ok(notification_list(app))),
        methods::NOTIFICATION_CLEAR => {
            app.notifications.clear();
            Immediate(Ok(json!({})))
        }
        methods::SIDEBAR_SET_STATUS => Immediate(sidebar_set_status(app, params)),
        methods::SIDEBAR_CLEAR_STATUS => Immediate(sidebar_clear_status(app, params)),
        methods::SIDEBAR_SET_PROGRESS => Immediate(sidebar_set_progress(app, params)),

        // Browser
        methods::BROWSER_OPEN => Immediate(browser_open(app, params)),
        methods::BROWSER_NAVIGATE => Immediate(browser_navigate(app, params)),
        methods::BROWSER_BACK => Immediate(browser_history(app, params, HistoryOp::Back)),
        methods::BROWSER_FORWARD => Immediate(browser_history(app, params, HistoryOp::Forward)),
        methods::BROWSER_RELOAD => Immediate(browser_history(app, params, HistoryOp::Reload)),
        methods::BROWSER_URL => Immediate(browser_url(app, params)),
        methods::BROWSER_EVAL => browser_eval(app, params),
        methods::BROWSER_CLICK => browser_action(app, params, ActionKind::Click),
        methods::BROWSER_TYPE => browser_action(app, params, ActionKind::Type),
        methods::BROWSER_FILL => browser_action(app, params, ActionKind::Fill),
        methods::BROWSER_PRESS => browser_action(app, params, ActionKind::Press),
        methods::BROWSER_SNAPSHOT => browser_snapshot(app, params),
        methods::BROWSER_SCREENSHOT => Immediate(browser_screenshot(app, params)),

        other => Immediate(Err(JsonRpcError::method_not_found(other))),
    }
}

use DispatchOutcome::{Immediate, PendingJs};

/// Map a page JSON string into the final RPC result for a pending call.
pub fn map_pending_js(map: PendingJsMap, raw: &str) -> Result<Value, JsonRpcError> {
    let value = automation::parse_page_json(raw)
        .map_err(|e| JsonRpcError::internal(format!("browser JS result: {e}")))?;
    match map {
        PendingJsMap::Eval => {
            if value.get("ok").and_then(|v| v.as_bool()) == Some(false) {
                let err = value.get("error").and_then(|v| v.as_str()).unwrap_or("eval failed");
                return Err(JsonRpcError::internal(err.to_string()));
            }
            let result = value.get("value").cloned().unwrap_or(Value::Null);
            Ok(json!({ "value": result }))
        }
        PendingJsMap::Snapshot | PendingJsMap::Action => {
            if value.get("ok").and_then(|v| v.as_bool()) == Some(false) {
                let err =
                    value.get("error").and_then(|v| v.as_str()).unwrap_or("browser action failed");
                return Err(JsonRpcError::internal(err.to_string()));
            }
            Ok(value)
        }
    }
}

/// Deserialize `params` into `T`, mapping failures to `INVALID_PARAMS`.
fn parse_params<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, JsonRpcError> {
    serde_json::from_value(params)
        .map_err(|err| JsonRpcError::new(INVALID_PARAMS, format!("invalid params: {err}")))
}

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

fn workspace_create(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: WorkspaceCreateParams = parse_params(params)?;
    let custom_name = params.name.filter(|n| !n.trim().is_empty());
    let seed = custom_name.clone().unwrap_or_else(|| "Terminal".to_string());
    let id = app.create_workspace_with_terminal(seed);
    if let Some(name) = custom_name {
        app.workspace_manager.rename_workspace(crate::workspace::model::WorkspaceId(id), name);
    }
    Ok(json!({ "id": id }))
}

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

fn workspace_close(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: WorkspaceCloseParams = parse_params(params)?;
    app.workspace_manager
        .close_workspace(WorkspaceId(params.id))
        .map_err(|err| JsonRpcError::internal(err.to_string()))?;
    app.publish_event("workspace.closed", json!({ "id": params.id }));
    Ok(json!({}))
}

fn surface_list(app: &RmuxApp) -> Value {
    let active_index = app.workspace_manager.active_index();
    let mut surfaces = Vec::new();
    for (i, ws) in app.workspace_manager.workspaces().iter().enumerate() {
        for pane in ws.pane_ids() {
            let kind = if ws.root.is_browser_pane(pane) { "browser" } else { "terminal" };
            surfaces.push(json!({
                "pane_id": pane.0,
                "workspace_id": ws.id.0,
                "active": i == active_index && pane == ws.active_pane,
                "kind": kind,
            }));
        }
    }
    json!({ "surfaces": surfaces })
}

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

fn surface_focus(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: SurfaceFocusParams = parse_params(params)?;
    if app.workspace_manager.focus_pane_global(PaneId(params.pane_id)) {
        Ok(json!({}))
    } else {
        Err(JsonRpcError::new(INVALID_PARAMS, format!("pane not found: {}", params.pane_id)))
    }
}

fn surface_send_text(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: SurfaceSendTextParams = parse_params(params)?;
    send_to_active_terminal(app, &params.text)
}

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

fn send_to_active_terminal(app: &mut RmuxApp, text: &str) -> Result<Value, JsonRpcError> {
    match app.workspace_manager.active_mut().active_terminal() {
        Some(terminal) => {
            terminal.send_text(text);
            Ok(json!({}))
        }
        None => Err(JsonRpcError::internal("active pane has no terminal")),
    }
}

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

fn sidebar_set_status(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: SidebarSetStatusParams = parse_params(params)?;
    let workspace = resolve_workspace_mut(app, params.workspace_id)?;
    workspace.status = Some(params.status);
    Ok(json!({}))
}

fn sidebar_clear_status(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: SidebarClearStatusParams = parse_params(params)?;
    let workspace = resolve_workspace_mut(app, params.workspace_id)?;
    workspace.status = None;
    workspace.progress = None;
    Ok(json!({}))
}

fn sidebar_set_progress(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: SidebarSetProgressParams = parse_params(params)?;
    let safe = if params.value.is_finite() { params.value.clamp(0.0, 1.0) } else { 0.0 };
    app.workspace_manager.active_mut().progress = Some(safe);
    Ok(json!({}))
}

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

// ── Browser handlers ───────────────────────────────────────────────────

enum HistoryOp {
    Back,
    Forward,
    Reload,
}

enum ActionKind {
    Click,
    Type,
    Fill,
    Press,
}

fn browser_open(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: BrowserOpenParams = parse_params(params)?;
    let pane_id = app
        .open_browser_split(params.url.as_deref())
        .map_err(|err| JsonRpcError::internal(err.to_string()))?;
    Ok(json!({ "pane_id": pane_id }))
}

fn browser_navigate(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: BrowserNavigateParams = parse_params(params)?;
    let browser = resolve_browser_mut(app, params.pane_id)?;
    browser.navigate(&params.url).map_err(|err| JsonRpcError::internal(err.to_string()))?;
    Ok(json!({ "url": browser.url() }))
}

fn browser_history(app: &mut RmuxApp, params: Value, op: HistoryOp) -> Result<Value, JsonRpcError> {
    let params: BrowserPaneParams = parse_params(params)?;
    let browser = resolve_browser_mut(app, params.pane_id)?;
    match op {
        HistoryOp::Back => browser.go_back(),
        HistoryOp::Forward => browser.go_forward(),
        HistoryOp::Reload => browser.reload(),
    }
    .map_err(|err| JsonRpcError::internal(err.to_string()))?;
    Ok(json!({ "url": browser.url() }))
}

fn browser_url(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    let params: BrowserPaneParams = parse_params(params)?;
    let (pane_id, browser) = resolve_browser_with_id(app, params.pane_id)?;
    Ok(json!({
        "url": browser.url(),
        "title": browser.title(),
        "loading": browser.is_loading(),
        "pane_id": pane_id.0,
    }))
}

fn browser_eval(app: &mut RmuxApp, params: Value) -> DispatchOutcome {
    let params: BrowserEvalParams = match parse_params(params) {
        Ok(p) => p,
        Err(e) => return Immediate(Err(e)),
    };
    let browser = match resolve_browser_mut(app, params.pane_id) {
        Ok(b) => b,
        Err(e) => return Immediate(Err(e)),
    };
    let script = automation::wrap_eval_expression(&params.script);
    match browser.run_automation_async(&script) {
        Ok(rx) => PendingJs { result_rx: rx, map: PendingJsMap::Eval },
        Err(e) => Immediate(Err(JsonRpcError::internal(e.to_string()))),
    }
}

fn browser_snapshot(app: &mut RmuxApp, params: Value) -> DispatchOutcome {
    let params: BrowserSnapshotParams = match parse_params(params) {
        Ok(p) => p,
        Err(e) => return Immediate(Err(e)),
    };
    let browser = match resolve_browser_mut(app, params.pane_id) {
        Ok(b) => b,
        Err(e) => return Immediate(Err(e)),
    };
    let depth = params.max_depth.unwrap_or(8).clamp(1, 20);
    let children = params.max_children.unwrap_or(40).clamp(1, 200);
    let script = automation::script_snapshot(depth, children);
    match browser.run_automation_async(&script) {
        Ok(rx) => PendingJs { result_rx: rx, map: PendingJsMap::Snapshot },
        Err(e) => Immediate(Err(JsonRpcError::internal(e.to_string()))),
    }
}

fn browser_screenshot(app: &mut RmuxApp, params: Value) -> Result<Value, JsonRpcError> {
    use base64::Engine;

    let params: BrowserScreenshotParams = parse_params(params)?;
    let (pane_id, browser) = resolve_browser_with_id(app, params.pane_id)?;

    let png = browser.screenshot_png().map_err(|e| JsonRpcError::internal(e.to_string()))?;
    let (width, height) = browser.last_frame_size().unwrap_or((0, 0));

    let want_b64 = params.include_base64.unwrap_or(params.path.is_none());
    let path_out = params.path.as_ref().map(std::path::PathBuf::from).or_else(|| {
        if want_b64 {
            None
        } else {
            let mut path = std::env::temp_dir();
            path.push(format!("rmux-screenshot-{}-{}.png", std::process::id(), pane_id.0));
            Some(path)
        }
    });

    let path_str = if let Some(ref path) = path_out {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|e| {
                JsonRpcError::internal(format!("create screenshot dir: {e}"))
            })?;
        }
        std::fs::write(path, &png)
            .map_err(|e| JsonRpcError::internal(format!("write screenshot: {e}")))?;
        Some(path.display().to_string())
    } else {
        None
    };

    let png_base64 = if want_b64 {
        Some(base64::engine::general_purpose::STANDARD.encode(&png))
    } else {
        None
    };

    Ok(json!({
        "path": path_str,
        "png_base64": png_base64,
        "width": width,
        "height": height,
        "pane_id": pane_id.0,
    }))
}

fn browser_action(app: &mut RmuxApp, params: Value, kind: ActionKind) -> DispatchOutcome {
    match kind {
        ActionKind::Click => {
            let p: BrowserSelectorParams = match parse_params(params) {
                Ok(p) => p,
                Err(e) => return Immediate(Err(e)),
            };
            let browser = match resolve_browser_mut(app, p.pane_id) {
                Ok(b) => b,
                Err(e) => return Immediate(Err(e)),
            };
            schedule_action(browser, &automation::script_click(&p.selector))
        }
        ActionKind::Fill => {
            let p: BrowserFillParams = match parse_params(params) {
                Ok(p) => p,
                Err(e) => return Immediate(Err(e)),
            };
            let browser = match resolve_browser_mut(app, p.pane_id) {
                Ok(b) => b,
                Err(e) => return Immediate(Err(e)),
            };
            schedule_action(browser, &automation::script_fill(&p.selector, &p.value))
        }
        ActionKind::Type => {
            let p: BrowserTypeParams = match parse_params(params) {
                Ok(p) => p,
                Err(e) => return Immediate(Err(e)),
            };
            let browser = match resolve_browser_mut(app, p.pane_id) {
                Ok(b) => b,
                Err(e) => return Immediate(Err(e)),
            };
            schedule_action(browser, &automation::script_type(&p.text, p.selector.as_deref()))
        }
        ActionKind::Press => {
            let p: BrowserPressParams = match parse_params(params) {
                Ok(p) => p,
                Err(e) => return Immediate(Err(e)),
            };
            let browser = match resolve_browser_mut(app, p.pane_id) {
                Ok(b) => b,
                Err(e) => return Immediate(Err(e)),
            };
            schedule_action(browser, &automation::script_press(&p.key, p.selector.as_deref()))
        }
    }
}

fn schedule_action(browser: &mut BrowserPane, script: &str) -> DispatchOutcome {
    match browser.run_automation_async(script) {
        Ok(rx) => PendingJs { result_rx: rx, map: PendingJsMap::Action },
        Err(e) => Immediate(Err(JsonRpcError::internal(e.to_string()))),
    }
}

/// Resolve a browser pane by optional id (active browser when `None`).
fn resolve_browser_mut(
    app: &mut RmuxApp,
    pane_id: Option<u64>,
) -> Result<&mut BrowserPane, JsonRpcError> {
    Ok(resolve_browser_with_id(app, pane_id)?.1)
}

fn resolve_browser_with_id(
    app: &mut RmuxApp,
    pane_id: Option<u64>,
) -> Result<(PaneId, &mut BrowserPane), JsonRpcError> {
    let target = match pane_id {
        Some(id) => PaneId(id),
        None => {
            let active_pane = app.workspace_manager.active().active_pane;
            if app.workspace_manager.active().root.is_browser_pane(active_pane) {
                active_pane
            } else {
                // Fall back: first browser in the active workspace.
                let mut found: Option<PaneId> = None;
                app.workspace_manager.active_mut().root.for_each_browser_mut(&mut |id, _| {
                    if found.is_none() {
                        found = Some(id);
                    }
                });
                found.ok_or_else(|| {
                    JsonRpcError::internal(
                        "no browser pane available — call browser.open or focus a browser pane",
                    )
                })?
            }
        }
    };

    for ws in app.workspace_manager.workspaces_mut() {
        if let Some(b) = ws.root.find_browser_mut(target) {
            return Ok((target, b));
        }
    }
    Err(JsonRpcError::new(INVALID_PARAMS, format!("browser pane not found: {}", target.0)))
}
