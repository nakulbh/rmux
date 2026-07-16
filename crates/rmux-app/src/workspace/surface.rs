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

    /// Title for the tab bar (cmux-style).
    ///
    /// Prefers the shell's current path (`~/…/project` or `user@host` at
    /// home). Custom renames (anything that is not the default
    /// `"Terminal N"` placeholder) win over the auto path. Truncated to
    /// [`MAX_DISPLAY_TITLE_CHARS`] without splitting a multi-byte codepoint.
    pub fn display_title(&self) -> String {
        let raw =
            if self.is_default_title() { self.terminal.tab_label() } else { self.title.clone() };
        truncate_title(&raw, MAX_DISPLAY_TITLE_CHARS)
    }

    /// True when `title` is still the auto-generated `Terminal N` label
    /// (or empty), so path-based labels should replace it.
    fn is_default_title(&self) -> bool {
        self.title.is_empty()
            || self
                .title
                .strip_prefix("Terminal ")
                .is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit()))
    }
}

/// Truncate `s` to at most `max` characters on a char boundary.
fn truncate_title(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let end = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
    s[..end].to_owned()
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
    fn test_surface_display_title_truncates_long_custom_titles() {
        let long = "a".repeat(30);
        let s = make_surface(1, &long);
        let display = s.display_title();
        assert_eq!(display.chars().count(), 24);
        assert!(display.chars().all(|c| c == 'a'));
    }

    #[test]
    fn test_surface_default_title_uses_path_or_user_host() {
        // Empty / "Terminal N" → auto label (path or user@host), not empty.
        let s = make_surface(1, "Terminal 1");
        let display = s.display_title();
        assert!(!display.is_empty());
    }

    #[test]
    fn test_truncate_title_on_char_boundary() {
        assert_eq!(truncate_title("hello", 10), "hello");
        assert_eq!(truncate_title("abcdefghij", 5).chars().count(), 5);
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
