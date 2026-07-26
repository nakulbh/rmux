//! `rmux-cli browser` — open and control the embedded browser pane.

use std::path::Path;

use anyhow::Result;
use clap::Subcommand;
use rmux_api::methods;
use serde_json::{Value, json};

use crate::output::{self, OutputOpts};
use crate::socket;

/// Browser-domain subcommands.
#[derive(Subcommand, Debug)]
pub enum BrowserCommand {
    /// Open a browser pane split (optionally navigate to a URL)
    Open {
        /// Optional initial URL
        url: Option<String>,
    },
    /// Navigate the active browser pane to a URL
    Navigate {
        /// Destination URL
        url: String,
    },
    /// Go back in browser history
    Back,
    /// Go forward in browser history
    Forward,
    /// Reload the current page
    Reload,
    /// Print the current URL of the active browser pane
    Url,
}

/// Run a browser subcommand.
///
/// # Errors
///
/// Returns an error if the socket call or output formatting fails.
pub fn run(cmd: BrowserCommand, socket_path: &Path, opts: OutputOpts) -> Result<()> {
    match cmd {
        BrowserCommand::Open { url } => {
            let (method, params) = open_request(url.as_deref());
            let result = socket::call(socket_path, method, params)?;
            output::print_id(&result, opts)
        }
        BrowserCommand::Navigate { url } => {
            let (method, params) = navigate_request(&url);
            let result = socket::call(socket_path, method, params)?;
            output::print_silent_or_json(&result, opts)
        }
        BrowserCommand::Back => {
            let (method, params) = back_request();
            let result = socket::call(socket_path, method, params)?;
            output::print_silent_or_json(&result, opts)
        }
        BrowserCommand::Forward => {
            let (method, params) = forward_request();
            let result = socket::call(socket_path, method, params)?;
            output::print_silent_or_json(&result, opts)
        }
        BrowserCommand::Reload => {
            let (method, params) = reload_request();
            let result = socket::call(socket_path, method, params)?;
            output::print_silent_or_json(&result, opts)
        }
        BrowserCommand::Url => {
            let (method, params) = url_request();
            let result = socket::call(socket_path, method, params)?;
            if opts.json {
                output::print_pretty_json(&result)
            } else {
                let url = result.get("url").and_then(Value::as_str).unwrap_or("");
                println!("{url}");
                Ok(())
            }
        }
    }
}

pub(crate) fn open_request(url: Option<&str>) -> (&'static str, Value) {
    (methods::BROWSER_OPEN, json!({ "url": url }))
}

pub(crate) fn navigate_request(url: &str) -> (&'static str, Value) {
    (methods::BROWSER_NAVIGATE, json!({ "url": url }))
}

pub(crate) fn back_request() -> (&'static str, Value) {
    (methods::BROWSER_BACK, json!({}))
}

pub(crate) fn forward_request() -> (&'static str, Value) {
    (methods::BROWSER_FORWARD, json!({}))
}

pub(crate) fn reload_request() -> (&'static str, Value) {
    (methods::BROWSER_RELOAD, json!({}))
}

pub(crate) fn url_request() -> (&'static str, Value) {
    (methods::BROWSER_URL, json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_builders() {
        assert_eq!(open_request(Some("https://example.com")).0, methods::BROWSER_OPEN);
        assert_eq!(navigate_request("https://x.ai").1, json!({ "url": "https://x.ai" }));
    }
}
