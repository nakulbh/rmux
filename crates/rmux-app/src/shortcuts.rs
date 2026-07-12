//! Global keyboard shortcut handling for [`RmuxApp`].
//!
//! Split out of `app.rs` to keep both modules focused: this module only
//! translates key chords into workspace/pane operations.

use std::collections::HashMap;

use egui::{Key, Modifiers};

/// An app-level action that can be triggered by a keyboard shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    /// Quit the application.
    Quit,
    /// Increase terminal font size.
    FontSizeUp,
    /// Decrease terminal font size.
    FontSizeDown,
    /// Reset terminal font size to default.
    FontSizeReset,
    /// Copy selected text from the active terminal.
    Copy,
    /// Open the find bar.
    Find,
    /// Find next match.
    FindNext,
    /// Find previous match.
    FindPrev,
    /// Use current selection as find query.
    UseSelectionForFind,
    /// Clear terminal scrollback.
    ClearScrollback,
    /// Clear screen (send Ctrl+L).
    ClearScreen,
    /// Toggle sidebar visibility.
    ToggleSidebar,
    /// Toggle notification panel visibility.
    ToggleNotifications,
    /// Create a new workspace.
    NewWorkspace,
    /// Split active pane to the right.
    SplitRight,
    /// Split active pane downward.
    SplitDown,
    /// Close the active pane.
    ClosePane,
    /// Open a browser split.
    OpenBrowserSplit,
    /// Focus the browser URL bar.
    FocusBrowserUrlBar,
    /// Reload the browser page.
    ReloadBrowser,
    /// Switch to workspace by index (0-based).
    SwitchWorkspace(usize),
    /// Close the active workspace.
    CloseWorkspace,
    /// Rename the active workspace.
    RenameWorkspace,
    /// Toggle pane zoom (maximize/restore).
    ToggleZoom,
    /// Equalize all split sizes.
    EqualizeSplits,
    /// Switch to previous workspace.
    PrevWorkspace,
    /// Switch to next workspace.
    NextWorkspace,
    /// Focus pane to the left.
    FocusLeft,
    /// Focus pane to the right.
    FocusRight,
    /// Focus pane above.
    FocusUp,
    /// Focus pane below.
    FocusDown,
}

/// Registry that maps keyboard chords to [`ShortcutAction`]s.
///
/// Built once at application startup and reused for every key event.
#[derive(Debug, Clone)]
pub struct ShortcutRegistry {
    /// Exact-match lookups: (modifiers, key) → action.
    exact: HashMap<(Modifiers, Key), ShortcutAction>,
}

impl ShortcutRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self { exact: HashMap::new() }
    }

    /// Register a shortcut.
    pub fn register(&mut self, modifiers: Modifiers, key: Key, action: ShortcutAction) {
        self.exact.insert((modifiers, key), action);
    }

    /// Look up an action for the given key chord.
    pub fn lookup(&self, modifiers: Modifiers, key: Key) -> Option<ShortcutAction> {
        self.exact.get(&(modifiers, key)).copied()
    }
}

