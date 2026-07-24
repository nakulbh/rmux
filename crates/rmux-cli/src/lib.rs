#![forbid(unsafe_code)]
//! Library surface of the `rmux-cli` binary.
//!
//! Exposes the socket client and command implementations so that
//! integration tests can exercise them directly without spawning
//! the binary.
//!
//! # Modules
//!
//! - [`socket`] — socket path resolution and the blocking line-protocol client
//! - [`commands`] — one function per CLI subcommand
//! - [`hooks`] — agent hook installers and event handlers
//! - [`tmux_compat`] — tmux shim translation for agent teams
//! - [`launchers`] — `claude-teams` and future integration launchers

pub mod commands;
pub mod hooks;
pub mod launchers;
pub mod socket;
pub mod tmux_compat;
