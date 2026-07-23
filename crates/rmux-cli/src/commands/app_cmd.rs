//! `rmux-cli app` — application-wide settings (font size, theme).

use std::path::Path;

use anyhow::Result;
use clap::Subcommand;
use rmux_api::methods;
use serde_json::{Value, json};

use crate::output::{self, OutputOpts};
use crate::socket;

/// Application-domain subcommands.
#[derive(Subcommand, Debug)]
pub enum AppCommand {
    /// Change terminal font size by a delta, or reset to default
    FontSize {
        /// Delta in points (e.g. 1.0 or -1.0). Use with --reset to restore default.
        #[arg(allow_hyphen_values = true)]
        delta: Option<f32>,
        /// Reset font size to the application default
        #[arg(long)]
        reset: bool,
    },
    /// Set the terminal color theme
    Theme {
        /// Theme name (e.g. onedark, dracula, tokyo-night)
        name: String,
    },
}

/// Run an app subcommand.
///
/// # Errors
///
/// Returns an error if the socket call or output formatting fails.
pub fn run(cmd: AppCommand, socket_path: &Path, opts: OutputOpts) -> Result<()> {
    match cmd {
        AppCommand::FontSize { delta, reset } => {
            let (method, params) = set_font_size_request(delta, reset);
            let result = socket::call(socket_path, method, params)?;
            if opts.json {
                output::print_pretty_json(&result)
            } else if let Some(size) = result.get("font_size") {
                println!("{size}");
                Ok(())
            } else {
                output::print_pretty_json(&result)
            }
        }
        AppCommand::Theme { name } => {
            let (method, params) = set_theme_request(&name);
            let result = socket::call(socket_path, method, params)?;
            output::print_silent_or_json(&result, opts)
        }
    }
}

pub(crate) fn set_font_size_request(delta: Option<f32>, reset: bool) -> (&'static str, Value) {
    (methods::APP_SET_FONT_SIZE, json!({ "delta": delta, "reset": reset }))
}

pub(crate) fn set_theme_request(theme: &str) -> (&'static str, Value) {
    (methods::APP_SET_THEME, json!({ "theme": theme }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_builders() {
        assert_eq!(
            set_font_size_request(Some(1.0), false),
            (methods::APP_SET_FONT_SIZE, json!({ "delta": 1.0, "reset": false }))
        );
        assert_eq!(set_font_size_request(None, true).1, json!({ "delta": null, "reset": true }));
        assert_eq!(
            set_theme_request("dracula"),
            (methods::APP_SET_THEME, json!({ "theme": "dracula" }))
        );
    }
}
