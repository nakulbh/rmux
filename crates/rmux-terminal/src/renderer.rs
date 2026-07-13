use crate::state::GridSnapshot;
use alacritty_terminal::vte::ansi::CursorShape;
use egui::{Color32, Pos2, Rect, Ui, Vec2};

const CURSOR_BLOCK_ALPHA: u8 = 128;
const CURSOR_LINE_ALPHA: u8 = 200;

fn cursor_color(alpha: u8, theme_color: Color32) -> Color32 {
    Color32::from_rgba_unmultiplied(theme_color.r(), theme_color.g(), theme_color.b(), alpha)
}

pub struct TerminalRenderer {
    pub font_size: f32,
    cell_size: Vec2,
    cell_size_measured: bool,
}

impl TerminalRenderer {
    pub fn new(font_size: f32) -> Self {
        let cell_size = Self::estimate_cell_size(font_size);
        Self { font_size, cell_size, cell_size_measured: false }
    }

    /// Measure cell size from the actual loaded font on the first call.
    /// Subsequent calls are a no-op.
    fn ensure_cell_size_measured(&mut self, ui: &Ui) {
        if self.cell_size_measured {
            return;
        }
        let font_id = egui::FontId::monospace(self.font_size);
        let glyph_width = ui.fonts(|f| {
            f.layout("M".to_string(), font_id.clone(), Color32::WHITE, f32::INFINITY).size().x
        });
        let row_height = ui.fonts(|f| f.row_height(&font_id));

        self.cell_size = Vec2::new(glyph_width, row_height);
        self.cell_size_measured = true;
    }

    pub fn draw(&mut self, ui: &mut Ui, rect: Rect, snapshot: &GridSnapshot, cursor_visible: bool) {
        if !ui.is_rect_visible(rect) {
            return;
        }

        self.ensure_cell_size_measured(ui);

        let painter = ui.painter();
        let cell_w = self.cell_size.x;
        let cell_h = self.cell_size.y;

        // Fill unused rows below the grid with terminal background
        let used_height = snapshot.rows as f32 * cell_h;
        if used_height < rect.height() {
            let fill = Rect::from_min_max(
                Pos2::new(rect.left(), rect.top() + used_height),
                Pos2::new(rect.right(), rect.bottom()),
            );
            painter.rect_filled(fill, 0.0, snapshot.terminal_bg);
        }
        // Fill unused columns to the right
        let used_width = snapshot.cols as f32 * cell_w;
        if used_width < rect.width() {
            let fill = Rect::from_min_max(
                Pos2::new(rect.left() + used_width, rect.top()),
                Pos2::new(rect.right(), rect.top() + used_height.min(rect.height())),
            );
            painter.rect_filled(fill, 0.0, snapshot.terminal_bg);
        }

        let visible_cols = ((rect.width() / cell_w).floor() as u16).min(snapshot.cols);
        let visible_rows = ((rect.height() / cell_h).floor() as u16).min(snapshot.rows);

        let font_id = egui::FontId::monospace(self.font_size);

        for row in 0..visible_rows {
            for col in 0..visible_cols {
                let cell = &snapshot.cells[row as usize][col as usize];

                let cell_rect = Rect::from_min_size(
                    Pos2::new(rect.left() + col as f32 * cell_w, rect.top() + row as f32 * cell_h),
                    self.cell_size,
                );

                painter.rect_filled(cell_rect, 0.0, cell.bg);

                if cell.c != ' ' {
                    let base_x = cell_rect.left();
                    let base_y = cell_rect.top();

                    // Faux bold: offset the character 0.5px to the right and draw again
                    if cell.bold {
                        painter.text(
                            Pos2::new(base_x + 0.5, base_y),
                            egui::Align2::LEFT_TOP,
                            cell.c,
                            font_id.clone(),
                            cell.fg,
                        );
                        painter.text(
                            Pos2::new(base_x, base_y),
                            egui::Align2::LEFT_TOP,
                            cell.c,
                            font_id.clone(),
                            cell.fg,
                        );
                    } else {
                        painter.text(
                            Pos2::new(base_x, base_y),
                            egui::Align2::LEFT_TOP,
                            cell.c,
                            font_id.clone(),
                            cell.fg,
                        );
                    }

                    if cell.underline {
                        let line_y = cell_rect.bottom() - 1.5;
                        let underline_rect = Rect::from_min_max(
                            Pos2::new(cell_rect.left(), line_y),
                            Pos2::new(cell_rect.right(), cell_rect.bottom() - 0.5),
                        );
                        painter.rect_filled(underline_rect, 0.0, cell.fg);
                    }
                }

                if cell.is_cursor && cursor_visible {
                    let overlay_color = match snapshot.cursor_shape {
                        CursorShape::Block | CursorShape::HollowBlock => {
                            cursor_color(CURSOR_BLOCK_ALPHA, snapshot.cursor_color)
                        }
                        CursorShape::Underline => {
                            let underline_rect = Rect::from_min_max(
                                Pos2::new(cell_rect.left(), cell_rect.bottom() - 2.0),
                                Pos2::new(cell_rect.right(), cell_rect.bottom()),
                            );
                            painter.rect_filled(
                                underline_rect,
                                0.0,
                                cursor_color(CURSOR_LINE_ALPHA, snapshot.cursor_color),
                            );
                            continue;
                        }
                        CursorShape::Beam => {
                            let beam_rect = Rect::from_min_max(
                                Pos2::new(cell_rect.left(), cell_rect.top()),
                                Pos2::new(cell_rect.left() + 2.0, cell_rect.bottom()),
                            );
                            painter.rect_filled(
                                beam_rect,
                                0.0,
                                cursor_color(CURSOR_LINE_ALPHA, snapshot.cursor_color),
                            );
                            continue;
                        }
                        CursorShape::Hidden => continue,
                    };
                    painter.rect_filled(cell_rect, 0.0, overlay_color);
                }
            }
        }
    }

    pub fn set_font_size(&mut self, font_size: f32) {
        self.font_size = font_size;
        self.cell_size = Self::estimate_cell_size(font_size);
        self.cell_size_measured = false;
    }

    fn estimate_cell_size(font_size: f32) -> Vec2 {
        Vec2::new(font_size * 0.6, font_size)
    }

    pub fn cell_size(&self) -> Vec2 {
        self.cell_size
    }

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
    fn test_estimate_fallback() {
        let renderer = TerminalRenderer::new(14.0);
        let size = renderer.cell_size();
        assert!(size.x > 5.0);
        assert!(size.y > 10.0);
    }

    #[test]
    fn test_cols_rows_for_rect() {
        let renderer = TerminalRenderer::new(14.0);
        let rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(800.0, 480.0));
        let (cols, rows) = renderer.cols_rows_for_rect(rect);
        assert!(cols > 0);
        assert!(rows > 0);
    }

    #[test]
    fn test_new_renderer() {
        let renderer = TerminalRenderer::new(12.0);
        assert_eq!(renderer.font_size, 12.0);
    }

    #[test]
    fn test_set_font_size_resets_measurement() {
        let mut renderer = TerminalRenderer::new(14.0);
        let original = renderer.cell_size();

        renderer.set_font_size(20.0);
        assert_eq!(renderer.font_size, 20.0);
        assert!(!renderer.cell_size_measured);

        let updated = renderer.cell_size();
        assert!(updated.x > original.x);
        assert!(updated.y > original.y);
    }
}
