#![forbid(unsafe_code)]
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
//! - `osc` — Scan PTY output for notification OSC sequences (9/99/777)

mod backend;
mod input;
mod osc;
mod renderer;
mod state;
mod theme;

pub use backend::{PtyBackend, PtyError, PtyResult};
pub use input::InputMapper;
pub use osc::{OscKind, OscNotification, OscScanner};
pub use renderer::TerminalRenderer;
pub use state::{GridCell, GridSnapshot, TermState};
pub use theme::{NamedTheme, TerminalTheme};

// Re-export cursor shape from alacritty_terminal for convenience
pub use alacritty_terminal::vte::ansi::CursorShape;
