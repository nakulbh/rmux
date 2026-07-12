//! Keyboard shortcut dispatch — translates ShortcutAction into workspace operations.

use crate::app::RmuxApp;
use crate::shortcuts::ShortcutAction;
use crate::workspace::splits::SplitDirection;

impl RmuxApp {
    /// Handle global keyboard shortcuts for workspace/pane operations.
    pub(crate) fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let input = ctx.input(|i| i.clone());

        // === Always-active shortcuts (work even when a text widget has focus) ===

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

            // Normalize modifiers for lookup: strip Ctrl on macOS when Cmd is present
            let lookup_mods = if cfg!(target_os = "macos") && modifiers.command && modifiers.ctrl {
                // Special case: macOS Ctrl+Cmd bracket chords
                *modifiers
            } else if cfg!(target_os = "macos") {
                // On macOS, only Command matters for app shortcuts
                let mut m = *modifiers;
                m.ctrl = false;
                m
            } else {
                // On Linux/Windows, collapse Command into Ctrl for lookup
                let mut m = *modifiers;
                if m.command {
                    m.command = false;
                    m.ctrl = true;
                }
                m
            };

            if let Some(action) = self.shortcut_registry.lookup(lookup_mods, *key)
                && self.dispatch_shortcut_action(ctx, action, mod_active)
            {
                return; // Quit shortcut stops processing
            }
        }

        // === Focus-dependent shortcuts (skip if any text widget is focused) ===

        // Skip shortcuts if any text input is focused (don't steal typing from terminal)
        if ctx.wants_keyboard_input() {
            return;
        }

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

            // Normalize modifiers for lookup
            let lookup_mods = if cfg!(target_os = "macos") && modifiers.command && modifiers.ctrl {
                // Special case: macOS Ctrl+Cmd bracket chords
                *modifiers
            } else if cfg!(target_os = "macos") {
                let mut m = *modifiers;
                m.ctrl = false;
                m
            } else {
                let mut m = *modifiers;
                if m.command {
                    m.command = false;
                    m.ctrl = true;
                }
                m
            };

            if let Some(action) = self.shortcut_registry.lookup(lookup_mods, *key)
                && self.dispatch_shortcut_action(ctx, action, mod_active)
            {
                return;
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
        }

        false
    }
}
