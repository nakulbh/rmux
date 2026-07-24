#![forbid(unsafe_code)]
//! CLI tool for controlling a running rmux instance.
//!
//! Communicates with the rmux socket server over a Unix domain socket
//! using a newline-delimited JSON protocol. Each subcommand performs a
//! single request/response roundtrip.
//!
//! # Exit codes
//!
//! - `0` — success
//! - `1` — server-side or local error
//! - `2` — cannot connect to the rmux socket

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

use rmux_cli::{commands, socket};

/// Command-line arguments for `rmux-cli`.
#[derive(Parser, Debug)]
#[command(
    name = "rmux-cli",
    version,
    about = "Control a running rmux instance over its socket API",
    long_about = None
)]
struct Cli {
    /// Path to the rmux control socket (takes precedence over $RMUX_SOCKET_PATH)
    #[arg(long, global = true, value_name = "PATH")]
    socket: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

/// Subcommands supported by `rmux-cli`.
#[derive(Subcommand, Debug)]
enum Command {
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
        direction: SplitDirection,
    },
    /// Send text to the active pane
    Send {
        /// Text to send. Backslash escapes \n, \r, \t, \e (escape) and \\
        /// are interpreted; unknown escapes pass through unchanged.
        text: String,
    },
    /// Open a browser pane split (optional URL)
    BrowserOpen {
        /// URL to load after open
        url: Option<String>,
    },
    /// Navigate the browser to a URL
    BrowserNav {
        /// Target URL
        url: String,
        /// Browser pane id (default: active / first browser)
        #[arg(long)]
        pane: Option<u64>,
    },
    /// Print the browser's current URL and title
    BrowserUrl {
        #[arg(long)]
        pane: Option<u64>,
    },
    /// Evaluate a JavaScript expression in the page
    BrowserEval {
        /// JS expression (e.g. `document.title`)
        script: String,
        #[arg(long)]
        pane: Option<u64>,
    },
    /// Click an element by CSS selector
    BrowserClick {
        selector: String,
        #[arg(long)]
        pane: Option<u64>,
    },
    /// Fill an input by CSS selector
    BrowserFill {
        selector: String,
        value: String,
        #[arg(long)]
        pane: Option<u64>,
    },
    /// Dump a DOM / accessibility snapshot as JSON
    BrowserSnapshot {
        #[arg(long)]
        pane: Option<u64>,
    },
    /// Capture a PNG screenshot of the browser content (Chromium OSR)
    BrowserScreenshot {
        /// Output PNG path (default: temp file)
        #[arg(long, short = 'o')]
        out: Option<String>,
        #[arg(long)]
        pane: Option<u64>,
    },
}

/// Split direction accepted by `new-split`.
#[derive(ValueEnum, Clone, Copy, Debug)]
enum SplitDirection {
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

/// Dispatch the parsed CLI to the matching command implementation.
fn run(cli: Cli) -> anyhow::Result<()> {
    let socket_path = socket::effective_socket_path(cli.socket);
    match cli.command {
        Command::Ping => commands::ping(&socket_path),
        Command::Capabilities => commands::capabilities(&socket_path),
        Command::Notify { title, subtitle, body } => {
            commands::notify(&socket_path, &title, subtitle.as_deref(), body.as_deref())
        }
        Command::NewWorkspace { name } => commands::new_workspace(&socket_path, name.as_deref()),
        Command::ListWorkspaces { json } => commands::list_workspaces(&socket_path, json),
        Command::NewSplit { direction } => commands::new_split(&socket_path, direction.as_str()),
        Command::Send { text } => commands::send(&socket_path, &text),
        Command::BrowserOpen { url } => commands::browser_open(&socket_path, url.as_deref()),
        Command::BrowserNav { url, pane } => commands::browser_nav(&socket_path, &url, pane),
        Command::BrowserUrl { pane } => commands::browser_url(&socket_path, pane),
        Command::BrowserEval { script, pane } => {
            commands::browser_eval(&socket_path, &script, pane)
        }
        Command::BrowserClick { selector, pane } => {
            commands::browser_click(&socket_path, &selector, pane)
        }
        Command::BrowserFill { selector, value, pane } => {
            commands::browser_fill(&socket_path, &selector, &value, pane)
        }
        Command::BrowserSnapshot { pane } => commands::browser_snapshot(&socket_path, pane),
        Command::BrowserScreenshot { out, pane } => {
            commands::browser_screenshot(&socket_path, out.as_deref(), pane)
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            if let Some(connect) = err.downcast_ref::<socket::ConnectError>() {
                eprintln!(
                    "error: cannot connect to rmux at {} — is rmux running?",
                    connect.path.display()
                );
                ExitCode::from(2)
            } else if let Some(server) = err.downcast_ref::<socket::ServerError>() {
                eprintln!("error [{}]: {}", server.code, server.message);
                ExitCode::from(1)
            } else {
                eprintln!("error: {err:#}");
                ExitCode::from(1)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_all_subcommands() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn socket_flag_is_global() {
        let cli = Cli::parse_from(["rmux-cli", "ping", "--socket", "/tmp/custom.sock"]);
        assert_eq!(cli.socket, Some(PathBuf::from("/tmp/custom.sock")));
    }

    #[test]
    fn split_direction_maps_to_wire_strings() {
        assert_eq!(SplitDirection::Right.as_str(), "right");
        assert_eq!(SplitDirection::Down.as_str(), "down");
    }
}
