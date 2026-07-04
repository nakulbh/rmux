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

pub mod commands;
pub mod socket;
