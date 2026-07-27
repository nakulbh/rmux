//! `rmux-cli events` — subscribe to the live app event stream.

use std::path::Path;

use anyhow::Result;
use clap::Subcommand;

use crate::output::OutputOpts;
use crate::socket;

/// Event-stream subcommands.
#[derive(Subcommand, Debug)]
pub enum EventsCommand {
    /// Stream NDJSON events until Ctrl-C or the app disconnects
    Stream,
}

/// Run an events subcommand.
///
/// # Errors
///
/// Returns an error if the socket cannot be connected or the stream fails.
pub fn run(cmd: EventsCommand, socket_path: &Path, _opts: OutputOpts) -> Result<()> {
    match cmd {
        EventsCommand::Stream => socket::stream_events(socket_path),
    }
}
