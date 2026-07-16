use crate::state::GridSnapshot;
use alacritty_terminal::vte::ansi::CursorShape;
use egui::{Color32, FontFamily, FontId, Pos2, Rect, Ui, Vec2};

const CURSOR_BLOCK_ALPHA: u8 = 128;
const CURSOR_LINE_ALPHA: u8 = 200;

/// Extra vertical padding factor applied on top of measured glyph height so
/// descenders ("gypq") and combining marks don't clip, while still keeping
/// cells tight enough that box-drawing / block-element TUIs (LazyVim logo,
/// borders) tile without visible gaps. Ghostty/cmux use a similar tight
/// line height around JetBrains Mono.
const LINE_HEIGHT_PAD: f32 = 1.15;

fn cursor_color(alpha: u8, theme_color: Color32) -> Color32 {
    Color32::from_rgba_unmultiplied(theme_color.r(), theme_color.g(), theme_color.b(), alpha)
}

/// True for Unicode block elements that should be painted as geometry so they
/// fill the cell edge-to-edge (critical for LazyVim / ASCII-art logos).
fn is_block_element(c: char) -> bool {
    matches!(c, '\u{2580}'..='\u{259F}')
}

/// True for Private Use Area / Nerd Font icon ranges that look best
/// centered in the cell (devicons, codicons, material, etc.).
fn is_nerd_icon(c: char) -> bool {
    let cp = c as u32;
    matches!(
        cp,
        0xE000..=0xF8FF // BMP PUA (most Nerd Font icons live here)
            | 0xF0000..=0xFFFFD // Supplementary PUA-A
            | 0x100000..=0x10FFFD // Supplementary PUA-B
            | 0x23FB..=0x23FE // power symbols
            | 0x2665 // heart
            | 0x26A1 // high voltage
            | 0x2B58 // heavy circle
    )
}

