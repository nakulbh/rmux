//! Keyboard shortcut dispatch — translates ShortcutAction into workspace operations.

use crate::app::RmuxApp;
use crate::shortcuts::ShortcutAction;
use crate::workspace::splits::SplitDirection;

/// Whether a [`ShortcutAction`] should still be dispatched when a text widget
/// has keyboard focus (e.g. the terminal or its find bar).
///
/// These actions are "always-active" and must fire even while the user is
/// typing into the terminal. All other actions (split, focus, rename, etc.)
/// are skipped when text input is focused so we don't steal keystrokes
/// from the terminal — in particular bare `Escape` and `Enter`, which are
/// bound to `Find` / `FindNext` with `Modifiers::NONE`.
fn should_dispatch_when_text_focused(action: ShortcutAction) -> bool {
    matches!(
        action,
        ShortcutAction::Quit
            | ShortcutAction::Copy
            | ShortcutAction::FontSizeUp
            | ShortcutAction::FontSizeDown
            | ShortcutAction::FontSizeReset
            | ShortcutAction::ClearScreen
            | ShortcutAction::ClearScrollback
    )
}

/// Normalize raw egui modifiers into the canonical form used for
/// [`crate::shortcuts::ShortcutRegistry`] lookups.
///
/// - Always clears `mac_cmd`: egui-winit sets `mac_cmd` alongside `command`
///   for every physical Cmd press on macOS, but registry entries built from
///   `Modifiers::COMMAND` never set it — left alone, the `HashMap`'s derived
///   `Eq`/`Hash` would miss on every Cmd chord even though `command` matches.
/// - Leaves `ctrl` untouched on macOS. Bare `Ctrl` chords (`⌃1..9 →
///   SelectSurface`, registered via `ctrl_only()`) and combined `Ctrl+Cmd`
///   bracket chords (registered via `Modifiers::CTRL | Modifiers::COMMAND`)
///   both need the physical Ctrl bit to reach the registry unchanged.
/// - On Linux/Windows, collapses `command` into `ctrl` since the registry's
///   `cmd_ctrl()`-family helpers store `Modifiers::CTRL` as the canonical
///   app-shortcut modifier there.
fn normalize_lookup_mods(mut modifiers: egui::Modifiers) -> egui::Modifiers {
    modifiers.mac_cmd = false;
    if !cfg!(target_os = "macos") && modifiers.command {
        modifiers.command = false;
        modifiers.ctrl = true;
    }
    modifiers
}

impl RmuxApp {
    /// Handle global keyboard shortcuts for workspace/pane operations.
    pub(crate) fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let input = ctx.input(|i| i.clone());

