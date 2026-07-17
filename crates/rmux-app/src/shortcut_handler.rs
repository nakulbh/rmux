//! Dispatch layer: [`AppCommand`] → workspace / terminal operations.
//!
//! This module never inspects keyboard state. The
//! [`crate::shortcut_manager::ShortcutManager`] is solely responsible for
//! turning input into commands.

use crate::app::RmuxApp;
use crate::shortcut_manager::{AppCommand, PollOptions, ShortcutManager};
use crate::workspace::splits::SplitDirection;

impl RmuxApp {
    /// Poll the shortcut manager and dispatch any fired commands.
    ///
    /// Must run **before** the terminal UI so `consume_shortcut` removes
    /// reserved chords from the input stream — otherwise on Linux (where
    /// Ctrl sets both `ctrl` and `command`) the PTY steals the keystroke
    /// and the user has to press twice.
    pub(crate) fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let (find_visible, has_selection) = {
            let ws = self.workspace_manager.active();
            let term = ws.root.find_pane(ws.active_pane).and_then(|n| n.active_terminal());
            let find_visible = term.map(|t| t.is_find_visible()).unwrap_or(false);
            let has_selection = term.map(|t| t.has_selection()).unwrap_or(false);
            (find_visible, has_selection)
        };
        // Previous-frame text focus is fine for gating bare Escape/Enter.
        let text_focused = ctx.wants_keyboard_input();

        let commands = self
            .shortcut_manager
            .poll(ctx, PollOptions { text_focused, find_visible, has_selection });

