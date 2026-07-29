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
        let title = title.to_owned();
        let body = body.map(str::to_owned);
        // Emit from a short-lived background thread: on macOS,
        // `notify-rust` pumps the native run loop while delivering the
        // notification, and doing that inside the winit event handler
        // (we are called from `update()`) re-enters the event loop and
        // aborts the process.
        let spawned = std::thread::Builder::new().name("rmux-notify".to_owned()).spawn(move || {
            // Best-effort: a failed desktop notification must never
            // crash the app (e.g. no notification daemon on Linux).
            let outcome = std::panic::catch_unwind(|| {
                #[cfg(target_os = "macos")]
                ensure_macos_identity();

                let mut notification = notify_rust::Notification::new();
                // No-op on macOS (both notify-rust backends silently ignore
                // `appname`) but required on Linux/XDG so the banner groups
                // under "rmux" — matching the `rmux.desktop` entry / icon
                // name `scripts/install.sh` installs — instead of a blank
                // or PID-derived app name.
                notification.appname("rmux");
                notification.icon("rmux");
                notification.summary(&title);
                if let Some(body) = &body {
                    notification.body(body);
                }
                if let Err(err) = notification.show() {
                    tracing::warn!("failed to emit desktop notification: {err}");
                }
            });
            if outcome.is_err() {
                tracing::warn!("desktop notification backend panicked; notification dropped");
            }
        });
        if let Err(err) = spawned {
            tracing::warn!("failed to spawn desktop notification thread: {err}");
        }
    }
}

/// Give rmux its own macOS Notification Center identity instead of silently
/// posting as `com.apple.Finder`.
///
/// `notify-rust`/`mac-notification-sys` require a registered bundle
/// identifier to post through `NSUserNotificationCenter`. When the running
/// binary isn't inside a proper `.app` bundle (any debug build, `cargo run`,
/// or a `cargo install`ed binary), the crate silently falls back to
/// `com.apple.Finder`'s identity on first send. Notifications then land in
/// Notification Center's history — proving delivery "worked" — but never
/// pop up as a banner, because Finder posts constant low-priority ejects/
/// trash notifications and most users have long since muted its alert style.
///
/// `com.nakulbh.rmux` matches the `CFBundleIdentifier` `scripts/install.sh`
/// registers for `~/Applications/rmux.app`. Launch Services keys identity by
/// bundle id, not by which binary is currently executing, so this succeeds
/// even from a raw dev binary as long as that `.app` has been installed once.
/// If it hasn't (fresh clone, never ran the installer), fall back to
/// `com.apple.Terminal` — always registered, and a more sensible identity for
/// a terminal multiplexer than Finder in the meantime.
///
/// Idempotent and cheap to call repeatedly: `mac_notification_sys` guards the
/// underlying `setApplication` call with a process-wide `Once`, so only the
/// very first call (across every notification ever sent) does any work.
#[cfg(target_os = "macos")]
fn ensure_macos_identity() {
    if notify_rust::set_application("com.nakulbh.rmux").is_err() {
        let _ = notify_rust::set_application("com.apple.Terminal");
    }
}
