//! Terminal renderer.
//!
//! Converts a [`GridSnapshot`] into egui paint commands for display.
//! Handles color mapping, cursor rendering, and font metrics.

use crate::state::GridSnapshot;
use alacritty_terminal::vte::ansi::CursorShape;
use egui::{Color32, Pos2, Rect, Ui, Vec2};

/// Arbor terminal cursor color `#ebdbb2` (see `docs/UI_REDESIGN.md`).
///
/// Kept as literal channel values (not a theme token) so the renderer
/// stays decoupled from the app's theme module.
const CURSOR_RGB: (u8, u8, u8) = (0xeb, 0xdb, 0xb2);
/// Cursor alpha for block/hollow shapes (glyph underneath stays readable).
const CURSOR_BLOCK_ALPHA: u8 = 128;
/// Cursor alpha for the thin underline/beam shapes (near-opaque).
const CURSOR_LINE_ALPHA: u8 = 200;

/// The cursor overlay color at the given alpha.
fn cursor_color(alpha: u8) -> Color32 {
    let (r, g, b) = CURSOR_RGB;
    Color32::from_rgba_unmultiplied(r, g, b, alpha)
}

/// Renders terminal grid cells as egui paint commands.
///
/// Handles background rectangles, foreground glyphs, and cursor overlay.
/// The renderer uses a monospace font and caches cell dimensions.
pub struct TerminalRenderer {
    /// Font size for terminal text in pixels.
    pub font_size: f32,
    /// Pre-calculated dimensions of one character cell.
    cell_size: Vec2,
}

impl TerminalRenderer {
    /// Create a new renderer with the given font size.
    ///
    /// The cell size is calculated based on the font size
    /// using a fixed estimate for monospace character proportions.
    pub fn new(font_size: f32) -> Self {
        let cell_size = Self::calc_cell_size(font_size);
        Self { font_size, cell_size }
    }

    /// Draw the terminal grid into the egui UI.
    ///
    /// # Arguments
    ///
    /// * `ui` - The egui UI to draw into.
    /// * `rect` - The allocated region for the terminal.
    /// * `snapshot` - The grid snapshot to render.
    /// * `cursor_visible` - Whether the cursor should blink/show.
    pub fn draw(&self, ui: &mut Ui, rect: Rect, snapshot: &GridSnapshot, cursor_visible: bool) {
        if !ui.is_rect_visible(rect) {
            return;
        }

        let painter = ui.painter();
        let font_id = egui::FontId::monospace(self.font_size);

        // Calculate how many rows/cols we can display
        let visible_cols = ((rect.width() / self.cell_size.x).floor() as u16).min(snapshot.cols);
        let visible_rows = ((rect.height() / self.cell_size.y).floor() as u16).min(snapshot.rows);

        for row in 0..visible_rows {
            for col in 0..visible_cols {
                let cell = &snapshot.cells[row as usize][col as usize];

                let cell_rect = Rect::from_min_size(
                    Pos2::new(
                        rect.left() + col as f32 * self.cell_size.x,
                        rect.top() + row as f32 * self.cell_size.y,
                    ),
                    self.cell_size,
                );

                // Draw background
                painter.rect_filled(cell_rect, 0.0, cell.bg);

                // Draw text character (skip spaces for performance)
                if cell.c != ' ' {
                    let text_pos = Pos2::new(cell_rect.left(), cell_rect.top());
                    let color = cell.fg;
                    painter.text(text_pos, egui::Align2::LEFT_TOP, cell.c, font_id.clone(), color);
                }

                // Draw cursor overlay
                if cell.is_cursor && cursor_visible {
                    let overlay_color = match snapshot.cursor_shape {
                        CursorShape::Block | CursorShape::HollowBlock => {
                            cursor_color(CURSOR_BLOCK_ALPHA)
                        }
                        CursorShape::Underline => {
                            // Draw a thin line at the bottom of the cell
                            let underline_rect = Rect::from_min_max(
                                Pos2::new(cell_rect.left(), cell_rect.bottom() - 2.0),
                                Pos2::new(cell_rect.right(), cell_rect.bottom()),
                            );
                            painter.rect_filled(
                                underline_rect,
                                0.0,
                                cursor_color(CURSOR_LINE_ALPHA),
                            );
                            continue;
                        }
                        CursorShape::Beam => {
                            // Draw a thin vertical bar at the left of the cell
                            let beam_rect = Rect::from_min_max(
                                Pos2::new(cell_rect.left(), cell_rect.top()),
                                Pos2::new(cell_rect.left() + 2.0, cell_rect.bottom()),
                            );
                            painter.rect_filled(beam_rect, 0.0, cursor_color(CURSOR_LINE_ALPHA));
                            continue;
                        }
                        CursorShape::Hidden => continue,
                    };
                    painter.rect_filled(cell_rect, 0.0, overlay_color);
                }
            }
        }
    }

    /// Change the font size and recalculate cell dimensions.
    ///
    /// After calling this, [`cols_rows_for_rect`](Self::cols_rows_for_rect) and
    /// [`draw`](Self::draw) will use the new font size.
    pub fn set_font_size(&mut self, font_size: f32) {
        self.font_size = font_size;
        self.cell_size = Self::calc_cell_size(font_size);
    }

    /// Calculate the required cell size for the given font.
    ///
    /// In this MVP, we use a fixed estimate: monospace characters
    /// are approximately 0.6 × font_size wide and font_size tall.
    fn calc_cell_size(font_size: f32) -> Vec2 {
        // Monospace character width is roughly 0.6 of font height
        let char_width = font_size * 0.6;
        Vec2::new(char_width, font_size)
    }

    /// Get the current cell dimensions.
    pub fn cell_size(&self) -> Vec2 {
        self.cell_size
    }

    /// Calculate the number of columns and rows that fit in a given pixel area.
    pub fn cols_rows_for_rect(&self, rect: Rect) -> (u16, u16) {
        let cols = (rect.width() / self.cell_size.x).floor() as u16;
        let rows = (rect.height() / self.cell_size.y).floor() as u16;
        (cols.max(1), rows.max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_size_calculation() {
        let renderer = TerminalRenderer::new(14.0);
        let size = renderer.cell_size();
        assert!(size.x > 5.0, "Cell width should be reasonable");
        assert!(size.y > 10.0, "Cell height should be reasonable");
    }

    #[test]
    fn test_cols_rows_for_rect() {
        let renderer = TerminalRenderer::new(14.0);
        let rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(800.0, 480.0));
        let (cols, rows) = renderer.cols_rows_for_rect(rect);
        assert!(cols > 0, "Should have at least 1 column");
        assert!(rows > 0, "Should have at least 1 row");
    }

    #[test]
    fn test_new_renderer() {
        let renderer = TerminalRenderer::new(12.0);
        assert_eq!(renderer.font_size, 12.0);
    }

    #[test]
    fn test_set_font_size() {
        let mut renderer = TerminalRenderer::new(14.0);
        let original_cell_size = renderer.cell_size();

        renderer.set_font_size(20.0);
        assert_eq!(renderer.font_size, 20.0);

        let new_cell_size = renderer.cell_size();
        assert!(new_cell_size.x > original_cell_size.x);
        assert!(new_cell_size.y > original_cell_size.y);
    }
}
