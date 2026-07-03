//! Application state and main egui rendering logic.
//!
//! The `RmuxApp` struct owns the top-level application state
//! and implements the `eframe::App` trait to drive the UI.

use egui::{Color32, Pos2, Stroke};

/// The root application state.
///
/// Holds the main egui context and orchestrates all subsystems.
/// In Phase 0, it simply renders a placeholder terminal grid.
pub struct RmuxApp {
    /// Grid cell dimensions in pixels.
    cell_width: f32,
    /// Grid cell height in pixels.
    cell_height: f32,
}

impl RmuxApp {
    /// Create a new application state.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self { cell_width: 10.0, cell_height: 20.0 }
    }
}

impl eframe::App for RmuxApp {
    /// Called each frame to update the UI.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request continuous repaints for terminal animation (will be needed later)
        ctx.request_repaint();

        egui::CentralPanel::default().show(ctx, |ui| {
            draw_terminal_grid(ui, self.cell_width, self.cell_height);
        });
    }
}

/// Draw a placeholder terminal grid on the egui panel.
///
/// Renders a dark gray background with grid lines representing
/// terminal cell boundaries. This is a visual placeholder that
/// will be replaced by actual terminal rendering in Phase 1.
fn draw_terminal_grid(ui: &mut egui::Ui, cell_width: f32, cell_height: f32) {
    let available = ui.available_size();
    let (rect, _response) = ui.allocate_exact_size(available, egui::Sense::hover());

    if !ui.is_rect_visible(rect) {
        return;
    }

    let painter = ui.painter();

    // --- Background ---
    // Dark gray simulating a terminal background
    let bg_color = Color32::from_gray(30);
    painter.rect_filled(rect, 0.0, bg_color);

    // --- Grid lines ---
    let grid_color = Color32::from_gray(50);
    let line_stroke = Stroke::new(0.5, grid_color);

    // Horizontal lines (separating rows)
    let mut y = rect.top();
    while y < rect.bottom() {
        painter.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], line_stroke);
        y += cell_height;
    }

    // Vertical lines (separating columns)
    let mut x = rect.left();
    while x < rect.right() {
        painter.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())], line_stroke);
        x += cell_width;
    }

    // --- Cursor highlight ---
    // Draw a subtle cursor at a fixed position as a visual indicator
    let cursor_color = Color32::from_rgba_premultiplied(255, 255, 255, 60);
    let cursor_rect = egui::Rect::from_min_size(
        Pos2::new(rect.left() + cell_width * 4.0, rect.top() + cell_height * 2.0),
        egui::Vec2::new(cell_width, cell_height),
    );
    painter.rect_filled(cursor_rect, 0.0, cursor_color);
}
