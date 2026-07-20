//! Gate that blocks PTY keystrokes while a true text field owns input.
//!
//! egui's [`Context::wants_keyboard_input`] is **not** suitable for this: in
//! egui 0.31 it returns `memory.focused().is_some()`, so any focused Button
//! (e.g. after Tab focus navigation) silences the terminal. That made Tab /
//! Backspace feel like they needed multiple presses — the first keystroke
//! often moved focus to chrome, then every following key was dropped.
//!
//! Text sinks (sidebar rename, find bar, browser URL) set a per-frame flag
//! instead. The active terminal only yields when that flag is set.

use egui::{Context, Id};

const FLAG_ID: &str = "rmux.pty_text_sink";

/// Call once at the start of each frame before any UI is drawn.
pub fn begin_frame(ctx: &Context) {
    ctx.data_mut(|d| d.insert_temp(Id::new(FLAG_ID), false));
}

/// Mark that a TextEdit / IME field currently owns typing (rename, find, URL).
pub fn mark_active(ctx: &Context) {
    ctx.data_mut(|d| d.insert_temp(Id::new(FLAG_ID), true));
}

/// True when a text field claimed the keyboard this frame.
pub fn is_active(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(Id::new(FLAG_ID)).unwrap_or(false))
}
