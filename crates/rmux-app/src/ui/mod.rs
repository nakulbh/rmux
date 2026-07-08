// UI module — egui-based user interface components.
//!
//! # Components
//!
//! - `top_bar` — Top chrome bar (sidebar toggle, title, bell)
//! - `sidebar` — `SidebarView` for workspace tab navigation
//! - `workspace_view` — Pane tree renderer for split layouts
//! - `terminal_pane` — Terminal pane widget (PTY + rendering + input)
//! - `notification_panel` — Right-side notification list panel
//! - `status_bar` — Bottom status bar (workspace context, counts)

pub mod notification_panel;
pub mod sidebar;
pub mod status_bar;
mod terminal_pane;
pub mod theme;
pub mod top_bar;
pub mod workspace_view;

pub use notification_panel::NotificationPanel;
pub use terminal_pane::{DEFAULT_FONT_SIZE, TerminalPane};
