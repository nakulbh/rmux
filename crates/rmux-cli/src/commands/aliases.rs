//! Flat Phase 3 CLI aliases for backward compatibility.
//!
//! Prefer the hierarchical domain commands (`system ping`, `workspace list`, …).
//! These keep existing scripts working without changes.

use std::path::Path;

use anyhow::Result;
use clap::{Subcommand, ValueEnum};

use crate::commands::{notification, surface, system, workspace};
use crate::output::OutputOpts;

/// Flat subcommands retained for back-compat with the Phase 3 CLI.
#[derive(Subcommand, Debug)]
pub enum AliasCommand {
    /// Check that rmux is reachable (prints "pong")
    Ping,
    /// Print the server capabilities as pretty JSON
    Capabilities,
    /// Create a notification in rmux
    Notify {
        /// Notification title
        #[arg(long)]
        title: String,
        /// Notification subtitle
        #[arg(long)]
        subtitle: Option<String>,
        /// Notification body text
        #[arg(long)]
        body: Option<String>,
    },
    /// Create a new workspace
    NewWorkspace {
        /// Optional workspace name
        name: Option<String>,
    },
    /// List workspaces
    ListWorkspaces {
        /// Print the raw JSON result instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Split the focused pane
    NewSplit {
        /// Direction to split in
        #[arg(value_enum)]
        direction: AliasSplitDirection,
    },
    /// Send text to the active pane
    Send {
        /// Text to send. Backslash escapes \n, \r, \t, \e and \\ are interpreted
        text: String,
    },
}

/// Split direction for the legacy `new-split` alias.
#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum AliasSplitDirection {
    /// Split to the right
    Right,
    /// Split downward
    Down,
}

/// Run a legacy flat alias by forwarding to the hierarchical domain modules.
///
/// # Errors
///
/// Propagates socket and formatting errors from the domain handlers.
pub fn run(cmd: AliasCommand, socket_path: &Path, opts: OutputOpts) -> Result<()> {
    match cmd {
        AliasCommand::Ping => system::run(system::SystemCommand::Ping, socket_path, opts),
        AliasCommand::Capabilities => {
            system::run(system::SystemCommand::Capabilities, socket_path, opts)
        }
        AliasCommand::Notify { title, subtitle, body } => notification::run(
            notification::NotificationCommand::Create { title, subtitle, body },
            socket_path,
            opts,
        ),
        AliasCommand::NewWorkspace { name } => {
            workspace::run(workspace::WorkspaceCommand::Create { name }, socket_path, opts)
        }
        AliasCommand::ListWorkspaces { json } => {
            // Local `--json` on the alias overrides the global flag.
            let mut opts = opts;
            if json {
                opts.json = true;
            }
            workspace::run(workspace::WorkspaceCommand::List, socket_path, opts)
        }
        AliasCommand::NewSplit { direction } => surface::run(
            surface::SurfaceCommand::Split {
                direction: match direction {
                    AliasSplitDirection::Right => surface::SplitDirection::Right,
                    AliasSplitDirection::Down => surface::SplitDirection::Down,
                },
            },
            socket_path,
            opts,
        ),
        AliasCommand::Send { text } => {
            surface::run(surface::SurfaceCommand::Send { text }, socket_path, opts)
        }
    }
}
