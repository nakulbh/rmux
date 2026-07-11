//! Global keyboard shortcut handling for [`RmuxApp`].
//!
//! Split out of `app.rs` to keep both modules focused: this module only
//! translates key chords into workspace/pane operations.

use egui::Key;

use crate::app::RmuxApp;
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

            // Cmd/Ctrl+Q or Cmd/Ctrl+Shift+Q: Quit application
            if mod_active && !modifiers.alt && *key == Key::Q {
                tracing::info!("Quit shortcut pressed, closing window");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return; // quit immediately, skip other shortcuts
            }

            // Cmd/Ctrl+Plus or Cmd/Ctrl+Equals: Increase font size
            if mod_active && !modifiers.shift && (*key == Key::Plus || *key == Key::Equals) {
                self.set_font_size(1.0);
                continue;
            }

            // Cmd/Ctrl+Minus: Decrease font size
            if mod_active && !modifiers.shift && *key == Key::Minus {
                self.set_font_size(-1.0);
                continue;
            }

            // Cmd/Ctrl+0: Reset font size to default
            if mod_active && !modifiers.shift && *key == Key::Num0 {
                self.set_font_size(0.0);
                continue;
            }

            // Cmd/Ctrl+C: Copy selected text from active terminal pane
            if mod_active && !modifiers.shift && !modifiers.alt && *key == Key::C {
                if let Some(terminal) = self.active_terminal_mut()
                    && let Some(text) = terminal.copy_selection()
                {
                    ctx.copy_text(text.clone());
                    self.last_copied_text = Some(text);
                    tracing::debug!("Copied terminal selection to clipboard");
                }
                continue;
            }

            // Escape: Close find bar if visible
            if !modifiers.command
                && !modifiers.ctrl
                && !modifiers.alt
                && !modifiers.shift
                && *key == Key::Escape
                && let Some(term) = self.active_terminal_mut()
                && term.is_find_visible()
            {
                term.close_find_bar();
                continue;
            }

            // Enter: Find next match when find bar is visible
            if !modifiers.command
                && !modifiers.ctrl
                && !modifiers.alt
                && !modifiers.shift
                && *key == Key::Enter
                && let Some(term) = self.active_terminal_mut()
                && term.is_find_visible()
            {
                term.find_next_match();
                continue;
            }

            // Cmd/Ctrl+F: Toggle find bar
            if mod_active && !modifiers.alt && *key == Key::F {
                if let Some(term) = self.active_terminal_mut() {
                    term.toggle_find();
                }
                continue;
            }

            // Cmd/Ctrl+G: Find next match
            if mod_active
                && !modifiers.alt
                && *key == Key::G
                && let Some(term) = self.active_terminal_mut()
                && term.is_find_visible()
            {
                term.find_next_match();
                continue;
            }

            // Alt+Cmd/Ctrl+G: Find previous match
            if mod_active
                && modifiers.alt
                && *key == Key::G
                && let Some(term) = self.active_terminal_mut()
                && term.is_find_visible()
            {
                term.find_prev_match();
                continue;
            }

            // Cmd/Ctrl+E: Use selection for find
            if mod_active && !modifiers.alt && *key == Key::E {
                if let Some(term) = self.active_terminal_mut() {
                    if !term.is_find_visible() {
                        term.toggle_find();
                    }
                    term.find_with_selection();
                }
                continue;
            }

            // Cmd/Ctrl+K: Clear terminal scrollback
            if mod_active && !modifiers.shift && !modifiers.alt && *key == Key::K {
                if let Some(term) = self.active_terminal_mut() {
                    term.clear_scrollback();
                    tracing::debug!("Terminal scrollback cleared via shortcut");
                }
                continue;
            }

            // Cmd/Ctrl+Shift+K: Clear screen (sends Ctrl+L to terminal)
            if mod_active && modifiers.shift && !modifiers.alt && *key == Key::K {
                if let Some(term) = self.active_terminal_mut() {
                    term.send_text("\x0c");
                    tracing::debug!("Terminal screen cleared via shortcut");
                }
                continue;
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
            let shift_active = modifiers.shift;

            // Cmd/Ctrl+B: Toggle sidebar
            if mod_active && !shift_active && *key == Key::B {
                self.sidebar.toggle();
                tracing::debug!("Sidebar toggled via keyboard shortcut");
            }

            // Cmd/Ctrl+I: Toggle notification panel (matches cmux)
            if mod_active && !shift_active && *key == Key::I {
                self.notification_panel.toggle();
            }

            // Cmd/Ctrl+N: New workspace
            if mod_active && !shift_active && *key == Key::N {
                let count = self.workspace_manager.workspace_count() + 1;
                let ws = self.create_workspace_with_terminal(format!("Workspace {count}"));
                tracing::info!(workspace_id = ws, "Created workspace");
            }

            // Cmd/Ctrl+D: Split right
            if mod_active && !shift_active && *key == Key::D {
                match self.split_active_with_terminal(SplitDirection::Horizontal) {
                    Ok(pane_id) => tracing::info!(pane_id, "Split right"),
                    Err(e) => tracing::warn!("Split right failed: {e}"),
                }
            }

            // Cmd/Ctrl+Shift+D: Split down
            if mod_active && shift_active && *key == Key::D {
                match self.split_active_with_terminal(SplitDirection::Vertical) {
                    Ok(pane_id) => tracing::info!(pane_id, "Split down"),
                    Err(e) => tracing::warn!("Split down failed: {e}"),
                }
            }

            // Cmd/Ctrl+W: Close active pane
            if mod_active && !shift_active && *key == Key::W {
                match self.close_active_pane_with_event() {
                    Ok(()) => tracing::info!("Closed active pane"),
                    Err(e) => tracing::warn!("Close pane failed: {e}"),
                }
            }

            // Cmd/Ctrl+Shift+L: Open browser split
            if mod_active && shift_active && *key == Key::L {
                match self.open_browser_split(None) {
                    Ok(pane_id) => tracing::info!(pane_id, "Opened browser split"),
                    Err(e) => tracing::warn!("Open browser split failed: {e}"),
                }
            }

            // Cmd/Ctrl+L: Focus browser address bar (when active pane is browser)
            if mod_active && !shift_active && *key == Key::L && self.active_browser_mut().is_some()
            {
                tracing::debug!("Focus browser address bar");
            }

            // Cmd/Ctrl+R: Reload browser page (when active pane is browser)
            if mod_active
                && !shift_active
                && *key == Key::R
                && let Some(browser) = self.active_browser_mut()
            {
                let _ = browser.reload();
                tracing::debug!("Browser reload via shortcut");
            }

            // Cmd/Ctrl+1..9: Switch to workspace by index
            if mod_active
                && !shift_active
                && let Some(index) = key_to_workspace_index(*key)
            {
                self.workspace_manager.switch_to(index);
                tracing::info!(index, "Switched to workspace");
            }

            // Cmd/Ctrl+Shift+W: Close active workspace
            if mod_active && shift_active && *key == Key::W {
                match self.close_active_workspace_with_event() {
                    Ok(id) => tracing::info!(id, "Closed workspace via shortcut"),
                    Err(e) => tracing::warn!("Close workspace failed: {e}"),
                }
            }

            // Cmd/Ctrl+Shift+R: Rename active workspace (start inline rename)
            if mod_active && shift_active && *key == Key::R {
                self.start_workspace_rename();
            }

            // Cmd/Ctrl+Shift+Enter: Toggle pane zoom (maximize/restore)
            if mod_active && shift_active && *key == Key::Enter {
                self.workspace_manager.toggle_zoom();
            }

            // Cmd/Ctrl+Shift+=: Equalize all split sizes
            if mod_active && shift_active && *key == Key::Equals {
                self.workspace_manager.equalize_splits();
                tracing::debug!("Equalized split sizes via shortcut");
            }

            // Cmd/Ctrl+Shift+[ or ]: Previous/next workspace.
            // On macOS, also accept cmux's Ctrl+Cmd+[ or ] chord.
            let mac_ctrl_cmd_bracket = cfg!(target_os = "macos")
                && modifiers.command
                && modifiers.ctrl
                && !modifiers.shift
                && !modifiers.alt;
            let workspace_bracket_chord = (mod_active && shift_active) || mac_ctrl_cmd_bracket;
            if workspace_bracket_chord {
                match *key {
                    Key::OpenBracket => self.workspace_manager.switch_prev(),
                    Key::CloseBracket => self.workspace_manager.switch_next(),
                    _ => {}
                }
            }

            // Arrow keys for pane focus (Cmd/Ctrl+Arrow without shift)
            if mod_active && !shift_active {
                match *key {
                    Key::ArrowLeft | Key::ArrowUp => {
                        self.workspace_manager.active_mut().focus_prev();
                    }
                    Key::ArrowRight | Key::ArrowDown => {
                        self.workspace_manager.active_mut().focus_next();
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Convert a number key (1-9) to a workspace index (0-8).
fn key_to_workspace_index(key: Key) -> Option<usize> {
    match key {
        Key::Num1 => Some(0),
        Key::Num2 => Some(1),
        Key::Num3 => Some(2),
        Key::Num4 => Some(3),
        Key::Num5 => Some(4),
        Key::Num6 => Some(5),
        Key::Num7 => Some(6),
        Key::Num8 => Some(7),
        Key::Num9 => Some(8),
        _ => None,
    }
}