        for command in commands {
            if self.dispatch_command(ctx, command) {
                break; // Quit
            }
        }
    }

    /// Dispatch a high-level [`AppCommand`].
    ///
    /// Returns `true` if the command was Quit (stop further processing).
    pub(crate) fn dispatch_command(&mut self, ctx: &egui::Context, command: AppCommand) -> bool {
        match command {
            AppCommand::Quit => {
                tracing::info!("Quit shortcut pressed, closing window");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return true;
            }

            AppCommand::FontSizeUp => self.set_font_size(1.0),
            AppCommand::FontSizeDown => self.set_font_size(-1.0),
            AppCommand::FontSizeReset => self.set_font_size(0.0),

            AppCommand::Copy => {
                if let Some(terminal) = self.active_terminal_mut()
                    && let Some(text) = terminal.copy_selection()
                {
                    tracing::debug!(len = text.len(), "Copied terminal selection to clipboard");
                    ctx.copy_text(text.clone());
                    self.last_copied_text = Some(text);
                }
            }

            AppCommand::Paste => {
                // Backup path when Event::Paste is not delivered (rare).
                // Primary paste path is Event::Paste in terminal_pane.
                let text = arboard::Clipboard::new()
                    .ok()
                    .and_then(|mut c| c.get_text().ok())
                    .unwrap_or_default();
                if !text.is_empty()
                    && let Some(terminal) = self.active_terminal_mut()
                {
                    terminal.paste_text(&text);
                }
            }

            AppCommand::Find => {
                // COMMAND+F toggles; bare Escape (only when find is open) closes.
                if let Some(term) = self.active_terminal_mut() {
                    if term.is_find_visible() {
                        term.close_find_bar();
                    } else {
                        term.toggle_find();
                    }
                }
            }

            AppCommand::FindNext => {
                if let Some(term) = self.active_terminal_mut()
                    && term.is_find_visible()
                {
                    term.find_next_match();
                }
            }

            AppCommand::FindPrev => {
                if let Some(term) = self.active_terminal_mut()
                    && term.is_find_visible()
                {
                    term.find_prev_match();
                }
            }

            AppCommand::UseSelectionForFind => {
                if let Some(term) = self.active_terminal_mut() {
                    if !term.is_find_visible() {
                        term.toggle_find();
                    }
                    term.find_with_selection();
                }
            }

            AppCommand::ClearScrollback => {
                if let Some(term) = self.active_terminal_mut() {
                    term.clear_scrollback();
                    tracing::debug!("Terminal scrollback cleared via shortcut");
                }
            }

            AppCommand::ClearScreen => {
                if let Some(term) = self.active_terminal_mut() {
                    term.send_text("\x0c");
                    tracing::debug!("Terminal screen cleared via shortcut");
                }
            }

            AppCommand::ToggleSidebar => {
                self.sidebar.toggle();
                tracing::debug!("Sidebar toggled via keyboard shortcut");
            }

            AppCommand::ToggleNotifications => {
                self.notification_panel.toggle();
            }

            AppCommand::NewWorkspace => {
                let count = self.workspace_manager.workspace_count() + 1;
                let ws = self.create_workspace_with_terminal(format!("Workspace {count}"));
                tracing::info!(workspace_id = ws, "Created workspace");
            }

            AppCommand::SplitRight => {
                match self.split_active_with_terminal(SplitDirection::Horizontal) {
                    Ok(pane_id) => tracing::info!(pane_id, "Split right"),
                    Err(e) => tracing::warn!("Split right failed: {e}"),
                }
            }

            AppCommand::SplitDown => {
                match self.split_active_with_terminal(SplitDirection::Vertical) {
                    Ok(pane_id) => tracing::info!(pane_id, "Split down"),
                    Err(e) => tracing::warn!("Split down failed: {e}"),
                }
            }

            AppCommand::ClosePane => match self.close_active_pane_with_event() {
                Ok(()) => tracing::info!("Closed active pane"),
                Err(e) => tracing::warn!("Close pane failed: {e}"),
            },

            AppCommand::OpenBrowserSplit => match self.open_browser_split(None) {
                Ok(pane_id) => tracing::info!(pane_id, "Opened browser split"),
                Err(e) => tracing::warn!("Open browser split failed: {e}"),
            },

            AppCommand::FocusBrowserUrlBar => {
                if let Some(browser) = self.active_browser_mut() {
                    browser.focus_url_bar = true;
                }
            }

            AppCommand::ReloadBrowser => {
                if let Some(browser) = self.active_browser_mut() {
                    let _ = browser.reload();
                    tracing::debug!("Browser reload via shortcut");
                }
            }

            AppCommand::SwitchWorkspace(index) => {
                self.workspace_manager.switch_to(index);
                tracing::info!(index, "Switched to workspace");
            }

            AppCommand::CloseWorkspace => match self.close_active_workspace_with_event() {
                Ok(id) => tracing::info!(id, "Closed workspace via shortcut"),
                Err(e) => tracing::warn!("Close workspace failed: {e}"),
            },

            AppCommand::RenameWorkspace => {
                self.start_workspace_rename();
            }

            AppCommand::ToggleZoom => {
                self.workspace_manager.toggle_zoom();
            }

            AppCommand::EqualizeSplits | AppCommand::EqualizeSplitsAlt => {
                self.workspace_manager.equalize_splits();
                tracing::debug!("Equalized split sizes via shortcut");
            }

            AppCommand::PrevWorkspace | AppCommand::PrevWorkspaceAlt => {
                self.workspace_manager.switch_prev();
            }

            AppCommand::NextWorkspace | AppCommand::NextWorkspaceAlt => {
                self.workspace_manager.switch_next();
            }

            AppCommand::FocusLeft => {
                self.workspace_manager.active_mut().focus_left();
            }
            AppCommand::FocusRight => {
                self.workspace_manager.active_mut().focus_right();
            }
            AppCommand::FocusUp => {
                self.workspace_manager.active_mut().focus_up();
            }
            AppCommand::FocusDown => {
                self.workspace_manager.active_mut().focus_down();
            }

            AppCommand::NewSurface => {
                if let Err(e) = self.new_surface_with_terminal(None) {
                    tracing::warn!("New surface failed: {e}");
                }
            }

            AppCommand::NextSurface => {
                if let Err(e) = self.workspace_manager.next_surface_in_active() {
                    tracing::warn!("Next surface failed: {e}");
                }
            }

            AppCommand::PreviousSurface => {
                if let Err(e) = self.workspace_manager.previous_surface_in_active() {
                    tracing::warn!("Previous surface failed: {e}");
                }
            }

            AppCommand::SelectSurface(idx) => {
                if let Err(e) = self.workspace_manager.select_surface_in_active(idx) {
                    tracing::warn!("Select surface {idx} failed: {e}");
                }
            }

            AppCommand::RenameTab => {
                tracing::info!("Rename tab requested (UI not yet wired)");
            }

            AppCommand::CloseTab => {
                if self.workspace_manager.terminal_count() > 1 {
                    match self.workspace_manager.close_surface_in_active_with_capture(None) {
                        Ok(_) => tracing::debug!("Closed active surface"),
                        Err(e) => tracing::warn!("Close tab failed: {e}"),
                    }
                } else {
                    match self.close_active_pane_with_event() {
                        Ok(()) => tracing::info!("Closed active pane (via CloseTab fallback)"),
                        Err(e) => tracing::warn!("Close pane failed: {e}"),
                    }
                }
            }

            AppCommand::CloseOtherTabs => {
                if let Err(e) = self.workspace_manager.close_other_surfaces_in_active() {
                    tracing::warn!("Close other tabs failed: {e}");
                }
            }

            AppCommand::ReopenLastClosed => match self.workspace_manager.reopen_last_closed_tab() {
                Ok(()) => tracing::info!("Reopened last closed tab"),
                Err(crate::workspace::model::WorkspaceError::NoClosedTabs) => {
                    tracing::warn!("Reopen last closed: no closed tabs to restore");
                }
                Err(e) => tracing::warn!("Reopen last closed failed: {e}"),
            },

            AppCommand::ToggleCopyMode => {
                if let Some(t) = self.active_terminal_mut() {
                    let now = t.toggle_copy_mode();
                    tracing::debug!(copy_mode = now, "Toggled copy mode");
                }
            }

            AppCommand::PasteImage => {
                if let Some(t) = self.active_terminal_mut() {
                    if t.try_paste_image() {
                        tracing::info!("Pasted clipboard image to terminal");
                    } else {
                        tracing::debug!("No image on clipboard to paste");
                    }
                }
            }

            AppCommand::SplitBrowserRight => {
                tracing::warn!("Browser split not yet implemented");
            }
            AppCommand::SplitBrowserDown => {
                tracing::warn!("Browser split not yet implemented");
            }

            AppCommand::ToggleRightSidebar => {
                self.sidebar.toggle_right();
            }

            AppCommand::NewWindow => {
                tracing::warn!("New window not yet implemented (multi-window support planned)");
            }
            AppCommand::CloseWindow => {
                tracing::warn!("Close window not yet implemented (multi-window support planned)");
            }
        }

        false
    }
}

/// Access to the manager type for docs / external wiring.
#[allow(dead_code)]
pub type Manager = ShortcutManager;

#[cfg(test)]
mod tests {
    use crate::shortcut_manager::{AppCommand, PollOptions, ShortcutManager, primary_mod_pressed};
    use egui::Key;

    #[test]
    fn manager_resolves_core_chords() {
        let m = ShortcutManager::with_defaults();
        let p = primary_mod_pressed();
        assert_eq!(m.resolve(p, Key::T), Some(AppCommand::NewSurface));
        assert_eq!(m.resolve(p, Key::D), Some(AppCommand::SplitRight));
        assert_eq!(m.resolve(p, Key::B), Some(AppCommand::ToggleSidebar));
    }

    #[test]
    fn find_escape_gated() {
        let m = ShortcutManager::with_defaults();
        assert!(
            m.resolve_with_options(egui::Modifiers::NONE, Key::Escape, PollOptions::default())
                .is_none()
        );
        assert_eq!(
            m.resolve_with_options(
                egui::Modifiers::NONE,
                Key::Escape,
                PollOptions { find_visible: true, text_focused: false, has_selection: false },
            ),
            Some(AppCommand::Find)
        );
    }
}
