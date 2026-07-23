//! Typed method registry for the socket API.
//!
//! Defines the method-name constants plus serde parameter/result
//! structs that form the wire contract of the Phase 3 method set.
//! The application implements the semantics; this module only pins
//! down names and shapes so both sides (and external clients) agree.

use serde::{Deserialize, Serialize};

/// Health check; result is [`PingResult`].
pub const SYSTEM_PING: &str = "system.ping";
/// Report protocol version and supported methods; result is [`CapabilitiesResult`].
pub const SYSTEM_CAPABILITIES: &str = "system.capabilities";
/// Identify the serving application; result is [`IdentifyResult`].
pub const SYSTEM_IDENTIFY: &str = "system.identify";
/// List workspaces; result is [`WorkspaceListResult`].
pub const WORKSPACE_LIST: &str = "workspace.list";
/// Create a workspace; params [`WorkspaceCreateParams`], result [`WorkspaceCreateResult`].
pub const WORKSPACE_CREATE: &str = "workspace.create";
/// Select a workspace by index; params [`WorkspaceSelectParams`].
pub const WORKSPACE_SELECT: &str = "workspace.select";
/// Close a workspace by id; params [`WorkspaceCloseParams`].
pub const WORKSPACE_CLOSE: &str = "workspace.close";
/// List panes across workspaces; result is [`SurfaceListResult`].
pub const SURFACE_LIST: &str = "surface.list";
/// Split the active pane; params [`SurfaceSplitParams`], result [`SurfaceSplitResult`].
pub const SURFACE_SPLIT: &str = "surface.split";
/// Focus a pane; params [`SurfaceFocusParams`].
pub const SURFACE_FOCUS: &str = "surface.focus";
/// Type text into the active pane; params [`SurfaceSendTextParams`].
pub const SURFACE_SEND_TEXT: &str = "surface.send_text";
/// Send a named key to the active pane; params [`SurfaceSendKeyParams`].
pub const SURFACE_SEND_KEY: &str = "surface.send_key";
/// Create a notification; params [`NotificationCreateParams`], result
/// [`NotificationCreateResult`].
pub const NOTIFICATION_CREATE: &str = "notification.create";
/// List pending notifications; result is [`NotificationListResult`].
pub const NOTIFICATION_LIST: &str = "notification.list";
/// Clear all notifications.
pub const NOTIFICATION_CLEAR: &str = "notification.clear";
/// Set a sidebar status string; params [`SidebarSetStatusParams`].
pub const SIDEBAR_SET_STATUS: &str = "sidebar.set_status";
/// Clear a sidebar status string; params [`SidebarClearStatusParams`].
pub const SIDEBAR_CLEAR_STATUS: &str = "sidebar.clear_status";
/// Set the sidebar progress indicator; params [`SidebarSetProgressParams`].
pub const SIDEBAR_SET_PROGRESS: &str = "sidebar.set_progress";
/// Switch the connection to event-streaming mode (handled by the server).
pub const EVENTS_STREAM: &str = "events.stream";

/// All method names supported by the Phase 3 protocol.
///
/// # Examples
///
/// ```
/// assert!(rmux_api::methods::all_methods().contains(&"system.ping"));
/// ```
#[must_use]
pub fn all_methods() -> &'static [&'static str] {
    &[
        SYSTEM_PING,
        SYSTEM_CAPABILITIES,
        SYSTEM_IDENTIFY,
        WORKSPACE_LIST,
        WORKSPACE_CREATE,
        WORKSPACE_SELECT,
        WORKSPACE_CLOSE,
        SURFACE_LIST,
        SURFACE_SPLIT,
        SURFACE_FOCUS,
        SURFACE_SEND_TEXT,
        SURFACE_SEND_KEY,
        NOTIFICATION_CREATE,
        NOTIFICATION_LIST,
        NOTIFICATION_CLEAR,
        SIDEBAR_SET_STATUS,
        SIDEBAR_CLEAR_STATUS,
        SIDEBAR_SET_PROGRESS,
        EVENTS_STREAM,
    ]
}

/// Result of [`SYSTEM_PING`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PingResult {
    /// Always `true`.
    pub pong: bool,
}

/// Result of [`SYSTEM_CAPABILITIES`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitiesResult {
    /// Application version string.
    pub version: String,
    /// All supported method names (see [`all_methods`]).
    pub methods: Vec<String>,
}

