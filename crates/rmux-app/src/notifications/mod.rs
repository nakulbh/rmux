//! Notification data model and desktop notification emission.
//!
//! Holds the [`Notification`] record type, the [`DesktopNotifier`] trait
//! used to abstract desktop notification emission (so tests never fire
//! real notifications), and the [`NotificationManager`] (in `manager`)
//! that stores and tracks notifications.
//!
//! IDs reference panes and workspaces as raw `u64` values (the inner
//! value of `PaneId` / `WorkspaceId`) to avoid import cycles with the
//! workspace module.
mod manager;

pub use manager::NotificationManager;

use std::time::SystemTime;

/// A single notification, either raised by a pane's OSC output or
/// created externally (e.g. via the CLI/socket API).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Unique, monotonically increasing identifier.
    pub id: u64,
    /// Raw pane id (`PaneId.0`) that raised this notification; `None`
    /// for external (CLI-created) notifications.
    pub pane_id: Option<u64>,
    /// Raw workspace id (`WorkspaceId.0`) the notification belongs to.
    pub workspace_id: Option<u64>,
    /// Notification title.
    pub title: String,
    /// Optional notification body text.
    pub body: Option<String>,
    /// When the notification was created.
    pub timestamp: SystemTime,
    /// Whether the user has seen this notification.
    pub read: bool,
}

/// Abstraction over desktop notification emission.
///
/// The real implementation ([`SystemNotifier`]) forwards to the OS
/// notification system; tests substitute a recording fake so no real
/// notifications are fired.
pub trait DesktopNotifier: Send {
    /// Emit a desktop notification with the given title and optional body.
    ///
    /// Implementations must not panic on failure — emission is best-effort.
    fn notify(&self, title: &str, body: Option<&str>);
}

/// [`DesktopNotifier`] backed by the OS notification system via `notify-rust`.
pub struct SystemNotifier;

impl DesktopNotifier for SystemNotifier {
    fn notify(&self, title: &str, body: Option<&str>) {
        let mut notification = notify_rust::Notification::new();
        notification.summary(title);
        if let Some(body) = body {
            notification.body(body);
        }
        // Best-effort: a failed desktop notification must never crash
        // the app (e.g. no notification daemon running on Linux).
        if let Err(err) = notification.show() {
            tracing::warn!("failed to emit desktop notification: {err}");
        }
    }
}