/// Paint a Unicode block element as filled rectangles that exactly cover the
/// cell (or fractions of it). Returns `true` if handled.
fn paint_block_element(painter: &egui::Painter, cell: Rect, c: char, fg: Color32) -> bool {
    let w = cell.width();
    let h = cell.height();
    let left = cell.left();
    let top = cell.top();

    // Helper: fill a sub-rect of the cell given fractional x0,y0,x1,y1 in 0..=1.
    let fill = |x0: f32, y0: f32, x1: f32, y1: f32| {
        let r = Rect::from_min_max(
            Pos2::new(left + x0 * w, top + y0 * h),
            Pos2::new(left + x1 * w, top + y1 * h),
        );
        painter.rect_filled(r, 0.0, fg);
    };

    match c {
        // Upper half block
        '\u{2580}' => fill(0.0, 0.0, 1.0, 0.5),
        // Lower N/8 blocks
        '\u{2581}' => fill(0.0, 7.0 / 8.0, 1.0, 1.0), // 1/8
        '\u{2582}' => fill(0.0, 6.0 / 8.0, 1.0, 1.0), // 1/4
        '\u{2583}' => fill(0.0, 5.0 / 8.0, 1.0, 1.0), // 3/8
        '\u{2584}' => fill(0.0, 0.5, 1.0, 1.0),       // lower half
        '\u{2585}' => fill(0.0, 3.0 / 8.0, 1.0, 1.0), // 5/8
        '\u{2586}' => fill(0.0, 2.0 / 8.0, 1.0, 1.0), // 3/4
        '\u{2587}' => fill(0.0, 1.0 / 8.0, 1.0, 1.0), // 7/8
        '\u{2588}' => fill(0.0, 0.0, 1.0, 1.0),       // full block
        // Left N/8 blocks
        '\u{2589}' => fill(0.0, 0.0, 7.0 / 8.0, 1.0),
        '\u{258A}' => fill(0.0, 0.0, 6.0 / 8.0, 1.0),
        '\u{258B}' => fill(0.0, 0.0, 5.0 / 8.0, 1.0),
        '\u{258C}' => fill(0.0, 0.0, 0.5, 1.0), // left half
        '\u{258D}' => fill(0.0, 0.0, 3.0 / 8.0, 1.0),
        '\u{258E}' => fill(0.0, 0.0, 2.0 / 8.0, 1.0),
        '\u{258F}' => fill(0.0, 0.0, 1.0 / 8.0, 1.0),
        // Right half
        '\u{2590}' => fill(0.5, 0.0, 1.0, 1.0),
        // Light / medium / dark shade — approximate with alpha
        '\u{2591}' => {
            let c = Color32::from_rgba_unmultiplied(fg.r(), fg.g(), fg.b(), 64);
            painter.rect_filled(cell, 0.0, c);
        }
        '\u{2592}' => {
            let c = Color32::from_rgba_unmultiplied(fg.r(), fg.g(), fg.b(), 128);
            painter.rect_filled(cell, 0.0, c);
        }
        '\u{2593}' => {
            let c = Color32::from_rgba_unmultiplied(fg.r(), fg.g(), fg.b(), 192);
            painter.rect_filled(cell, 0.0, c);
        }
        // Quadrants
        '\u{2596}' => fill(0.0, 0.5, 0.5, 1.0), // lower left
        '\u{2597}' => fill(0.5, 0.5, 1.0, 1.0), // lower right
        '\u{2598}' => fill(0.0, 0.0, 0.5, 0.5), // upper left
        '\u{2599}' => {
            // upper left + lower left + lower right
            fill(0.0, 0.0, 0.5, 1.0);
            fill(0.5, 0.5, 1.0, 1.0);
        }
        '\u{259A}' => {
            // upper left + lower right
            fill(0.0, 0.0, 0.5, 0.5);
            fill(0.5, 0.5, 1.0, 1.0);
        }
        '\u{259B}' => {
            // upper left + upper right + lower left
            fill(0.0, 0.0, 1.0, 0.5);
            fill(0.0, 0.5, 0.5, 1.0);
        }
        '\u{259C}' => {
            // upper left + upper right + lower right
            fill(0.0, 0.0, 1.0, 0.5);
            fill(0.5, 0.5, 1.0, 1.0);
        }
        '\u{259D}' => fill(0.5, 0.0, 1.0, 0.5), // upper right
        '\u{259E}' => {
            // upper right + lower left
            fill(0.5, 0.0, 1.0, 0.5);
            fill(0.0, 0.5, 0.5, 1.0);
        }
        '\u{259F}' => {
            // upper right + lower left + lower right
            fill(0.5, 0.0, 1.0, 0.5);
            fill(0.0, 0.5, 1.0, 1.0);
        }
        _ => return false,
    }
    true
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
        let font_id = FontId::monospace(self.font_size);
        let glyph_width = ui.fonts(|f| {
            f.layout("M".to_string(), font_id.clone(), Color32::WHITE, f32::INFINITY).size().x
        });
        // Prefer a tight height derived from the font size rather than
        // egui's paragraph `row_height`, which includes extra leading that
        // leaves visible gaps between block-element rows (LazyVim logo).
        let row_height = self.font_size * LINE_HEIGHT_PAD;

        self.cell_size = Vec2::new(glyph_width.max(1.0), row_height.max(1.0));
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

        let font_regular = FontId::monospace(self.font_size);
        let font_bold = FontId::new(self.font_size, FontFamily::Name("JetBrainsMonoBold".into()));

        for row in 0..visible_rows {
            let mut col = 0_u16;
            while col < visible_cols {
                let cell = &snapshot.cells[row as usize][col as usize];

                // Double-width cells (CJK, many emoji, some ambiguous-width
                // symbols) span this column and the next. Widen this cell's
                // rect to cover both and skip the next column entirely — if
                // we painted it separately, its own opaque background fill
                // would land on top of (and clip) the right half of this
                // cell's glyph, since text isn't clipped per-cell.
                let span = if cell.wide && col + 1 < visible_cols { 2 } else { 1 };

                let cell_rect = Rect::from_min_size(
                    Pos2::new(rect.left() + col as f32 * cell_w, rect.top() + row as f32 * cell_h),
                    Vec2::new(cell_w * span as f32, cell_h),
                );

                painter.rect_filled(cell_rect, 0.0, cell.bg);

                if cell.c != ' ' {
                    // Block elements: geometric fill so TUIs tile cleanly.
                    if is_block_element(cell.c) {
                        paint_block_element(painter, cell_rect, cell.c, cell.fg);
                    } else {
                        let font_id =
                            if cell.bold { font_bold.clone() } else { font_regular.clone() };

                        // Layout once so we can center icons and vertically
                        // settle glyphs inside the tight cell.
                        let galley =
                            ui.fonts(|f| f.layout_no_wrap(cell.c.to_string(), font_id, cell.fg));
                        let gw = galley.size().x;
                        let gh = galley.size().y;

                        let x = if is_nerd_icon(cell.c) {
                            // Center nerd icons in the cell (Ghostty-style).
                            cell_rect.left() + (cell_rect.width() - gw) * 0.5
                        } else {
                            cell_rect.left()
                        };
                        // Vertically center so descenders/ascenders share the
                        // tight cell evenly (avoids top-biased "floating" glyphs).
                        let y = cell_rect.top() + (cell_rect.height() - gh) * 0.5;

                        painter.galley(Pos2::new(x, y), galley, cell.fg);
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
                    match snapshot.cursor_shape {
                        CursorShape::Block | CursorShape::HollowBlock => {
                            let overlay_color =
                                cursor_color(CURSOR_BLOCK_ALPHA, snapshot.cursor_color);
                            painter.rect_filled(cell_rect, 0.0, overlay_color);
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
                        }
                        CursorShape::Hidden => {}
                    }
                }

                col += span;
            }
        }
    }

    pub fn set_font_size(&mut self, font_size: f32) {
        self.font_size = font_size;
        self.cell_size = Self::estimate_cell_size(font_size);
        self.cell_size_measured = false;
    }

    fn estimate_cell_size(font_size: f32) -> Vec2 {
        // JetBrains Mono advance ≈ 0.6 × em; height uses the same pad factor
        // as the measured path so resize math stays stable before first paint.
        Vec2::new(font_size * 0.6, font_size * LINE_HEIGHT_PAD)
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

    #[test]
    fn test_block_elements_recognized() {
        assert!(is_block_element('█'));
        assert!(is_block_element('▄'));
        assert!(is_block_element('▀'));
        assert!(!is_block_element('A'));
    }

    #[test]
    fn test_nerd_icon_ranges() {
        assert!(is_nerd_icon('\u{f002}')); // search icon
        assert!(is_nerd_icon('\u{e0b0}')); // powerline
        assert!(!is_nerd_icon('A'));
    }
}
