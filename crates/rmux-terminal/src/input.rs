//! Input mapping module.
//!
//! Maps egui keyboard and mouse events to terminal escape sequences
//! that can be written to the PTY. Supports modifier keys,
//! bracket paste mode, and mouse reporting.
//!
//! Will be fully implemented in Phase 1.

/// Input mapper — will be implemented in Phase 1.
///
/// Translates egui events into terminal escape sequences:
/// - `map_key()` — keyboard events → terminal bytes
/// - `map_mouse()` — mouse events → terminal bytes (for mouse reporting)
/// - `paste()` — wrap text in bracket paste sequences
pub struct InputMapper;

impl InputMapper {
    /// Create a new, uninitialized input mapper.
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
    fn test_input_mapper_placeholder_exists() {
        let mapper = InputMapper::new();
        let _ = mapper;
    }
}
