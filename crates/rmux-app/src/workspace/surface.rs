//! Surface — a single terminal "tab" within a `PaneNode::Leaf`.
//!
//! A leaf in the pane tree can hold one or more surfaces. Each surface owns
//! its own [`TerminalPane`] and a display title shown in the tab bar. The
//! leaf's `active_surface` index selects which surface is currently focused.
//!
//! The model mirrors cmux's tab system: every leaf is a stack of surfaces
//! (terminal sessions, browser tabs, etc.) and the user navigates between
//! them with `Ctrl+1..9`, `Ctrl+Tab`, etc.

#![allow(dead_code)]

use crate::ui::TerminalPane;

/// Maximum number of characters in a tab-bar display title.
///
/// Titles longer than this are truncated to keep the tab strip compact.
const MAX_DISPLAY_TITLE_CHARS: usize = 24;

/// A unique identifier for a surface within a workspace.
///
/// `SurfaceId` is local to a leaf — two leaves may each contain a surface
/// with `SurfaceId(1)`. The `Leaf`'s `PaneId` is the global identity used
/// for spatial layout and focus.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct SurfaceId(pub u64);

/// A single terminal surface (a "tab") within a pane leaf.
///
/// Each surface owns its [`TerminalPane`] and a human-readable title.
/// The terminal is non-optional because a surface without a backing
/// PTY has no reason to exist — the lazy-spawn state is represented
/// at the leaf level (a leaf with zero surfaces is the "uninitialized"
/// state, distinct from a leaf with one or more surfaces).
pub struct Surface {
    /// Unique id of this surface within its containing leaf.
    pub id: SurfaceId,
    /// Display title (e.g. "Terminal 1", or a custom name set via
    /// `Cmd+R` rename).
    pub title: String,
    /// The terminal backing this surface.
    pub terminal: TerminalPane,
}

impl Surface {
    /// Create a new surface with the given id, title, and terminal.
    pub fn new(id: SurfaceId, title: impl Into<String>, terminal: TerminalPane) -> Self {
        Self { id, title: title.into(), terminal }
    }

    /// Title for the tab bar, truncated to [`MAX_DISPLAY_TITLE_CHARS`]
    /// characters without splitting a multi-byte codepoint.
    ///
    /// Slicing on byte indices would panic on `&self.title[..n]` when `n`
    /// lands inside a UTF-8 codepoint, so we use `char_indices()` to find
    /// the safe byte boundary at the `n`-th char.
    pub fn display_title(&self) -> &str {
        if self.title.chars().count() <= MAX_DISPLAY_TITLE_CHARS {
            return &self.title;
        }
        self.title
            .char_indices()
            .nth(MAX_DISPLAY_TITLE_CHARS)
            .map(|(byte_idx, _)| &self.title[..byte_idx])
            .unwrap_or(&self.title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_surface(id: u64, title: &str) -> Surface {
        let term = TerminalPane::spawn(1, 1, 14.0).expect("dummy terminal spawn");
        Surface::new(SurfaceId(id), title, term)
    }

    #[test]
    fn test_surface_creation() {
        let s = make_surface(1, "Terminal 1");
        assert_eq!(s.id, SurfaceId(1));
        assert_eq!(s.title, "Terminal 1");
    }

    #[test]
    fn test_surface_display_title_truncates_long_titles() {
        let long = "a".repeat(30);
        let s = make_surface(1, &long);
        let display = s.display_title();
        assert_eq!(display.chars().count(), 24);
        assert!(display.chars().all(|c| c == 'a'));
    }

    #[test]
    fn test_surface_display_title_returns_empty_for_empty_title() {
        let s = make_surface(1, "");
        assert_eq!(s.display_title(), "");
    }

    #[test]
    fn test_surface_id_uniqueness() {
        let a = make_surface(1, "A");
        let b = make_surface(2, "B");
        assert_ne!(a.id, b.id);
        let a2 = make_surface(1, "A again");
        assert_eq!(a.id, a2.id);
    }
}
