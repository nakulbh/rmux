//! `rmux-cli sidebar` — status text and progress indicator.

use std::path::Path;

use anyhow::Result;
use clap::Subcommand;
use rmux_api::methods;
use serde_json::{Value, json};

use crate::output::{self, OutputOpts};
use crate::socket;

/// Sidebar-domain subcommands.
#[derive(Subcommand, Debug)]
pub enum SidebarCommand {
    /// Status text on a workspace tab
    #[command(subcommand)]
    Status(StatusCommand),
    /// Set the active workspace progress bar (0.0..=1.0)
    Progress {
        /// Progress value in 0.0..=1.0
        value: f32,
    },
}

/// Status set / clear.
#[derive(Subcommand, Debug)]
pub enum StatusCommand {
    /// Set the status text
    Set {
        /// Status string to display
        status: String,
        /// Target workspace id (active when omitted)
        #[arg(long)]
        workspace: Option<u64>,
    },
    /// Clear the status text and progress bar
    Clear {
        /// Target workspace id (active when omitted)
        #[arg(long)]
        workspace: Option<u64>,
    },
}

/// Run a sidebar subcommand.
///
/// # Errors
///
/// Returns an error if the socket call or output formatting fails.
pub fn run(cmd: SidebarCommand, socket_path: &Path, opts: OutputOpts) -> Result<()> {
    match cmd {
        SidebarCommand::Status(StatusCommand::Set { status, workspace }) => {
            let (method, params) = set_status_request(workspace, &status);
            let result = socket::call(socket_path, method, params)?;
            output::print_silent_or_json(&result, opts)
        }
        SidebarCommand::Status(StatusCommand::Clear { workspace }) => {
            let (method, params) = clear_status_request(workspace);
            let result = socket::call(socket_path, method, params)?;
            output::print_silent_or_json(&result, opts)
        }
        SidebarCommand::Progress { value } => {
            let (method, params) = set_progress_request(value);
            let result = socket::call(socket_path, method, params)?;
            output::print_silent_or_json(&result, opts)
        }
    }
}

pub(crate) fn set_status_request(workspace_id: Option<u64>, status: &str) -> (&'static str, Value) {
    (methods::SIDEBAR_SET_STATUS, json!({ "workspace_id": workspace_id, "status": status }))
}

pub(crate) fn clear_status_request(workspace_id: Option<u64>) -> (&'static str, Value) {
    (methods::SIDEBAR_CLEAR_STATUS, json!({ "workspace_id": workspace_id }))
}

pub(crate) fn set_progress_request(value: f32) -> (&'static str, Value) {
    (methods::SIDEBAR_SET_PROGRESS, json!({ "value": value }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_builders() {
        assert_eq!(
            set_status_request(Some(1), "ok").1,
            json!({ "workspace_id": 1, "status": "ok" })
        );
        assert_eq!(set_progress_request(0.5).1, json!({ "value": 0.5 }));
    }
}
