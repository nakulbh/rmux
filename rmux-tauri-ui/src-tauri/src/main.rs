#![forbid(unsafe_code)]
//! rmux Tauri v2 backend entry point.
//!
//! Provides all Tauri commands for terminal management, workspace
//! management, pane tree operations, and notifications.

mod notifications;
mod pane;
mod pty;
mod workspace;

use std::sync::{Arc, Mutex};

use tauri::State;

use crate::notifications::NotificationManager;
use crate::pane::PaneId;
use crate::workspace::{Workspace, WorkspaceId, WorkspaceManager};

// ------------------------------------------------------------------
// Shared application state
// ------------------------------------------------------------------

/// Thread-safe application state shared across Tauri commands.
pub struct AppState {
    workspaces: Arc<Mutex<WorkspaceManager>>,
    notifications: Arc<Mutex<NotificationManager>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            workspaces: Arc::new(Mutex::new(WorkspaceManager::new())),
            notifications: Arc::new(Mutex::new(NotificationManager::new())),
        }
    }
}

// ------------------------------------------------------------------
// Terminal commands
// ------------------------------------------------------------------

/// Spawn a new terminal PTY session for a pane in a workspace.
#[tauri::command]
fn spawn_terminal(
    state: State<'_, AppState>,
    workspace_id: u64,
    pane_id: u64,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let mut mgr = state.workspaces.lock().map_err(|e| e.to_string())?;
    mgr.spawn_terminal(WorkspaceId(workspace_id), PaneId(pane_id), cols, rows)
        .map_err(|e| e.to_string())
}

/// Write input bytes to a terminal pane.
#[tauri::command]
fn write_terminal(
    state: State<'_, AppState>,
    pane_id: u64,
    data: Vec<u8>,
) -> Result<(), String> {
    let mut mgr = state.workspaces.lock().map_err(|e| e.to_string())?;
    mgr.write_terminal(PaneId(pane_id), &data)
        .map_err(|e| e.to_string())
}

/// Read buffered output from a terminal pane (returns base64-encoded bytes).
#[tauri::command]
fn read_terminal(
    state: State<'_, AppState>,
    pane_id: u64,
) -> Result<String, String> {
    let mut mgr = state.workspaces.lock().map_err(|e| e.to_string())?;
    let bytes = mgr.read_terminal(PaneId(pane_id));
    Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes))
}

/// Resize a terminal pane.
#[tauri::command]
fn resize_terminal(
    state: State<'_, AppState>,
    pane_id: u64,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let mut mgr = state.workspaces.lock().map_err(|e| e.to_string())?;
    mgr.resize_terminal(PaneId(pane_id), cols, rows)
        .map_err(|e| e.to_string())
}

/// Close a terminal pane and its PTY session.
#[tauri::command]
fn close_terminal(
    state: State<'_, AppState>,
    pane_id: u64,
) -> Result<(), String> {
    let mut mgr = state.workspaces.lock().map_err(|e| e.to_string())?;
    mgr.close_terminal(PaneId(pane_id))
        .map_err(|e| e.to_string())
}

// ------------------------------------------------------------------
// Workspace commands
// ------------------------------------------------------------------

/// Create a new workspace and return its ID.
#[tauri::command]
fn create_workspace(
    state: State<'_, AppState>,
    name: String,
) -> Result<u64, String> {
    let mut mgr = state.workspaces.lock().map_err(|e| e.to_string())?;
    let id = mgr.create_workspace(name);
    Ok(id.0)
}

/// Switch the active workspace.
#[tauri::command]
fn switch_workspace(
    state: State<'_, AppState>,
    workspace_id: u64,
) -> Result<(), String> {
    let mut mgr = state.workspaces.lock().map_err(|e| e.to_string())?;
    if mgr.switch_workspace(WorkspaceId(workspace_id)) {
        Ok(())
    } else {
        Err("Workspace not found".to_string())
    }
}

/// Close a workspace and all its panes.
#[tauri::command]
fn close_workspace(
    state: State<'_, AppState>,
    workspace_id: u64,
) -> Result<(), String> {
    let mut mgr = state.workspaces.lock().map_err(|e| e.to_string())?;
    if mgr.close_workspace(WorkspaceId(workspace_id)) {
        Ok(())
    } else {
        Err("Workspace not found".to_string())
    }
}

