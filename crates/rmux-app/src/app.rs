//! Application state and main egui rendering logic.
//!
//! The `RmuxApp` struct owns the top-level application state including the
//! workspace manager, sidebar view, and terminal panes. It implements
//! `eframe::App` to drive the UI and handles keyboard shortcuts.

use egui::Key;

use crate::ui::sidebar::SidebarView;
use crate::ui::{TerminalPane, workspace_view};
use crate::workspace::WorkspaceManager;

/// The root application state.
///
/// Holds the workspace manager, sidebar view, and orchestrates all subsystems.
/// Implements `eframe::App` to render the UI each frame.
pub struct RmuxApp {
    /// Manages all workspaces, panes, and splits.
    workspace_manager: WorkspaceManager,
    /// The sidebar view for workspace tab navigation.
    sidebar: SidebarView,
}

impl RmuxApp {
    /// Create a new application state with a default workspace and terminal pane.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app =
            Self { workspace_manager: WorkspaceManager::new(), sidebar: SidebarView::new() };

        let pane_id = app.workspace_manager.active().active_pane;
        let cols = 80u16;
        let rows = 24u16;

        match TerminalPane::spawn(cols, rows) {
            Ok(terminal) => {
                app.workspace_manager.active_mut().set_terminal(pane_id, terminal);
            }
            Err(e) => {
                tracing::error!("Failed to spawn initial terminal pane: {e}");
            }
        }

        tracing::info!(
            workspaces = app.workspace_manager.workspace_count(),
            panes = app.workspace_manager.total_pane_count(),
            "Application initialized"
        );
        app
    }
}

impl eframe::App for RmuxApp {
    /// Called each frame to update the UI.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process PTY output for all terminal panes
        self.workspace_manager.process_all_panes();

        // Request continuous repaints for terminal updates (PTY output, cursor blink)
        ctx.request_repaint_after(std::time::Duration::from_millis(16));

        // Process keyboard shortcuts
        self.handle_keyboard_shortcuts(ctx);

        // Render the sidebar (left panel)
        self.sidebar.show(ctx, &mut self.workspace_manager);

        // Render the workspace view (central panel)
        self.render_workspace(ctx);
    }
}

impl RmuxApp {
    /// Handle global keyboard shortcuts for workspace/pane operations.
    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
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
                let ws = self.workspace_manager.create_workspace(format!("Workspace {count}"));
                let pane_id = self.workspace_manager.active().active_pane;
                let cols = 80u16;
                let rows = 24u16;
                if let Ok(terminal) = TerminalPane::spawn(cols, rows) {
                    self.workspace_manager.active_mut().set_terminal(pane_id, terminal);
                }
                tracing::info!(workspace_id = ws.0, "Created workspace");
            }

            // Cmd/Ctrl+D: Split right
            if mod_active && !shift_active && *key == Key::D {
                match self.workspace_manager.split_active_right() {
                    Ok(new_id) => {
                        let cols = 80u16;
                        let rows = 24u16;
                        if let Ok(terminal) = TerminalPane::spawn(cols, rows) {
                            self.workspace_manager.active_mut().set_terminal(new_id, terminal);
                        }
                        tracing::info!(pane_id = new_id.0, "Split right");
                    }
                    Err(e) => {
                        tracing::warn!("Split right failed: {e}");
                    }
                }
            }

            // Cmd/Ctrl+Shift+D: Split down
            if mod_active && shift_active && *key == Key::D {
                match self.workspace_manager.split_active_down() {
                    Ok(new_id) => {
                        let cols = 80u16;
                        let rows = 24u16;
                        if let Ok(terminal) = TerminalPane::spawn(cols, rows) {
                            self.workspace_manager.active_mut().set_terminal(new_id, terminal);
                        }
                        tracing::info!(pane_id = new_id.0, "Split down");
                    }
                    Err(e) => {
                        tracing::warn!("Split down failed: {e}");
                    }
                }
            }

            // Cmd/Ctrl+W: Close active pane
            if mod_active && !shift_active && *key == Key::W {
                match self.workspace_manager.close_active_pane() {
                    Ok(()) => {
                        tracing::info!("Closed active pane");
                    }
                    Err(e) => {
                        tracing::warn!("Close pane failed: {e}");
                    }
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

    /// Render the workspace area in the central panel of the window.
    fn render_workspace(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let active_pane = self.workspace_manager.active().active_pane;
            workspace_view::render_pane_tree(
                ui,
                &mut self.workspace_manager.active_mut().root,
                active_pane,
            );
        });
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
