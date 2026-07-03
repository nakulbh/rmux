//! Application state and main egui rendering logic.
//!
//! The `RmuxApp` struct owns the top-level application state
//! and implements the `eframe::App` trait to drive the UI.

use crate::ui::TerminalPane;

/// The root application state.
///
/// Holds the active terminal pane and orchestrates all subsystems.
/// In Phase 1, a single terminal pane with a real PTY shell is rendered.
pub struct RmuxApp {
    /// The active terminal pane with PTY-backed shell.
    pane: TerminalPane,
}

impl RmuxApp {
    /// Create a new application state with a spawned terminal pane.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let cols: u16 = 80;
        let rows: u16 = 24;

        let pane = TerminalPane::spawn(cols, rows).expect("Failed to spawn terminal pane");

        Self { pane }
    }
}

impl eframe::App for RmuxApp {
    /// Called each frame to update the UI.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request continuous repaints for terminal updates
        // (PTY output, cursor blink, etc.)
        ctx.request_repaint_after(std::time::Duration::from_millis(16));

        egui::CentralPanel::default().show(ctx, |ui| {
            self.pane.show(ui);
        });

        // Handle window close request
        if ctx.input(|i| i.viewport().close_requested()) {
            tracing::info!("Window close requested, shutting down");
        }
    }
}
