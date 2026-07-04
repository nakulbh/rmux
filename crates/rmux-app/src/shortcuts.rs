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

        // Skip shortcuts if any text input is focused (don't steal typing from terminal)
        if ctx.wants_keyboard_input() {
            return;
        }

        for event in &input.events {
            let egui::Event::Key { key, pressed: true, modifiers, .. } = event else {
                continue;
            };

            let mod_active = modifiers.command || modifiers.ctrl;
            let shift_active = modifiers.shift;

            // Cmd/Ctrl+B: Toggle sidebar
            if mod_active && !shift_active && *key == Key::B {
                self.sidebar.toggle();
                tracing::debug!("Sidebar toggled via keyboard shortcut");
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

            // Cmd/Ctrl+1..9: Switch to workspace by index
            if mod_active
                && !shift_active
                && let Some(index) = key_to_workspace_index(*key)
            {
                self.workspace_manager.switch_to(index);
                tracing::info!(index, "Switched to workspace");
            }

            // Cmd/Ctrl+Shift+[ or ]: Previous/next workspace
            if mod_active && shift_active {
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