        for event in &input.events {
            let egui::Event::Key { key, pressed: true, modifiers, .. } = event else {
                continue;
            };

            // On macOS, Cmd is for app shortcuts, Ctrl is for terminal control characters.
            // On Linux/Windows, both are used for app shortcuts.
            let mod_active = if cfg!(target_os = "macos") {
                modifiers.command && !modifiers.ctrl
            } else {
                modifiers.command || modifiers.ctrl
            };

            let lookup_mods = normalize_lookup_mods(*modifiers);

            let Some(action) = self.shortcut_registry.lookup(lookup_mods, *key) else {
                continue;
            };

            // Skip focus-dependent actions when a text widget is focused so we
            // don't steal keystrokes (especially bare Escape/Enter) from the
            // terminal. Always-active actions fall through.
            if ctx.wants_keyboard_input() && !should_dispatch_when_text_focused(action) {
                continue;
            }

            if self.dispatch_shortcut_action(ctx, action, mod_active) {
                return; // Quit shortcut stops processing
            }
        }
    }

    /// Dispatch a [`ShortcutAction`] to the appropriate handler.
    ///
    /// Returns `true` if the action was a Quit request (which stops further
    /// shortcut processing).
    fn dispatch_shortcut_action(
        &mut self,
        ctx: &egui::Context,
        action: ShortcutAction,
        mod_active: bool,
    ) -> bool {
        match action {
            ShortcutAction::Quit => {
                tracing::info!("Quit shortcut pressed, closing window");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return true;
            }

            ShortcutAction::FontSizeUp => {
                self.set_font_size(1.0);
            }
            ShortcutAction::FontSizeDown => {
                self.set_font_size(-1.0);
            }
            ShortcutAction::FontSizeReset => {
                self.set_font_size(0.0);
            }

            ShortcutAction::Copy => {
                if let Some(terminal) = self.active_terminal_mut()
                    && let Some(text) = terminal.copy_selection()
                {
                    ctx.copy_text(text.clone());
                    self.last_copied_text = Some(text);
                    tracing::debug!("Copied terminal selection to clipboard");
                }
            }

            ShortcutAction::Find => {
                if let Some(term) = self.active_terminal_mut() {
                    if !mod_active {
                        // Escape pressed without modifier: close find bar if visible
                        if term.is_find_visible() {
                            term.close_find_bar();
                        }
                    } else {
                        // Cmd/Ctrl+F: toggle find bar
                        term.toggle_find();
                    }
                }
            }

            ShortcutAction::FindNext => {
                if let Some(term) = self.active_terminal_mut() {
                    if !mod_active {
                        // Enter pressed without modifier: find next if find visible
                        if term.is_find_visible() {
                            term.find_next_match();
                        }
                    } else {
                        // Cmd/Ctrl+G: find next if find visible
                        if term.is_find_visible() {
                            term.find_next_match();
                        }
                    }
                }
            }

            ShortcutAction::FindPrev => {
                if let Some(term) = self.active_terminal_mut()
                    && term.is_find_visible()
                {
                    term.find_prev_match();
                }
            }

            ShortcutAction::UseSelectionForFind => {
                if let Some(term) = self.active_terminal_mut() {
                    if !term.is_find_visible() {
                        term.toggle_find();
                    }
                    term.find_with_selection();
                }
            }

            ShortcutAction::ClearScrollback => {
                if let Some(term) = self.active_terminal_mut() {
                    term.clear_scrollback();
                    tracing::debug!("Terminal scrollback cleared via shortcut");
                }
            }

            ShortcutAction::ClearScreen => {
                if let Some(term) = self.active_terminal_mut() {
                    term.send_text("\x0c");
                    tracing::debug!("Terminal screen cleared via shortcut");
                }
            }

            ShortcutAction::ToggleSidebar => {
                self.sidebar.toggle();
                tracing::debug!("Sidebar toggled via keyboard shortcut");
            }

            ShortcutAction::ToggleNotifications => {
                self.notification_panel.toggle();
            }

            ShortcutAction::NewWorkspace => {
                let count = self.workspace_manager.workspace_count() + 1;
                let ws = self.create_workspace_with_terminal(format!("Workspace {count}"));
                tracing::info!(workspace_id = ws, "Created workspace");
            }

            ShortcutAction::SplitRight => {
                match self.split_active_with_terminal(SplitDirection::Horizontal) {
                    Ok(pane_id) => tracing::info!(pane_id, "Split right"),
                    Err(e) => tracing::warn!("Split right failed: {e}"),
                }
            }

            ShortcutAction::SplitDown => {
                match self.split_active_with_terminal(SplitDirection::Vertical) {
                    Ok(pane_id) => tracing::info!(pane_id, "Split down"),
                    Err(e) => tracing::warn!("Split down failed: {e}"),
                }
            }

            ShortcutAction::ClosePane => match self.close_active_pane_with_event() {
                Ok(()) => tracing::info!("Closed active pane"),
                Err(e) => tracing::warn!("Close pane failed: {e}"),
            },

            ShortcutAction::OpenBrowserSplit => match self.open_browser_split(None) {
                Ok(pane_id) => tracing::info!(pane_id, "Opened browser split"),
                Err(e) => tracing::warn!("Open browser split failed: {e}"),
            },

            ShortcutAction::FocusBrowserUrlBar => {
                if let Some(browser) = self.active_browser_mut() {
                    browser.focus_url_bar = true;
                }
            }

            ShortcutAction::ReloadBrowser => {
                if let Some(browser) = self.active_browser_mut() {
                    let _ = browser.reload();
                    tracing::debug!("Browser reload via shortcut");
                }
            }

            ShortcutAction::SwitchWorkspace(index) => {
                self.workspace_manager.switch_to(index);
                tracing::info!(index, "Switched to workspace");
            }

            ShortcutAction::CloseWorkspace => match self.close_active_workspace_with_event() {
                Ok(id) => tracing::info!(id, "Closed workspace via shortcut"),
                Err(e) => tracing::warn!("Close workspace failed: {e}"),
            },

            ShortcutAction::RenameWorkspace => {
                self.start_workspace_rename();
            }

            ShortcutAction::ToggleZoom => {
                self.workspace_manager.toggle_zoom();
            }

            ShortcutAction::EqualizeSplits => {
                self.workspace_manager.equalize_splits();
                tracing::debug!("Equalized split sizes via shortcut");
            }

            ShortcutAction::PrevWorkspace => {
                self.workspace_manager.switch_prev();
            }

            ShortcutAction::NextWorkspace => {
                self.workspace_manager.switch_next();
            }

            ShortcutAction::FocusLeft => {
                self.workspace_manager.active_mut().focus_left();
            }

            ShortcutAction::FocusRight => {
                self.workspace_manager.active_mut().focus_right();
            }

            ShortcutAction::FocusUp => {
                self.workspace_manager.active_mut().focus_up();
            }

            ShortcutAction::FocusDown => {
                self.workspace_manager.active_mut().focus_down();
            }

            // --- cmux shortcuts (W4.1) ---
            ShortcutAction::NewSurface => {
                match self.workspace_manager.new_surface_in_active(None) {
                    Ok(id) => tracing::info!(surface_id = id.0, "Created new surface"),
                    Err(e) => tracing::warn!("New surface failed: {e}"),
                }
            }

            ShortcutAction::NextSurface => {
                if let Err(e) = self.workspace_manager.next_surface_in_active() {
                    tracing::warn!("Next surface failed: {e}");
                }
            }

            ShortcutAction::PreviousSurface => {
                if let Err(e) = self.workspace_manager.previous_surface_in_active() {
                    tracing::warn!("Previous surface failed: {e}");
                }
            }

            ShortcutAction::SelectSurface(idx) => {
                if let Err(e) = self.workspace_manager.select_surface_in_active(idx) {
                    tracing::warn!("Select surface {idx} failed: {e}");
                }
            }

            ShortcutAction::RenameTab => {
                // UI for inline tab-rename is not yet wired (see W3.2 follow-up
                // in worker-notes.md). The Cmd+R chord is currently bound to
                // ReloadBrowser, so this arm is only reachable when the
                // dispatcher manually routes here.
                tracing::info!("Rename tab requested (UI not yet wired)");
            }

            ShortcutAction::CloseTab => {
                if self.workspace_manager.terminal_count() > 1 {
                    match self.workspace_manager.close_surface_in_active_with_capture(None) {
                        Ok(_) => tracing::debug!("Closed active surface"),
                        Err(e) => tracing::warn!("Close tab failed: {e}"),
                    }
                } else {
                    // Fall through to close pane when only one surface is open.
                    match self.close_active_pane_with_event() {
                        Ok(()) => tracing::info!("Closed active pane (via Cmd+W fallback)"),
                        Err(e) => tracing::warn!("Close pane failed: {e}"),
                    }
                }
            }

            ShortcutAction::CloseOtherTabs => {
                if let Err(e) = self.workspace_manager.close_other_surfaces_in_active() {
                    tracing::warn!("Close other tabs failed: {e}");
                }
            }

            ShortcutAction::ReopenLastClosed => {
                match self.workspace_manager.reopen_last_closed_tab() {
                    Ok(()) => tracing::info!("Reopened last closed tab"),
                    Err(crate::workspace::model::WorkspaceError::NoClosedTabs) => {
                        tracing::warn!("Reopen last closed: no closed tabs to restore");
                    }
                    Err(e) => tracing::warn!("Reopen last closed failed: {e}"),
                }
            }

            ShortcutAction::ToggleCopyMode => {
                if let Some(t) = self.active_terminal_mut() {
                    let now = t.toggle_copy_mode();
                    tracing::debug!(copy_mode = now, "Toggled copy mode");
                }
            }

            ShortcutAction::SplitBrowserRight => {
                tracing::warn!("Browser split not yet implemented");
            }

            ShortcutAction::SplitBrowserDown => {
                tracing::warn!("Browser split not yet implemented");
            }

            ShortcutAction::ToggleRightSidebar => {
                self.sidebar.toggle_right();
            }

            ShortcutAction::NewWindow => {
                tracing::warn!("New window not yet implemented (multi-window support planned)");
            }

            ShortcutAction::CloseWindow => {
                tracing::warn!("Close window not yet implemented (multi-window support planned)");
            }

            ShortcutAction::EqualizeSplitsAlt => {
                self.workspace_manager.equalize_splits();
                tracing::debug!("Equalized split sizes via shortcut (alt binding)");
            }

            ShortcutAction::PrevWorkspaceAlt => {
                self.workspace_manager.switch_prev();
            }

            ShortcutAction::NextWorkspaceAlt => {
                self.workspace_manager.switch_next();
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Always-active actions (must dispatch even when text is focused) ---

    #[test]
    fn quit_dispatches_when_text_focused() {
        assert!(should_dispatch_when_text_focused(ShortcutAction::Quit));
    }

    #[test]
    fn copy_dispatches_when_text_focused() {
        assert!(should_dispatch_when_text_focused(ShortcutAction::Copy));
    }

    #[test]
    fn font_size_up_dispatches_when_text_focused() {
        assert!(should_dispatch_when_text_focused(ShortcutAction::FontSizeUp));
    }

    #[test]
    fn font_size_down_dispatches_when_text_focused() {
        assert!(should_dispatch_when_text_focused(ShortcutAction::FontSizeDown));
    }

    #[test]
    fn font_size_reset_dispatches_when_text_focused() {
        assert!(should_dispatch_when_text_focused(ShortcutAction::FontSizeReset));
    }

    #[test]
    fn clear_screen_dispatches_when_text_focused() {
        assert!(should_dispatch_when_text_focused(ShortcutAction::ClearScreen));
    }

    #[test]
    fn clear_scrollback_dispatches_when_text_focused() {
        assert!(should_dispatch_when_text_focused(ShortcutAction::ClearScrollback));
    }

    // --- Focus-dependent actions (must be skipped when text is focused) ---

    #[test]
    fn find_skipped_when_text_focused() {
        assert!(!should_dispatch_when_text_focused(ShortcutAction::Find));
    }

    #[test]
    fn find_next_skipped_when_text_focused() {
        assert!(!should_dispatch_when_text_focused(ShortcutAction::FindNext));
    }

    #[test]
    fn switch_workspace_skipped_when_text_focused() {
        assert!(!should_dispatch_when_text_focused(ShortcutAction::SwitchWorkspace(0)));
    }

    #[test]
    fn split_right_skipped_when_text_focused() {
        assert!(!should_dispatch_when_text_focused(ShortcutAction::SplitRight));
    }

    #[test]
    fn split_down_skipped_when_text_focused() {
        assert!(!should_dispatch_when_text_focused(ShortcutAction::SplitDown));
    }

    #[test]
    fn focus_left_skipped_when_text_focused() {
        assert!(!should_dispatch_when_text_focused(ShortcutAction::FocusLeft));
    }

    #[test]
    fn toggle_sidebar_skipped_when_text_focused() {
        assert!(!should_dispatch_when_text_focused(ShortcutAction::ToggleSidebar));
    }

    #[test]
    fn rename_workspace_skipped_when_text_focused() {
        assert!(!should_dispatch_when_text_focused(ShortcutAction::RenameWorkspace));
    }

    // --- normalize_lookup_mods (macOS) ---
    //
    // These assert against `cfg!(target_os = "macos")` behavior, matching
    // the dev/CI environment this crate targets.

    #[test]
    fn bare_ctrl_survives_normalization_for_select_surface() {
        // ⌃1 (SelectSurface) must reach the registry as plain Ctrl, matching
        // `ctrl_only()`. Regression test: a prior version unconditionally
        // zeroed `ctrl` on macOS, which made `⌃1..9` unreachable.
        let raw = egui::Modifiers { ctrl: true, ..Default::default() };
        let normalized = normalize_lookup_mods(raw);
        assert_eq!(normalized, egui::Modifiers::CTRL);
    }

    #[test]
    fn bare_cmd_normalizes_to_command_only() {
        // Physical Cmd sets both `command` and `mac_cmd` on macOS; only
        // `command` should survive, matching `cmd_ctrl()`'s registry entries.
        let raw = egui::Modifiers { command: true, mac_cmd: true, ..Default::default() };
        let normalized = normalize_lookup_mods(raw);
        assert_eq!(normalized, egui::Modifiers::COMMAND);
    }

    #[test]
    fn ctrl_cmd_bracket_chord_keeps_both_bits() {
        // ⌃⌘[ / ⌃⌘] (PrevWorkspace/NextWorkspace) are registered as
        // `Modifiers::CTRL | Modifiers::COMMAND`; both bits must survive.
        let raw =
            egui::Modifiers { ctrl: true, command: true, mac_cmd: true, ..Default::default() };
        let normalized = normalize_lookup_mods(raw);
        assert_eq!(normalized, egui::Modifiers::CTRL | egui::Modifiers::COMMAND);
    }

    #[test]
    fn cmd_alt_arrow_keeps_alt_and_command_without_ctrl() {
        let raw = egui::Modifiers { command: true, mac_cmd: true, alt: true, ..Default::default() };
        let normalized = normalize_lookup_mods(raw);
        assert_eq!(normalized, egui::Modifiers::COMMAND | egui::Modifiers::ALT);
    }
}
