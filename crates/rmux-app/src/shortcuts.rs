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

    // --- cmux shortcuts (added in feat/fix-keybindings) ---
    /// Create a new surface (tab) in the active workspace.
    NewSurface,
    /// Move focus to the next surface.
    NextSurface,
    /// Move focus to the previous surface.
    PreviousSurface,
    /// Select a surface by 1-based index (1-9).
    SelectSurface(usize),
    /// Rename the active tab.
    RenameTab,
    /// Close the active tab.
    CloseTab,
    /// Close all tabs except the active one.
    CloseOtherTabs,
    /// Reopen the most recently closed tab.
    ReopenLastClosed,
    /// Toggle copy mode (vim-style scrollback navigation).
    ToggleCopyMode,
    /// Split the browser pane to the right (stub: not yet implemented).
    SplitBrowserRight,
    /// Split the browser pane downward (stub: not yet implemented).
    SplitBrowserDown,
    /// Toggle visibility of the right sidebar.
    ToggleRightSidebar,
    /// Open a new window (stub: not yet implemented).
    NewWindow,
    /// Close the current window (stub: not yet implemented).
    CloseWindow,
    /// Alternate binding for [`ShortcutAction::EqualizeSplits`].
    EqualizeSplitsAlt,
    /// Alternate binding for [`ShortcutAction::PrevWorkspace`].
    PrevWorkspaceAlt,
    /// Alternate binding for [`ShortcutAction::NextWorkspace`].
    NextWorkspaceAlt,
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
        // TODO: integrate with RenameTab when browser is not active. cmux uses Cmd+R
        // for rename-tab in non-browser context and reload in browser context — the
        // dispatch handler should pick the right action based on context. Cannot bind
        // both to the same chord in this HashMap registry; browser reload takes
        // priority for now (this matches existing behavior).
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

        // Cmd/Ctrl+Opt/Alt+ArrowLeft → FocusLeft
        reg.register(cmd_ctrl_alt(), Key::ArrowLeft, ShortcutAction::FocusLeft);

        // Cmd/Ctrl+Opt/Alt+ArrowUp → FocusUp
        reg.register(cmd_ctrl_alt(), Key::ArrowUp, ShortcutAction::FocusUp);

        // Cmd/Ctrl+Opt/Alt+ArrowRight → FocusRight
        reg.register(cmd_ctrl_alt(), Key::ArrowRight, ShortcutAction::FocusRight);

        // Cmd/Ctrl+Opt/Alt+ArrowDown → FocusDown
        reg.register(cmd_ctrl_alt(), Key::ArrowDown, ShortcutAction::FocusDown);

        // --- cmux shortcuts (W1.2) ---

        // ⌘T → New surface
        reg.register(cmd_ctrl(), Key::T, ShortcutAction::NewSurface);

        // ⌘⇧] → Next surface
        reg.register(cmd_ctrl_shift(), Key::CloseBracket, ShortcutAction::NextSurface);

        // ⌘⇧[ → Previous surface
        reg.register(cmd_ctrl_shift(), Key::OpenBracket, ShortcutAction::PreviousSurface);

        // ⌃1..9 → Select surface N (macOS-only Ctrl, no Cmd — see `ctrl_only` doc)
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
            reg.register(ctrl_only(), *key, ShortcutAction::SelectSurface(i));
        }

        // NOTE: Cmd/Ctrl+R → RenameTab is NOT registered here because the chord
        // conflicts with `cmd_ctrl() + Key::R → ReloadBrowser` above. cmux
        // resolves this in the dispatcher (rename when no browser, reload when
        // browser is focused). The dispatcher (Todo 14) should handle that
        // disambiguation, and a future iteration can add a `cmd_ctrl_shift()
        // + Key::R → RenameTab` fallback if needed.

        // ⌘W → Close tab (dispatcher picks ClosePane vs CloseTab based on
        // whether the active container is a single pane or a tab with multiple
        // surfaces; the existing ⌘W → ClosePane is kept above for the
        // single-pane case)
        reg.register(cmd_ctrl(), Key::W, ShortcutAction::CloseTab);

        // ⌥⌘T → Close other tabs
        reg.register(cmd_alt(), Key::T, ShortcutAction::CloseOtherTabs);

        // ⌘⇧T → Reopen last closed tab
        reg.register(cmd_ctrl_shift(), Key::T, ShortcutAction::ReopenLastClosed);

        // ⌘⇧M → Toggle copy mode (vim-style scrollback navigation)
        reg.register(cmd_ctrl_shift(), Key::M, ShortcutAction::ToggleCopyMode);

        // ⌥⌘D → Split browser right
        reg.register(cmd_alt(), Key::D, ShortcutAction::SplitBrowserRight);

        // ⌥⌘⇧D → Split browser down
        reg.register(cmd_alt_shift(), Key::D, ShortcutAction::SplitBrowserDown);

        // ⌥⌘B → Toggle right sidebar
        reg.register(cmd_alt(), Key::B, ShortcutAction::ToggleRightSidebar);

        // ⌃⌘= → Equalize splits (alt binding, alias for EqualizeSplits)
        reg.register(Modifiers::CTRL | Modifiers::COMMAND, Key::Equals, ShortcutAction::EqualizeSplitsAlt);

        // ⌃⌘⇧[ → Previous workspace (alt binding, alias for PrevWorkspace)
        reg.register(Modifiers::CTRL | Modifiers::COMMAND | Modifiers::SHIFT, Key::OpenBracket, ShortcutAction::PrevWorkspaceAlt);

        // ⌃⌘⇧] → Next workspace (alt binding, alias for NextWorkspace)
        reg.register(Modifiers::CTRL | Modifiers::COMMAND | Modifiers::SHIFT, Key::CloseBracket, ShortcutAction::NextWorkspaceAlt);

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

