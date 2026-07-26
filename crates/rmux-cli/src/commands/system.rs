//! `rmux-cli system` — health, capabilities, identity.

use std::path::Path;

use anyhow::Result;
use clap::Subcommand;
use rmux_api::methods;
use serde_json::{Value, json};

use crate::output::{self, OutputOpts};
use crate::socket;

/// System-domain subcommands.
#[derive(Subcommand, Debug)]
pub enum SystemCommand {
    /// Check that rmux is reachable (prints "pong")
    Ping,
    /// Print the server capabilities as pretty JSON
    Capabilities,
    /// Print app name, version, and process id
    Identify,
}

/// Run a system subcommand.
///
/// # Errors
///
/// Returns an error if the socket call or output formatting fails.
pub fn run(cmd: SystemCommand, socket_path: &Path, opts: OutputOpts) -> Result<()> {
    match cmd {
        SystemCommand::Ping => {
            let (method, params) = ping_request();
            let result = socket::call(socket_path, method, params)?;
            if opts.json {
                output::print_pretty_json(&result)?;
            } else {
                println!("pong");
            }
            Ok(())
        }
        SystemCommand::Capabilities => {
            let (method, params) = capabilities_request();
            let result = socket::call(socket_path, method, params)?;
            output::print_pretty_json(&result)
        }
        SystemCommand::Identify => {
            let (method, params) = identify_request();
            let result = socket::call(socket_path, method, params)?;
            if opts.json {
                output::print_pretty_json(&result)
            } else {
                let app = result.get("app").and_then(Value::as_str).unwrap_or("rmux");
                let version = result.get("version").and_then(Value::as_str).unwrap_or("?");
                let pid = result.get("pid").map(|v| v.to_string()).unwrap_or_else(|| "?".into());
                println!("{app} {version} (pid {pid})");
                Ok(())
            }
        }
    }
}

pub(crate) fn ping_request() -> (&'static str, Value) {
    (methods::SYSTEM_PING, json!({}))
}

pub(crate) fn capabilities_request() -> (&'static str, Value) {
    (methods::SYSTEM_CAPABILITIES, json!({}))
}

pub(crate) fn identify_request() -> (&'static str, Value) {
    (methods::SYSTEM_IDENTIFY, json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameterless_requests_use_expected_methods() {
        assert_eq!(ping_request().0, methods::SYSTEM_PING);
        assert_eq!(capabilities_request().0, methods::SYSTEM_CAPABILITIES);
        assert_eq!(identify_request().0, methods::SYSTEM_IDENTIFY);
    }
}