/// List all workspaces.
#[tauri::command]
fn list_workspaces(
    state: State<'_, AppState>,
) -> Result<Vec<Workspace>, String> {
    let mgr = state.workspaces.lock().map_err(|e| e.to_string())?;
    Ok(mgr.list_workspaces().into_iter().cloned().collect())
}

/// Rename a workspace.
#[tauri::command]
fn rename_workspace(
    state: State<'_, AppState>,
    workspace_id: u64,
    name: String,
) -> Result<(), String> {
    let mut mgr = state.workspaces.lock().map_err(|e| e.to_string())?;
    if mgr.rename_workspace(WorkspaceId(workspace_id), name) {
        Ok(())
    } else {
        Err("Workspace not found".to_string())
    }
}

// ------------------------------------------------------------------
// Pane commands
// ------------------------------------------------------------------

/// Split a pane to the right (horizontal split).
#[tauri::command]
fn split_pane_right(
    state: State<'_, AppState>,
    workspace_id: u64,
    pane_id: u64,
) -> Result<u64, String> {
    let mut mgr = state.workspaces.lock().map_err(|e| e.to_string())?;
    let new_id = mgr.split_pane_right(WorkspaceId(workspace_id), PaneId(pane_id))
        .map_err(|e| e.to_string())?;
    Ok(new_id.0)
}

/// Split a pane downward (vertical split).
#[tauri::command]
fn split_pane_down(
    state: State<'_, AppState>,
    workspace_id: u64,
    pane_id: u64,
) -> Result<u64, String> {
    let mut mgr = state.workspaces.lock().map_err(|e| e.to_string())?;
    let new_id = mgr.split_pane_down(WorkspaceId(workspace_id), PaneId(pane_id))
        .map_err(|e| e.to_string())?;
    Ok(new_id.0)
}

/// Close a pane in a workspace.
#[tauri::command]
fn close_pane(
    state: State<'_, AppState>,
    workspace_id: u64,
    pane_id: u64,
) -> Result<(), String> {
    let mut mgr = state.workspaces.lock().map_err(|e| e.to_string())?;
    mgr.close_pane(WorkspaceId(workspace_id), PaneId(pane_id))
        .map_err(|e| e.to_string())
}

/// Focus a pane in a workspace.
#[tauri::command]
fn focus_pane(
    state: State<'_, AppState>,
    workspace_id: u64,
    pane_id: u64,
) -> Result<(), String> {
    let mut mgr = state.workspaces.lock().map_err(|e| e.to_string())?;
    if mgr.focus_pane(WorkspaceId(workspace_id), PaneId(pane_id)) {
        Ok(())
    } else {
        Err("Pane not found in workspace".to_string())
    }
}

// ------------------------------------------------------------------
// Notification commands
// ------------------------------------------------------------------

/// Get all notifications.
#[tauri::command]
fn get_notifications(
    state: State<'_, AppState>,
) -> Result<Vec<crate::notifications::Notification>, String> {
    let mgr = state.notifications.lock().map_err(|e| e.to_string())?;
    Ok(mgr.list().to_vec())
}

/// Dismiss (mark as read) a single notification.
#[tauri::command]
fn dismiss_notification(
    state: State<'_, AppState>,
    notification_id: u64,
) -> Result<(), String> {
    let mut mgr = state.notifications.lock().map_err(|e| e.to_string())?;
    mgr.mark_read(notification_id);
    Ok(())
}

/// Dismiss all notifications.
#[tauri::command]
fn dismiss_all_notifications(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut mgr = state.notifications.lock().map_err(|e| e.to_string())?;
    mgr.mark_all_read();
    Ok(())
}

// ------------------------------------------------------------------
// Entry point
// ------------------------------------------------------------------

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            spawn_terminal,
            write_terminal,
            read_terminal,
            resize_terminal,
            close_terminal,
            create_workspace,
            switch_workspace,
            close_workspace,
            list_workspaces,
            rename_workspace,
            split_pane_right,
            split_pane_down,
            close_pane,
            focus_pane,
            get_notifications,
            dismiss_notification,
            dismiss_all_notifications,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