/// Result of [`SYSTEM_IDENTIFY`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentifyResult {
    /// Application name, always `"rmux"`.
    pub app: String,
    /// Application version string.
    pub version: String,
    /// Process id of the serving application.
    pub pid: u32,
}

/// One workspace entry in [`WorkspaceListResult`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    /// Stable workspace id.
    pub id: u64,
    /// Display name.
    pub name: String,
    /// Number of panes in the workspace.
    pub pane_count: usize,
    /// Whether this is the active workspace.
    pub active: bool,
}

/// Result of [`WORKSPACE_LIST`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceListResult {
    /// All workspaces in display order.
    pub workspaces: Vec<WorkspaceInfo>,
}

/// Parameters of [`WORKSPACE_CREATE`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCreateParams {
    /// Optional display name; the app picks a default when omitted.
    #[serde(default)]
    pub name: Option<String>,
}

/// Result of [`WORKSPACE_CREATE`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCreateResult {
    /// Id of the newly created workspace.
    pub id: u64,
}

/// Parameters of [`WORKSPACE_SELECT`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSelectParams {
    /// Zero-based index into the workspace list.
    pub index: usize,
}

/// Parameters of [`WORKSPACE_CLOSE`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCloseParams {
    /// Id of the workspace to close.
    pub id: u64,
}

/// One pane entry in [`SurfaceListResult`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceInfo {
    /// Stable pane id.
    pub pane_id: u64,
    /// Id of the workspace containing this pane.
    pub workspace_id: u64,
    /// Whether this pane has focus.
    pub active: bool,
}

/// Result of [`SURFACE_LIST`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceListResult {
    /// All panes across all workspaces.
    pub surfaces: Vec<SurfaceInfo>,
}

/// Split direction for [`SURFACE_SPLIT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SplitDirection {
    /// Split to the right (new pane on the right).
    Right,
    /// Split downward (new pane below).
    Down,
}

/// Parameters of [`SURFACE_SPLIT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceSplitParams {
    /// Direction of the split: `"right"` or `"down"`.
    pub direction: SplitDirection,
}

/// Result of [`SURFACE_SPLIT`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceSplitResult {
    /// Id of the newly created pane.
    pub pane_id: u64,
}

/// Parameters of [`SURFACE_FOCUS`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceFocusParams {
    /// Id of the pane to focus.
    pub pane_id: u64,
}

/// Parameters of [`SURFACE_SEND_TEXT`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceSendTextParams {
    /// Literal text to type into the active pane.
    pub text: String,
}

/// Parameters of [`SURFACE_SEND_KEY`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceSendKeyParams {
    /// Named key to send, e.g. `"enter"` or `"ctrl+c"`.
    pub key: String,
}

/// Parameters of [`NOTIFICATION_CREATE`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationCreateParams {
    /// Notification title.
    pub title: String,
    /// Optional subtitle.
    #[serde(default)]
    pub subtitle: Option<String>,
    /// Optional body text.
    #[serde(default)]
    pub body: Option<String>,
    /// Workspace that originated the notification (sidebar badge routing).
    #[serde(default)]
    pub workspace_id: Option<u64>,
    /// Pane/surface that originated the notification (jump-to-pane).
    #[serde(default)]
    pub pane_id: Option<u64>,
}

/// Result of [`NOTIFICATION_CREATE`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationCreateResult {
    /// Id of the newly created notification.
    pub id: u64,
}

/// One notification entry in [`NotificationListResult`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationInfo {
    /// Stable notification id.
    pub id: u64,
    /// Notification title.
    pub title: String,
    /// Optional subtitle.
    #[serde(default)]
    pub subtitle: Option<String>,
    /// Optional body text.
    #[serde(default)]
    pub body: Option<String>,
}

/// Result of [`NOTIFICATION_LIST`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationListResult {
    /// All pending notifications.
    pub notifications: Vec<NotificationInfo>,
}

/// Parameters of [`SIDEBAR_SET_STATUS`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidebarSetStatusParams {
    /// Target workspace; the active workspace when omitted.
    #[serde(default)]
    pub workspace_id: Option<u64>,
    /// Status string to display.
    pub status: String,
}

/// Parameters of [`SIDEBAR_CLEAR_STATUS`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidebarClearStatusParams {
    /// Target workspace; the active workspace when omitted.
    #[serde(default)]
    pub workspace_id: Option<u64>,
}

/// Parameters of [`SIDEBAR_SET_PROGRESS`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SidebarSetProgressParams {
    /// Progress value in `0.0..=1.0`.
    pub value: f32,
}
