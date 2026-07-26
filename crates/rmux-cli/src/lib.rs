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
//! - [`commands`] — hierarchical domain commands + back-compat aliases
//! - [`output`] — shared stdout formatting (`--json` / tables)
//! - [`util`] — escape interpretation and id extraction

pub mod commands;
pub mod output;
pub mod socket;
pub mod util;
