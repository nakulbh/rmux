//! Terminal state module.
//!
//! Wraps `alacritty_terminal::Term` and provides a clean query API
//! for the renderer. Manages grid state, scrollback, and cursor position.
//!
//! Will be fully implemented in Phase 1.

/// Terminal state — will be implemented in Phase 1.
///
/// This module is a placeholder. Full implementation will include:
/// - `TermState::new()` — create terminal with dimensions
/// - `TermState::feed_bytes()` — feed PTY output through VTE parser
/// - `TermState::grid_snapshot()` — captured grid for rendering
/// - `TermState::cursor()` — current cursor position
/// - `TermState::scroll()` — scroll the viewport
/// - `TermState::selection()` — get selected text
pub struct TermState;

impl TermState {
    /// Create a new, uninitialized terminal state.
    ///
    /// This is a placeholder constructor for Phase 0.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_term_state_placeholder_exists() {
        let state = TermState::new();
        let _ = state;
    }
}