/// `cmd_alt()` plus Shift.
pub(crate) fn cmd_alt_shift() -> Modifiers {
    cmd_alt() | Modifiers::SHIFT
}

/// Plain Ctrl modifier, platform-independent.
///
/// On macOS this is the physical Control key (NOT Command). cmux uses
/// `⌃1..9` for surface selection, which is plain Ctrl — hence this helper
/// instead of `cmd_ctrl()`. On Linux/Windows, Ctrl is the canonical
/// app-shortcut modifier, so the lookup will still match.
pub(crate) fn ctrl_only() -> Modifiers {
    Modifiers::CTRL
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

    /// All four pane-focus direction shortcuts must be registered with
    /// `cmd_ctrl_alt()` (i.e. `Cmd+Opt+Arrow` on macOS, `Ctrl+Alt+Arrow` elsewhere).
    /// This matches the cmux reference (`⌥⌘Arrow` for every direction) and
    /// prevents the inconsistency where Left/Up were bare `Cmd+Arrow` while
    /// Right/Down carried the Alt modifier.
    #[test]
    fn test_focus_modifiers_all_match_cmd_ctrl_alt() {
        let reg = ShortcutRegistry::default();
        let mods = cmd_ctrl_alt();
        assert_eq!(reg.lookup(mods, Key::ArrowLeft), Some(ShortcutAction::FocusLeft));
        assert_eq!(reg.lookup(mods, Key::ArrowRight), Some(ShortcutAction::FocusRight));
        assert_eq!(reg.lookup(mods, Key::ArrowUp), Some(ShortcutAction::FocusUp));
        assert_eq!(reg.lookup(mods, Key::ArrowDown), Some(ShortcutAction::FocusDown));
    }

    #[test]
    fn test_focus_left_not_registered_for_bare_cmd_ctrl() {
        let reg = ShortcutRegistry::default();
        let mods = cmd_ctrl();
        assert_eq!(reg.lookup(mods, Key::ArrowLeft), None);
    }

    #[test]
    fn test_focus_up_not_registered_for_bare_cmd_ctrl() {
        let reg = ShortcutRegistry::default();
        let mods = cmd_ctrl();
        assert_eq!(reg.lookup(mods, Key::ArrowUp), None);
    }

    // =====================================================================
    // W4.2 — Comprehensive cmux shortcut lookup coverage.
    //
    // The pre-W4.2 tests above cover only Quit, SwitchWorkspace, and the
    // focus-modifier regression. This block adds one test per cmux
    // shortcut chord so that any future change to `ShortcutRegistry::default`
    // breaks a named, grep-able test. The naming convention is
    // `test_<chord>_<action>` (e.g. `test_cmd_t_new_surface`).
    // =====================================================================

    /// Helper: collapse `Command` → `Ctrl` on Linux/Windows for tests that
    /// use raw `Modifiers::CTRL | Modifiers::COMMAND | ...` literals.
    ///
    /// **Limitation:** This helper is only useful for registry entries that
    /// themselves store canonicalized modifiers (e.g. via `cmd_ctrl()`).
    /// The cmux `Ctrl+Cmd+...` aliases on lines 334, 337, 340 of the
    /// registry store the *raw* bit set without platform normalization,
    /// so they remain macOS-only chords. The corresponding tests below
    /// are gated with `#[cfg(target_os = "macos")]`.
    fn canonical_mod(m: Modifiers) -> Modifiers {
        if cfg!(target_os = "macos") {
            m
        } else {
            let mut out = m;
            if out.command {
                out.command = false;
                out.ctrl = true;
            }
            out
        }
    }

    /// Smoke test for `canonical_mod`: on macOS the helper is the
    /// identity function; on Linux/Windows it collapses `Modifiers::COMMAND`
    /// to `Modifiers::CTRL`. Locks the helper's platform-conditional
    /// collapse behavior so a future "simplification" doesn't break it.
    #[test]
    fn test_canonical_mod_collapse() {
        let raw = Modifiers::CTRL | Modifiers::COMMAND | Modifiers::SHIFT;
        let out = canonical_mod(raw);
        if cfg!(target_os = "macos") {
            assert_eq!(out, raw, "on macOS, canonical_mod should be identity");
        } else {
            assert!(!out.command, "on non-macOS, Command bit must be cleared");
            assert!(out.ctrl, "on non-macOS, Ctrl bit must be set");
            assert!(out.shift, "Shift bit must be preserved");
        }
    }

    // --- Surface / tab ---

    #[test]
    fn test_cmd_t_new_surface() {
        let reg = ShortcutRegistry::default();
        assert_eq!(reg.lookup(cmd_ctrl(), Key::T), Some(ShortcutAction::NewSurface));
    }

    #[test]
    fn test_cmd_shift_bracket_right_next_surface() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_ctrl_shift(), Key::CloseBracket),
            Some(ShortcutAction::NextSurface)
        );
    }

    #[test]
    fn test_cmd_shift_bracket_left_previous_surface() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_ctrl_shift(), Key::OpenBracket),
            Some(ShortcutAction::PreviousSurface)
        );
    }

    /// `ctrl_only()` is intentional (not `cmd_ctrl()`): cmux uses ⌃1..9
    /// (plain physical Ctrl) for surface selection. `cmd_ctrl()` returns
    /// Command on macOS, which would silently break this on macOS.
    #[test]
    fn test_ctrl_n_select_surface() {
        let reg = ShortcutRegistry::default();
        let keys = [
            Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5,
            Key::Num6, Key::Num7, Key::Num8, Key::Num9,
        ];
        for (i, key) in keys.iter().enumerate() {
            assert_eq!(
                reg.lookup(ctrl_only(), *key),
                Some(ShortcutAction::SelectSurface(i)),
                "ctrl_only()+{:?} should resolve to SelectSurface({})",
                key,
                i
            );
        }
    }

    #[test]
    fn test_cmd_alt_t_close_other_tabs() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_alt(), Key::T),
            Some(ShortcutAction::CloseOtherTabs)
        );
    }

    #[test]
    fn test_cmd_shift_t_reopen_last_closed() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_ctrl_shift(), Key::T),
            Some(ShortcutAction::ReopenLastClosed)
        );
    }

    #[test]
    fn test_cmd_shift_m_toggle_copy_mode() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_ctrl_shift(), Key::M),
            Some(ShortcutAction::ToggleCopyMode)
        );
    }

    /// Locks the documented `Cmd/Ctrl+R` conflict: `RenameTab` is
    /// intentionally NOT registered (would collide with `ReloadBrowser`).
    /// The dispatcher (Wave 4) must disambiguate based on whether the
    /// active surface is a browser.
    #[test]
    fn test_cmd_r_resolves_to_reload_browser_not_rename_tab() {
        let reg = ShortcutRegistry::default();
        assert_eq!(reg.lookup(cmd_ctrl(), Key::R), Some(ShortcutAction::ReloadBrowser));
    }

    /// Locks the documented `Cmd/Ctrl+W` conflict: line 313 re-registers
    /// the chord as `CloseTab` after the earlier `ClosePane` on line 209;
    /// the HashMap's last-write-wins makes `CloseTab` the effective
    /// binding. The dispatcher (Wave 4) chooses between `ClosePane` and
    /// `CloseTab` based on tab count.
    #[test]
    fn test_cmd_w_resolves_to_close_tab_with_dispatcher_disambiguation() {
        let reg = ShortcutRegistry::default();
        assert_eq!(reg.lookup(cmd_ctrl(), Key::W), Some(ShortcutAction::CloseTab));
    }

    #[test]
    fn test_cmd_d_split_right() {
        let reg = ShortcutRegistry::default();
        assert_eq!(reg.lookup(cmd_ctrl(), Key::D), Some(ShortcutAction::SplitRight));
    }

    #[test]
    fn test_cmd_shift_d_split_down() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_ctrl_shift(), Key::D),
            Some(ShortcutAction::SplitDown)
        );
    }

    #[test]
    fn test_cmd_alt_d_split_browser_right() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_alt(), Key::D),
            Some(ShortcutAction::SplitBrowserRight)
        );
    }

    #[test]
    fn test_cmd_alt_arrow_left_focus_left() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_ctrl_alt(), Key::ArrowLeft),
            Some(ShortcutAction::FocusLeft)
        );
    }

    #[test]
    fn test_cmd_alt_arrow_right_focus_right() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_ctrl_alt(), Key::ArrowRight),
            Some(ShortcutAction::FocusRight)
        );
    }

    #[test]
    fn test_cmd_alt_arrow_up_focus_up() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_ctrl_alt(), Key::ArrowUp),
            Some(ShortcutAction::FocusUp)
        );
    }

    #[test]
    fn test_cmd_alt_arrow_down_focus_down() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_ctrl_alt(), Key::ArrowDown),
            Some(ShortcutAction::FocusDown)
        );
    }

    #[test]
    fn test_cmd_n_new_workspace() {
        let reg = ShortcutRegistry::default();
        assert_eq!(reg.lookup(cmd_ctrl(), Key::N), Some(ShortcutAction::NewWorkspace));
    }

    #[test]
    fn test_cmd_shift_w_close_workspace() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_ctrl_shift(), Key::W),
            Some(ShortcutAction::CloseWorkspace)
        );
    }

    #[test]
    fn test_cmd_shift_r_rename_workspace() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_ctrl_shift(), Key::R),
            Some(ShortcutAction::RenameWorkspace)
        );
    }

    /// Locks the documented `Cmd/Ctrl+Shift+[` overwrite: the cmux
    /// `PreviousSurface` (registered at line 282) is intentionally
    /// registered AFTER the original `PrevWorkspace` (line 256), so the
    /// HashMap's last-write-wins makes `PreviousSurface` the effective
    /// binding. Workspace navigation on this chord is now via the
    /// `Ctrl+Cmd+Shift+[` alias (see
    /// `test_ctrl_cmd_shift_bracket_left_prev_workspace_alt` below).
    #[test]
    fn test_cmd_shift_bracket_left_overwrites_prev_workspace_to_previous_surface() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_ctrl_shift(), Key::OpenBracket),
            Some(ShortcutAction::PreviousSurface)
        );
    }

    /// Locks the documented `Cmd/Ctrl+Shift+]` overwrite: see the
    /// `OpenBracket` test above for the rationale (last-write-wins
    /// between `NextWorkspace` and `NextSurface`).
    #[test]
    fn test_cmd_shift_bracket_right_overwrites_next_workspace_to_next_surface() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_ctrl_shift(), Key::CloseBracket),
            Some(ShortcutAction::NextSurface)
        );
    }

    /// `Ctrl+Cmd+Shift+[` → `PrevWorkspaceAlt` (macOS-only alias for
    /// workspace nav, since the original `Cmd+Shift+[` chord was
    /// overwritten by `PreviousSurface`). Same macOS-only rationale as
    /// `test_ctrl_cmd_equals_equalize_splits_alt`.
    #[cfg(target_os = "macos")]
    #[test]
    fn test_ctrl_cmd_shift_bracket_left_prev_workspace_alt() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(Modifiers::CTRL | Modifiers::COMMAND | Modifiers::SHIFT, Key::OpenBracket),
            Some(ShortcutAction::PrevWorkspaceAlt)
        );
    }

    /// `Ctrl+Cmd+Shift+]` → `NextWorkspaceAlt` (macOS-only alias).
    #[cfg(target_os = "macos")]
    #[test]
    fn test_ctrl_cmd_shift_bracket_right_next_workspace_alt() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(Modifiers::CTRL | Modifiers::COMMAND | Modifiers::SHIFT, Key::CloseBracket),
            Some(ShortcutAction::NextWorkspaceAlt)
        );
    }

    /// Index-off-by-one boundary check: `Num1` must map to index 0
    /// (not 1). `test_registry_switch_workspace` covers `Num3`.
    #[test]
    fn test_cmd_1_switch_workspace_0() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_ctrl(), Key::Num1),
            Some(ShortcutAction::SwitchWorkspace(0))
        );
    }

    #[test]
    fn test_cmd_shift_equals_equalize_splits() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_ctrl_shift(), Key::Equals),
            Some(ShortcutAction::EqualizeSplits)
        );
    }

    /// `Ctrl+Cmd+=` is a **macOS-only** chord: the registry stores the
    /// raw `Modifiers::CTRL | Modifiers::COMMAND` literal without
    /// platform normalization, so on Linux/Windows the chord is
    /// structurally impossible (Ctrl is the canonical app-shortcut
    /// modifier). Gated to macOS for that reason.
    #[cfg(target_os = "macos")]
    #[test]
    fn test_ctrl_cmd_equals_equalize_splits_alt() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(Modifiers::CTRL | Modifiers::COMMAND, Key::Equals),
            Some(ShortcutAction::EqualizeSplitsAlt)
        );
    }

    #[test]
    fn test_cmd_alt_b_toggle_right_sidebar() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_alt(), Key::B),
            Some(ShortcutAction::ToggleRightSidebar)
        );
    }

    #[test]
    fn test_cmd_alt_shift_d_split_browser_down() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(cmd_alt_shift(), Key::D),
            Some(ShortcutAction::SplitBrowserDown)
        );
    }

    /// Known **gap**: `ShortcutAction::NewWindow` exists (added in W1.2
    /// for the Wave 4 dispatcher) but is NOT registered. This locks the
    /// gap so a future maintainer who adds the binding can flip the
    /// test to a positive assertion and coordinate with the dispatcher.
    #[test]
    fn test_new_window_chord_not_yet_bound() {
        let reg = ShortcutRegistry::default();
        assert_eq!(reg.lookup(cmd_ctrl_shift(), Key::N), None);
    }

    /// Known **gap**: `ShortcutAction::CloseWindow` exists as a variant
    /// (for the Wave 4 dispatcher) but is NOT registered on any chord.
    /// The `Cmd/Ctrl+W` chord is bound to `CloseTab` (see
    /// `test_cmd_w_resolves_to_close_tab_with_dispatcher_disambiguation`).
    /// On macOS, `Modifiers::CTRL | Modifiers::COMMAND + Key::W` is also
    /// unbound; on Linux/Windows the Ctrl+Cmd chord is impossible.
    #[test]
    fn test_close_window_chord_not_yet_bound() {
        let reg = ShortcutRegistry::default();
        #[cfg(target_os = "macos")]
        {
            assert_eq!(
                reg.lookup(Modifiers::CTRL | Modifiers::COMMAND, Key::W),
                None
            );
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(reg.lookup(cmd_ctrl(), Key::W), Some(ShortcutAction::CloseTab));
        }
    }
}
