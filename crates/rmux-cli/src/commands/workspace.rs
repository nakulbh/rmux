//! `rmux-cli workspace` — create, list, select, close, rename.

use std::path::Path;

use anyhow::Result;
use clap::Subcommand;
use rmux_api::methods;
use serde_json::{Value, json};

use crate::output::{self, OutputOpts};
use crate::socket;

/// Workspace-domain subcommands.
#[derive(Subcommand, Debug)]
pub enum WorkspaceCommand {
    /// List workspaces
    List,
    /// Create a new workspace
    Create {
        /// Optional workspace name
        name: Option<String>,
    },
    /// Switch to a workspace by zero-based index
    Select {
        /// Zero-based workspace index
        index: usize,
    },
    /// Close a workspace by id
    Close {
        /// Workspace id
        id: u64,
    },
    /// Rename a workspace
    Rename {
        /// Workspace id
        id: u64,
        /// New display name
        name: String,
    },
}

/// Run a workspace subcommand.
///
/// # Errors
///
/// Returns an error if the socket call or output formatting fails.
pub fn run(cmd: WorkspaceCommand, socket_path: &Path, opts: OutputOpts) -> Result<()> {
    match cmd {
        WorkspaceCommand::List => {
            let (method, params) = list_request();
            let result = socket::call(socket_path, method, params)?;
            output::print_result(&result, opts, output::format_workspace_table)
        }
        WorkspaceCommand::Create { name } => {
            let (method, params) = create_request(name.as_deref());
            let result = socket::call(socket_path, method, params)?;
            output::print_id(&result, opts)
        }
        WorkspaceCommand::Select { index } => {
            let (method, params) = select_request(index);
            let result = socket::call(socket_path, method, params)?;
            output::print_silent_or_json(&result, opts)
        }
        WorkspaceCommand::Close { id } => {
            let (method, params) = close_request(id);
            let result = socket::call(socket_path, method, params)?;
            output::print_silent_or_json(&result, opts)
        }
        WorkspaceCommand::Rename { id, name } => {
            let (method, params) = rename_request(id, &name);
            let result = socket::call(socket_path, method, params)?;
            output::print_silent_or_json(&result, opts)
        }
    }
}

pub(crate) fn list_request() -> (&'static str, Value) {
    (methods::WORKSPACE_LIST, json!({}))
}

pub(crate) fn create_request(name: Option<&str>) -> (&'static str, Value) {
    (methods::WORKSPACE_CREATE, json!({ "name": name }))
}

pub(crate) fn select_request(index: usize) -> (&'static str, Value) {
    (methods::WORKSPACE_SELECT, json!({ "index": index }))
}

pub(crate) fn close_request(id: u64) -> (&'static str, Value) {
    (methods::WORKSPACE_CLOSE, json!({ "id": id }))
}

pub(crate) fn rename_request(id: u64, name: &str) -> (&'static str, Value) {
    (methods::WORKSPACE_RENAME, json!({ "id": id, "name": name }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_builders_use_expected_methods() {
        assert_eq!(list_request(), (methods::WORKSPACE_LIST, json!({})));
        assert_eq!(create_request(Some("dev")).1, json!({ "name": "dev" }));
        assert_eq!(create_request(None).1, json!({ "name": null }));
        assert_eq!(select_request(2).1, json!({ "index": 2 }));
        assert_eq!(close_request(9).1, json!({ "id": 9 }));
        assert_eq!(rename_request(1, "main").1, json!({ "id": 1, "name": "main" }));
        assert_eq!(create_request(None).0, methods::WORKSPACE_CREATE);
        assert_eq!(rename_request(1, "main").0, methods::WORKSPACE_RENAME);
    }
}
