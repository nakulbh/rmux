//! Hierarchical CLI command modules.
//!
//! Each domain (`system`, `workspace`, `surface`, …) owns its clap
//! subcommands, request builders, and run logic. The root [`Command`]
//! enum only aggregates domains plus back-compat aliases.

mod aliases;
mod app_cmd;
mod browser;
mod call;
mod events;
mod notification;
mod sidebar;
mod surface;
mod system;
mod workspace;

use std::path::Path;

use anyhow::Result;
use clap::Subcommand;

use crate::output::OutputOpts;

pub use aliases::AliasCommand;
pub use app_cmd::AppCommand;
pub use browser::BrowserCommand;
pub use call::CallCommand;
pub use events::EventsCommand;
pub use notification::NotificationCommand;
pub use sidebar::SidebarCommand;
pub use surface::SurfaceCommand;
pub use system::SystemCommand;
pub use workspace::WorkspaceCommand;

/// Top-level subcommands of `rmux-cli`.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// System health and capability queries
    #[command(subcommand)]
    System(SystemCommand),
    /// Workspace create / list / select / close / rename
    #[command(subcommand)]
    Workspace(WorkspaceCommand),
    /// Pane (surface) list / split / focus / send
    #[command(subcommand)]
    Surface(SurfaceCommand),
    /// Notification create / list / clear
    #[command(subcommand)]
    Notification(NotificationCommand),
    /// Sidebar status and progress
    #[command(subcommand)]
    Sidebar(SidebarCommand),
    /// Embedded browser pane control
    #[command(subcommand)]
    Browser(BrowserCommand),
    /// Application-wide settings (font, theme)
    #[command(subcommand)]
    App(AppCommand),
    /// Real-time event stream from the running app
    #[command(subcommand)]
    Events(EventsCommand),
    /// Invoke any socket method with raw JSON params (escape hatch)
    Call(CallCommand),
    /// Back-compat flat aliases from the Phase 3 CLI
    #[command(flatten)]
    Alias(AliasCommand),
}

/// Dispatch a top-level command against the socket at `socket_path`.
///
/// # Errors
///
/// Propagates socket, server, and formatting errors from domain handlers.
pub fn run(command: Command, socket_path: &Path, opts: OutputOpts) -> Result<()> {
    match command {
        Command::System(cmd) => system::run(cmd, socket_path, opts),
        Command::Workspace(cmd) => workspace::run(cmd, socket_path, opts),
        Command::Surface(cmd) => surface::run(cmd, socket_path, opts),
        Command::Notification(cmd) => notification::run(cmd, socket_path, opts),
        Command::Sidebar(cmd) => sidebar::run(cmd, socket_path, opts),
        Command::Browser(cmd) => browser::run(cmd, socket_path, opts),
        Command::App(cmd) => app_cmd::run(cmd, socket_path, opts),
        Command::Events(cmd) => events::run(cmd, socket_path, opts),
        Command::Call(cmd) => call::run(cmd, socket_path, opts),
        Command::Alias(cmd) => aliases::run(cmd, socket_path, opts),
    }
}
