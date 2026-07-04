//! UI module — egui-based user interface components.
//!
//! # Components
//!
//! - `sidebar` — `SidebarView` for workspace tab navigation
//! - `workspace_view` — Pane tree renderer for split layouts
//! - `terminal_pane` — Terminal pane widget (PTY + rendering + input)
//! - `notification_panel` — Right-side notification list panel

pub mod notification_panel;
pub mod sidebar;
mod terminal_pane;
pub mod workspace_view;

pub use notification_panel::NotificationPanel;
pub use terminal_pane::TerminalPane;
