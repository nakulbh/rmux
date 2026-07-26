//! `rmux-cli notification` — create, list, clear.

use std::path::Path;

use anyhow::Result;
use clap::Subcommand;
use rmux_api::methods;
use serde_json::{Value, json};

use crate::output::{self, OutputOpts};
use crate::socket;

/// Notification-domain subcommands.
#[derive(Subcommand, Debug)]
pub enum NotificationCommand {
    /// Create a notification in rmux
    Create {
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
    /// List pending notifications
    List,
    /// Clear all notifications
    Clear,
}

/// Run a notification subcommand.
///
/// # Errors
///
/// Returns an error if the socket call or output formatting fails.
pub fn run(cmd: NotificationCommand, socket_path: &Path, opts: OutputOpts) -> Result<()> {
    match cmd {
        NotificationCommand::Create { title, subtitle, body } => {
            let (method, params) = create_request(&title, subtitle.as_deref(), body.as_deref());
            let result = socket::call(socket_path, method, params)?;
            output::print_id(&result, opts)
        }
        NotificationCommand::List => {
            let (method, params) = list_request();
            let result = socket::call(socket_path, method, params)?;
            output::print_result(&result, opts, output::format_notification_table)
        }
        NotificationCommand::Clear => {
            let (method, params) = clear_request();
            let result = socket::call(socket_path, method, params)?;
            output::print_silent_or_json(&result, opts)
        }
    }
}

pub(crate) fn create_request(
    title: &str,
    subtitle: Option<&str>,
    body: Option<&str>,
) -> (&'static str, Value) {
    (methods::NOTIFICATION_CREATE, json!({ "title": title, "subtitle": subtitle, "body": body }))
}

pub(crate) fn list_request() -> (&'static str, Value) {
    (methods::NOTIFICATION_LIST, json!({}))
}

pub(crate) fn clear_request() -> (&'static str, Value) {
    (methods::NOTIFICATION_CLEAR, json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_request_includes_all_fields() {
        let (method, params) = create_request("Build", Some("rmux"), Some("done"));
        assert_eq!(method, methods::NOTIFICATION_CREATE);
        assert_eq!(params, json!({ "title": "Build", "subtitle": "rmux", "body": "done" }));
    }
}