impl Default for ShortcutRegistry {
    /// Populate the registry with all built-in shortcuts.
    ///
    /// Platform-specific modifier handling is applied at *lookup* time
    /// (see `RmuxApp::handle_keyboard_shortcuts`), so the registry stores
    /// shortcuts with the **canonical** modifier (Command on macOS,
    /// Control on Linux/Windows).
    fn default() -> Self {
        let mut reg = Self::new();

        // --- Always-active shortcuts ---

        // Cmd/Ctrl+Q → Quit
        reg.register(cmd_ctrl(), Key::Q, ShortcutAction::Quit);

        // Cmd/Ctrl+Plus or Equals → FontSizeUp
        reg.register(cmd_ctrl(), Key::Plus, ShortcutAction::FontSizeUp);
        reg.register(cmd_ctrl(), Key::Equals, ShortcutAction::FontSizeUp);

        // Cmd/Ctrl+Minus → FontSizeDown
        reg.register(cmd_ctrl(), Key::Minus, ShortcutAction::FontSizeDown);

        // Cmd/Ctrl+0 → FontSizeReset
        reg.register(cmd_ctrl(), Key::Num0, ShortcutAction::FontSizeReset);

        // Cmd/Ctrl+C → Copy
        reg.register(cmd_ctrl(), Key::C, ShortcutAction::Copy);

        // Escape → handled conditionally in dispatcher (CloseFind)
        reg.register(Modifiers::NONE, Key::Escape, ShortcutAction::Find);

        // Enter → handled conditionally in dispatcher (FindNext when find visible)
        reg.register(Modifiers::NONE, Key::Enter, ShortcutAction::FindNext);

        // Cmd/Ctrl+F → Find
        reg.register(cmd_ctrl(), Key::F, ShortcutAction::Find);

        // Cmd/Ctrl+G → FindNext (when find visible)
        reg.register(cmd_ctrl(), Key::G, ShortcutAction::FindNext);

        // Alt+Cmd/Ctrl+G → FindPrev (when find visible)
        reg.register(cmd_alt(), Key::G, ShortcutAction::FindPrev);

        // Cmd/Ctrl+E → UseSelectionForFind
        reg.register(cmd_ctrl(), Key::E, ShortcutAction::UseSelectionForFind);

        // Cmd/Ctrl+K → ClearScrollback
        reg.register(cmd_ctrl(), Key::K, ShortcutAction::ClearScrollback);

        // Cmd/Ctrl+Shift+K → ClearScreen
        reg.register(cmd_ctrl_shift(), Key::K, ShortcutAction::ClearScreen);

        // --- Focus-dependent shortcuts ---

        // Cmd/Ctrl+B → ToggleSidebar
        reg.register(cmd_ctrl(), Key::B, ShortcutAction::ToggleSidebar);

        // Cmd/Ctrl+I → ToggleNotifications
        reg.register(cmd_ctrl(), Key::I, ShortcutAction::ToggleNotifications);

        // Cmd/Ctrl+N → NewWorkspace
        reg.register(cmd_ctrl(), Key::N, ShortcutAction::NewWorkspace);

        // Cmd/Ctrl+D → SplitRight
        reg.register(cmd_ctrl(), Key::D, ShortcutAction::SplitRight);

        // Cmd/Ctrl+Shift+D → SplitDown
        reg.register(cmd_ctrl_shift(), Key::D, ShortcutAction::SplitDown);

        // Cmd/Ctrl+W → ClosePane
        reg.register(cmd_ctrl(), Key::W, ShortcutAction::ClosePane);

        // Cmd/Ctrl+Shift+W → CloseWorkspace
        reg.register(cmd_ctrl_shift(), Key::W, ShortcutAction::CloseWorkspace);

        // Cmd/Ctrl+Shift+L → OpenBrowserSplit
        reg.register(cmd_ctrl_shift(), Key::L, ShortcutAction::OpenBrowserSplit);

        // Cmd/Ctrl+L → FocusBrowserUrlBar (when browser active)
        reg.register(cmd_ctrl(), Key::L, ShortcutAction::FocusBrowserUrlBar);

        // Cmd/Ctrl+R → ReloadBrowser (when browser active)
        reg.register(cmd_ctrl(), Key::R, ShortcutAction::ReloadBrowser);

        // Cmd/Ctrl+1..9 → SwitchWorkspace(0..8)
        for (i, key) in [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ]
        .iter()
        .enumerate()
        {
            reg.register(cmd_ctrl(), *key, ShortcutAction::SwitchWorkspace(i));
        }

        // Cmd/Ctrl+Shift+R → RenameWorkspace
        reg.register(cmd_ctrl_shift(), Key::R, ShortcutAction::RenameWorkspace);

        // Cmd/Ctrl+Shift+Enter → ToggleZoom
        reg.register(cmd_ctrl_shift(), Key::Enter, ShortcutAction::ToggleZoom);

        // Cmd/Ctrl+Shift+Equals → EqualizeSplits
        reg.register(cmd_ctrl_shift(), Key::Equals, ShortcutAction::EqualizeSplits);

        // Cmd/Ctrl+Shift+[ → PrevWorkspace
        reg.register(cmd_ctrl_shift(), Key::OpenBracket, ShortcutAction::PrevWorkspace);

        // Cmd/Ctrl+Shift+] → NextWorkspace
        reg.register(cmd_ctrl_shift(), Key::CloseBracket, ShortcutAction::NextWorkspace);

        // Cmd/Ctrl+ArrowLeft → FocusLeft
        reg.register(cmd_ctrl(), Key::ArrowLeft, ShortcutAction::FocusLeft);

        // Cmd/Ctrl+ArrowUp → FocusUp
        reg.register(cmd_ctrl(), Key::ArrowUp, ShortcutAction::FocusUp);

        // Cmd/Ctrl+Opt/Alt+ArrowRight → FocusRight
        reg.register(cmd_ctrl_alt(), Key::ArrowRight, ShortcutAction::FocusRight);

        // Cmd/Ctrl+Opt/Alt+ArrowDown → FocusDown
        reg.register(cmd_ctrl_alt(), Key::ArrowDown, ShortcutAction::FocusDown);

        reg
    }
}

/// Return the canonical app-shortcut modifier for the current platform.
///
/// On macOS this is **Command**; on Linux/Windows it is **Control**.
pub(crate) fn cmd_ctrl() -> Modifiers {
    if cfg!(target_os = "macos") { Modifiers::COMMAND } else { Modifiers::CTRL }
}

/// `cmd_ctrl()` plus Shift.
pub(crate) fn cmd_ctrl_shift() -> Modifiers {
    cmd_ctrl() | Modifiers::SHIFT
}

/// `cmd_ctrl()` plus Alt/Option.
pub(crate) fn cmd_ctrl_alt() -> Modifiers {
    cmd_ctrl() | Modifiers::ALT
}

/// `cmd_ctrl()` plus Alt/Option (no shift).
pub(crate) fn cmd_alt() -> Modifiers {
    cmd_ctrl() | Modifiers::ALT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_default_includes_quit() {
        let reg = ShortcutRegistry::default();
        let mods = if cfg!(target_os = "macos") { Modifiers::COMMAND } else { Modifiers::CTRL };
        assert_eq!(reg.lookup(mods, Key::Q), Some(ShortcutAction::Quit));
    }

    #[test]
    fn test_registry_lookup_unknown() {
        let reg = ShortcutRegistry::default();
        assert_eq!(reg.lookup(Modifiers::NONE, Key::A), None);
    }

    #[test]
    fn test_registry_switch_workspace() {
        let reg = ShortcutRegistry::default();
        let mods = if cfg!(target_os = "macos") { Modifiers::COMMAND } else { Modifiers::CTRL };
        assert_eq!(reg.lookup(mods, Key::Num3), Some(ShortcutAction::SwitchWorkspace(2)));
    }
}
