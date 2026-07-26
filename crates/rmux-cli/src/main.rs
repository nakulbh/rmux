#![forbid(unsafe_code)]
//! CLI tool for controlling a running rmux instance.
//!
//! Communicates with the rmux socket server over a Unix domain socket
//! using a newline-delimited JSON protocol. Commands are organized by
//! domain (`system`, `workspace`, `surface`, …) so every part of the
//! application is scriptable. Use `call` to invoke any method by name.
//!
//! # Exit codes
//!
//! - `0` — success
//! - `1` — server-side or local error
//! - `2` — cannot connect to the rmux socket

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use rmux_cli::commands::{self, Command};
use rmux_cli::output::OutputOpts;
use rmux_cli::socket;

/// Command-line arguments for `rmux-cli`.
#[derive(Parser, Debug)]
#[command(
    name = "rmux-cli",
    version,
    about = "Control a running rmux instance over its socket API",
    long_about = "Scriptable control plane for the rmux terminal multiplexer.\n\n\
                  Domains: system, workspace, surface, notification, sidebar, browser, app, events.\n\
                  Escape hatch: call <method> [params_json]\n\
                  Flat aliases (ping, notify, …) remain for Phase 3 script compatibility."
)]
struct Cli {
    /// Path to the rmux control socket (takes precedence over $RMUX_SOCKET_PATH)
    #[arg(long, global = true, value_name = "PATH")]
    socket: Option<PathBuf>,

    /// Print machine-readable JSON instead of human tables
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

/// Dispatch the parsed CLI to the matching command implementation.
fn run(cli: Cli) -> anyhow::Result<()> {
    let socket_path = socket::effective_socket_path(cli.socket);
    let opts = OutputOpts { json: cli.json };
    commands::run(cli.command, &socket_path, opts)
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
    use clap::CommandFactory;

    #[test]
    fn cli_parses_all_subcommands() {
        Cli::command().debug_assert();
    }

    #[test]
    fn socket_flag_is_global() {
        let cli = Cli::parse_from(["rmux-cli", "system", "ping", "--socket", "/tmp/custom.sock"]);
        assert_eq!(cli.socket, Some(PathBuf::from("/tmp/custom.sock")));
    }

    #[test]
    fn hierarchical_and_alias_commands_parse() {
        let _ = Cli::parse_from(["rmux-cli", "workspace", "list"]);
        let _ = Cli::parse_from(["rmux-cli", "surface", "split", "right"]);
        let _ = Cli::parse_from(["rmux-cli", "notification", "create", "--title", "t"]);
        let _ = Cli::parse_from(["rmux-cli", "call", "system.ping", "{}"]);
        let _ = Cli::parse_from(["rmux-cli", "ping"]);
        let _ = Cli::parse_from(["rmux-cli", "list-workspaces", "--json"]);
        let _ = Cli::parse_from(["rmux-cli", "browser", "open", "https://example.com"]);
        let _ = Cli::parse_from(["rmux-cli", "app", "theme", "dracula"]);
        let _ = Cli::parse_from(["rmux-cli", "events", "stream"]);
    }

    #[test]
    fn global_json_flag_parses() {
        let cli = Cli::parse_from(["rmux-cli", "--json", "workspace", "list"]);
        assert!(cli.json);
    }
}
