//! Terminal emulation library for rmux.
//!
//! Wraps `alacritty_terminal` (VTE parser + grid state) and `portable-pty`
//! (cross-platform PTY management) into a clean API for the UI layer.
//!
//! # Modules
//!
//! - `backend` — PTY lifecycle management (spawn, read, write, resize)
//! - `state` — Terminal state wrapper around `alacritty_terminal::Term`
//! - `renderer` — Convert terminal grid cells into egui paint commands
//! - `input` — Map egui keyboard/mouse events to terminal escape sequences

pub mod backend;
pub mod input;
pub mod renderer;
pub mod state;
