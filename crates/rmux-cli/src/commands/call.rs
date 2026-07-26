//! `rmux-cli call` — invoke any socket method with raw JSON params.

use std::path::Path;

use anyhow::{Context, Result};
use clap::Args;
use serde_json::Value;

use crate::output::{self, OutputOpts};
use crate::socket;

/// Escape-hatch invocation of an arbitrary socket method.
#[derive(Args, Debug)]
pub struct CallCommand {
    /// Method name (e.g. `system.ping`, `workspace.create`)
    method: String,
    /// JSON object of parameters (default: `{}`)
    #[arg(default_value = "{}")]
    params: String,
}

/// Run a raw method call and print the result as pretty JSON.
///
/// # Errors
///
/// Returns an error if params are not valid JSON or the socket call fails.
pub fn run(cmd: CallCommand, socket_path: &Path, _opts: OutputOpts) -> Result<()> {
    let params: Value =
        serde_json::from_str(&cmd.params).context("params must be a valid JSON value")?;
    let result = socket::call(socket_path, &cmd.method, params)?;
    output::print_pretty_json(&result)
}
