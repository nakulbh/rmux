//! `rmux-cli surface` — pane list, split, focus, send, close, new.

use std::path::Path;

use anyhow::Result;
use clap::{Subcommand, ValueEnum};
use rmux_api::methods;
use serde_json::{Value, json};

use crate::output::{self, OutputOpts};
use crate::socket;
use crate::util::interpret_escapes;

/// Surface (pane) domain subcommands.
#[derive(Subcommand, Debug)]
pub enum SurfaceCommand {
    /// List panes across all workspaces
    List,
    /// Split the focused pane
    Split {
        /// Direction to split in
        #[arg(value_enum)]
        direction: SplitDirection,
    },
    /// Focus a pane by id
    Focus {
        /// Pane id
        pane_id: u64,
    },
    /// Close a pane (active when id omitted)
    Close {
        /// Optional pane id (defaults to the active pane)
        pane_id: Option<u64>,
    },
    /// Create a new terminal tab/surface in the active workspace
    New {
        /// Optional tab title
        #[arg(long)]
        title: Option<String>,
    },
    /// Send text to the active pane (backslash escapes supported)
    Send {
        /// Text to send. Backslash escapes \n, \r, \t, \e and \\ are interpreted
        text: String,
    },
    /// Send a named key to the active pane
    Key {
        /// Named key: enter, tab, escape, ctrl+c, ctrl+d
        key: String,
    },
}

/// Split direction accepted by `surface split`.
#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum SplitDirection {
    /// Split to the right (vertical divider)
    Right,
    /// Split downward (horizontal divider)
    Down,
}

impl SplitDirection {
    /// Wire representation expected by `surface.split`.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Right => "right",
            Self::Down => "down",
        }
    }
}

/// Run a surface subcommand.
///
/// # Errors
///
/// Returns an error if the socket call or output formatting fails.
pub fn run(cmd: SurfaceCommand, socket_path: &Path, opts: OutputOpts) -> Result<()> {
    match cmd {
        SurfaceCommand::List => {
            let (method, params) = list_request();
            let result = socket::call(socket_path, method, params)?;
            output::print_result(&result, opts, output::format_surface_table)
        }
        SurfaceCommand::Split { direction } => {
            let (method, params) = split_request(direction.as_str());
            let result = socket::call(socket_path, method, params)?;
            output::print_id(&result, opts)
        }
        SurfaceCommand::Focus { pane_id } => {
            let (method, params) = focus_request(pane_id);
            let result = socket::call(socket_path, method, params)?;
            output::print_silent_or_json(&result, opts)
        }
        SurfaceCommand::Close { pane_id } => {
            let (method, params) = close_request(pane_id);
            let result = socket::call(socket_path, method, params)?;
            output::print_silent_or_json(&result, opts)
        }
        SurfaceCommand::New { title } => {
            let (method, params) = new_request(title.as_deref());
            let result = socket::call(socket_path, method, params)?;
            output::print_id(&result, opts)
        }
        SurfaceCommand::Send { text } => {
            let (method, params) = send_request(&text);
            let result = socket::call(socket_path, method, params)?;
            output::print_silent_or_json(&result, opts)
        }
        SurfaceCommand::Key { key } => {
            let (method, params) = key_request(&key);
            let result = socket::call(socket_path, method, params)?;
            output::print_silent_or_json(&result, opts)
        }
    }
}

pub(crate) fn list_request() -> (&'static str, Value) {
    (methods::SURFACE_LIST, json!({}))
}

pub(crate) fn split_request(direction: &str) -> (&'static str, Value) {
    (methods::SURFACE_SPLIT, json!({ "direction": direction }))
}

pub(crate) fn focus_request(pane_id: u64) -> (&'static str, Value) {
    (methods::SURFACE_FOCUS, json!({ "pane_id": pane_id }))
}

pub(crate) fn close_request(pane_id: Option<u64>) -> (&'static str, Value) {
    (methods::SURFACE_CLOSE, json!({ "pane_id": pane_id }))
}

pub(crate) fn new_request(title: Option<&str>) -> (&'static str, Value) {
    (methods::SURFACE_NEW, json!({ "title": title }))
}

pub(crate) fn send_request(text: &str) -> (&'static str, Value) {
    (methods::SURFACE_SEND_TEXT, json!({ "text": interpret_escapes(text) }))
}

pub(crate) fn key_request(key: &str) -> (&'static str, Value) {
    (methods::SURFACE_SEND_KEY, json!({ "key": key }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_direction_maps_to_wire_strings() {
        assert_eq!(SplitDirection::Right.as_str(), "right");
        assert_eq!(SplitDirection::Down.as_str(), "down");
    }

    #[test]
    fn request_builders() {
        assert_eq!(split_request("right").1, json!({ "direction": "right" }));
        assert_eq!(send_request("ls\\n").1, json!({ "text": "ls\n" }));
        assert_eq!(key_request("enter").1, json!({ "key": "enter" }));
        assert_eq!(close_request(Some(3)).1, json!({ "pane_id": 3 }));
        assert_eq!(new_request(Some("shell")).1, json!({ "title": "shell" }));
    }
}
