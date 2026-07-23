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
        /// Target workspace id (defaults to $RMUX_WORKSPACE_ID)
        #[arg(long)]
        workspace: Option<u64>,
        /// Target pane id (defaults to $RMUX_PANE_ID)
        #[arg(long)]
        pane: Option<u64>,
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
    /// Install or handle agent lifecycle hooks (Claude Code, OpenCode)
    Hooks {
        #[command(subcommand)]
        action: HooksCommand,
    },
}

/// `rmux-cli hooks` subcommands.
#[derive(Subcommand, Debug)]
enum HooksCommand {
    /// Install agent hook configs (detects binaries on PATH)
    Setup {
        /// Agent to install: `claude`, `opencode`, or `all` (default)
        #[arg(long, short = 'a')]
        agent: Option<String>,
        /// Install even if the agent binary is not on PATH
        #[arg(long)]
        force: bool,
    },
    /// Remove rmux-owned agent hook configs
    Uninstall {
        /// Agent to uninstall: `claude`, `opencode`, or `all` (default)
        #[arg(long, short = 'a')]
        agent: Option<String>,
    },
    /// Handle a Claude Code hook event (reads JSON from stdin)
    Claude {
        /// Event: session-start, prompt-submit, stop, notification, push-notification, session-end
        event: String,
    },
    /// Handle an OpenCode plugin event (reads JSON from stdin)
    Opencode {
        /// Event: session-start, stop, notification, status
        event: String,
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
        Command::Notify { title, subtitle, body, workspace, pane } => commands::notify(
            &socket_path,
            &title,
            subtitle.as_deref(),
            body.as_deref(),
            workspace,
            pane,
        ),
        Command::NewWorkspace { name } => commands::new_workspace(&socket_path, name.as_deref()),
        Command::ListWorkspaces { json } => commands::list_workspaces(&socket_path, json),
        Command::NewSplit { direction } => commands::new_split(&socket_path, direction.as_str()),
        Command::Send { text } => commands::send(&socket_path, &text),
        Command::Hooks { action } => match action {
            HooksCommand::Setup { agent, force } => commands::hooks_setup(agent.as_deref(), force),
            HooksCommand::Uninstall { agent } => commands::hooks_uninstall(agent.as_deref()),
            HooksCommand::Claude { event } => commands::hooks_claude(&socket_path, &event),
            HooksCommand::Opencode { event } => commands::hooks_opencode(&socket_path, &event),
        },
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

    #[test]
    fn hooks_setup_parses() {
        let cli = Cli::parse_from(["rmux-cli", "hooks", "setup", "--agent", "claude", "--force"]);
        match cli.command {
            Command::Hooks { action: HooksCommand::Setup { agent, force } } => {
                assert_eq!(agent.as_deref(), Some("claude"));
                assert!(force);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn hooks_claude_stop_parses() {
        let cli = Cli::parse_from(["rmux-cli", "hooks", "claude", "stop"]);
        match cli.command {
            Command::Hooks { action: HooksCommand::Claude { event } } => {
                assert_eq!(event, "stop");
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
