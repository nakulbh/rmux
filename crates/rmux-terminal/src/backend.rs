//! PTY backend module.
//!
//! Manages the pseudo-terminal (PTY) lifecycle: spawning a shell,
//! reading output, writing input, and handling resize events.
//!
//! Built on `portable-pty` for cross-platform PTY support.

/// PTY backend — will be implemented in Phase 1.
///
/// This module is a placeholder. Full implementation will include:
/// - `PtyBackend::spawn()` — spawn a shell process
/// - `PtyBackend::write()` — send input to the PTY
/// - `PtyBackend::resize()` — update terminal dimensions
/// - `PtyBackend::is_alive()` — check if child process is running
/// - `PtyBackend::take_reader()` — extract reader for async I/O
pub struct PtyBackend;

impl PtyBackend {
    /// Create a new, uninitialized PTY backend.
    ///
    /// This is a placeholder constructor for Phase 0. In Phase 1,
    /// this will be replaced with `PtyBackend::spawn()`.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_backend_placeholder_exists() {
        let backend = PtyBackend::new();
        let _ = backend; // Verify it compiles and can be constructed
    }
}
