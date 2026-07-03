//! UI module — egui-based user interface components.
//!
//! # Components
//!
//! - `sidebar` — `SidebarView` for workspace tab navigation
//! - `workspace_view` — Pane tree renderer for split layouts
//! - `terminal_pane` — Terminal pane widget (PTY + rendering + input)

pub mod sidebar;
mod terminal_pane;
pub mod workspace_view;

pub use terminal_pane::TerminalPane;
