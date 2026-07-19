use crate::state::GridSnapshot;
use alacritty_terminal::vte::ansi::CursorShape;
use egui::{Color32, FontFamily, FontId, Pos2, Rect, Stroke, Ui, Vec2};

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

/// Unicode ranges where a missing font glyph should use geometry instead of tofu.
///
/// This is the **general** anti-tofu policy for the terminal: we do not need a
/// new `is_special_shape` arm for every symbol TUIs invent. If the full font
/// cascade (JetBrains → Nerd → system symbols) still lacks the codepoint, draw
/// a neutral geometric stand-in rather than a hollow □ replacement glyph.
fn is_symbol_range(c: char) -> bool {
    let cp = c as u32;
    matches!(
        cp,
        0x2190..=0x21FF // Arrows
            | 0x2200..=0x22FF // Mathematical Operators
            | 0x2300..=0x23FF // Miscellaneous Technical
            | 0x2460..=0x24FF // Enclosed Alphanumerics
            | 0x2500..=0x257F // Box Drawing (prefer font; fallback if absent)
            | 0x2580..=0x259F // Block Elements
            | 0x25A0..=0x25FF // Geometric Shapes
            | 0x2600..=0x26FF // Miscellaneous Symbols
            | 0x2700..=0x27BF // Dingbats
            | 0x27F0..=0x27FF // Supplemental Arrows-A
            | 0x2900..=0x297F // Supplemental Arrows-B
            | 0x2B00..=0x2BFF // Miscellaneous Symbols and Arrows
            | 0x1F300..=0x1F9FF // Misc Symbols and Pictographs / Supplemental (emoji-ish)
    )
}

/// Last-resort geometry when no font has the glyph.
///
/// Prefers the explicit special-shape painter when we know the codepoint;
/// otherwise draws a small diamond so the cell is never an empty tofu box.
fn paint_missing_symbol_fallback(painter: &egui::Painter, cell: Rect, c: char, fg: Color32) {
    if paint_special_shape(painter, cell, c, fg) {
        return;
    }
    // Neutral diamond stand-in — readable, not confusable with the □ tofu
    // that fonts emit for .notdef glyphs.
    let cx = cell.center().x;
    let cy = cell.center().y;
    let rx = cell.width() * 0.22;
    let ry = cell.height() * 0.28;
    painter.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(cx, cy - ry),
            Pos2::new(cx + rx, cy),
            Pos2::new(cx, cy + ry),
            Pos2::new(cx - rx, cy),
        ],
        fg,
        Stroke::NONE,
    ));
    // `c` reserved for future per-block heuristics (arrows vs dingbats).
    let _c = c;
}

/// Characters we draw as geometry instead of font glyphs.
///
/// Covers:
/// - Unicode block elements (LazyVim logo, progress bars)
/// - Media-control triangles (U+23F4–U+23FA) used by Claude Code etc.
///   JetBrains Mono and Symbols Nerd Font Mono do **not** include U+23F5,
///   so without this path those render as hollow □ tofu boxes.
/// - Common geometric triangles/squares (U+25B2–U+25C5, etc.) for a solid
///   cmux/Ghostty-like look independent of font coverage.
fn is_special_shape(c: char) -> bool {
    matches!(
        c,
        // Block elements
        '\u{2580}'..='\u{259F}'
        // Media controls: ◀ ▶ ▲ ▼ ⏸ ⏹ ⏺
        | '\u{23F4}'..='\u{23FA}'
        // Geometric shapes: triangles, pointers, squares, circles (subset)
        | '\u{25B2}'..='\u{25C5}'
        | '\u{25A0}'..='\u{25A3}'
        | '\u{25AA}'..='\u{25AB}'
        | '\u{25FB}'..='\u{25FE}'
        | '\u{25CF}' | '\u{25CB}' | '\u{25C9}' | '\u{25C6}' | '\u{25C7}' | '\u{25C8}' | '\u{25CE}'
        // Large squares (often used as TUI selection markers)
        | '\u{2B1B}' | '\u{2B1C}'
        // Ballot / check marks used by model pickers & TUIs (Claude, etc.)
        | '\u{2610}' | '\u{2611}' | '\u{2612}'
        | '\u{2713}' | '\u{2714}' | '\u{2717}' | '\u{2718}'
        // Powerline solid arrows (common in prompts)
        | '\u{E0B0}'..='\u{E0B3}'
    )
}

/// Draw a filled right-pointing triangle inset in `cell`.
fn fill_triangle_right(painter: &egui::Painter, cell: Rect, fg: Color32, pad: f32) {
    let x0 = cell.left() + cell.width() * pad;
    let x1 = cell.right() - cell.width() * pad;
    let y0 = cell.top() + cell.height() * pad;
    let y1 = cell.bottom() - cell.height() * pad;
    let mid_y = (y0 + y1) * 0.5;
    painter.add(egui::Shape::convex_polygon(
        vec![Pos2::new(x0, y0), Pos2::new(x1, mid_y), Pos2::new(x0, y1)],
        fg,
        Stroke::NONE,
    ));
}

fn fill_triangle_left(painter: &egui::Painter, cell: Rect, fg: Color32, pad: f32) {
    let x0 = cell.left() + cell.width() * pad;
    let x1 = cell.right() - cell.width() * pad;
    let y0 = cell.top() + cell.height() * pad;
    let y1 = cell.bottom() - cell.height() * pad;
    let mid_y = (y0 + y1) * 0.5;
    painter.add(egui::Shape::convex_polygon(
        vec![Pos2::new(x1, y0), Pos2::new(x0, mid_y), Pos2::new(x1, y1)],
        fg,
        Stroke::NONE,
    ));
}

fn fill_triangle_up(painter: &egui::Painter, cell: Rect, fg: Color32, pad: f32) {
    let x0 = cell.left() + cell.width() * pad;
    let x1 = cell.right() - cell.width() * pad;
    let y0 = cell.top() + cell.height() * pad;
    let y1 = cell.bottom() - cell.height() * pad;
    let mid_x = (x0 + x1) * 0.5;
    painter.add(egui::Shape::convex_polygon(
        vec![Pos2::new(mid_x, y0), Pos2::new(x1, y1), Pos2::new(x0, y1)],
        fg,
        Stroke::NONE,
    ));
}

fn fill_triangle_down(painter: &egui::Painter, cell: Rect, fg: Color32, pad: f32) {
    let x0 = cell.left() + cell.width() * pad;
    let x1 = cell.right() - cell.width() * pad;
    let y0 = cell.top() + cell.height() * pad;
    let y1 = cell.bottom() - cell.height() * pad;
    let mid_x = (x0 + x1) * 0.5;
    painter.add(egui::Shape::convex_polygon(
        vec![Pos2::new(x0, y0), Pos2::new(x1, y0), Pos2::new(mid_x, y1)],
        fg,
        Stroke::NONE,
    ));
}

fn fill_circle(painter: &egui::Painter, cell: Rect, fg: Color32, radius_frac: f32) {
    let r = cell.width().min(cell.height()) * radius_frac * 0.5;
    painter.circle_filled(cell.center(), r, fg);
}

fn stroke_circle(
    painter: &egui::Painter,
    cell: Rect,
    fg: Color32,
    radius_frac: f32,
    thickness: f32,
) {
    let r = cell.width().min(cell.height()) * radius_frac * 0.5;
    painter.circle_stroke(cell.center(), r, Stroke::new(thickness, fg));
}

/// Paint block elements, media controls, and geometric shapes as geometry so
/// they match Ghostty/cmux (solid triangles, edge-to-edge blocks) even when
/// the loaded fonts lack the codepoint. Returns `true` if handled.
fn paint_special_shape(painter: &egui::Painter, cell: Rect, c: char, fg: Color32) -> bool {
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
        // ── Block elements (U+2580–U+259F) ────────────────────────────
        '\u{2580}' => fill(0.0, 0.0, 1.0, 0.5),
        '\u{2581}' => fill(0.0, 7.0 / 8.0, 1.0, 1.0),
        '\u{2582}' => fill(0.0, 6.0 / 8.0, 1.0, 1.0),
        '\u{2583}' => fill(0.0, 5.0 / 8.0, 1.0, 1.0),
        '\u{2584}' => fill(0.0, 0.5, 1.0, 1.0),
        '\u{2585}' => fill(0.0, 3.0 / 8.0, 1.0, 1.0),
        '\u{2586}' => fill(0.0, 2.0 / 8.0, 1.0, 1.0),
        '\u{2587}' => fill(0.0, 1.0 / 8.0, 1.0, 1.0),
        '\u{2588}' => fill(0.0, 0.0, 1.0, 1.0),
        '\u{2589}' => fill(0.0, 0.0, 7.0 / 8.0, 1.0),
        '\u{258A}' => fill(0.0, 0.0, 6.0 / 8.0, 1.0),
        '\u{258B}' => fill(0.0, 0.0, 5.0 / 8.0, 1.0),
        '\u{258C}' => fill(0.0, 0.0, 0.5, 1.0),
        '\u{258D}' => fill(0.0, 0.0, 3.0 / 8.0, 1.0),
        '\u{258E}' => fill(0.0, 0.0, 2.0 / 8.0, 1.0),
        '\u{258F}' => fill(0.0, 0.0, 1.0 / 8.0, 1.0),
        '\u{2590}' => fill(0.5, 0.0, 1.0, 1.0),
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
        '\u{2596}' => fill(0.0, 0.5, 0.5, 1.0),
        '\u{2597}' => fill(0.5, 0.5, 1.0, 1.0),
        '\u{2598}' => fill(0.0, 0.0, 0.5, 0.5),
        '\u{2599}' => {
            fill(0.0, 0.0, 0.5, 1.0);
            fill(0.5, 0.5, 1.0, 1.0);
        }
        '\u{259A}' => {
            fill(0.0, 0.0, 0.5, 0.5);
            fill(0.5, 0.5, 1.0, 1.0);
        }
        '\u{259B}' => {
            fill(0.0, 0.0, 1.0, 0.5);
            fill(0.0, 0.5, 0.5, 1.0);
        }
        '\u{259C}' => {
            fill(0.0, 0.0, 1.0, 0.5);
            fill(0.5, 0.5, 1.0, 1.0);
        }
        '\u{259D}' => fill(0.5, 0.0, 1.0, 0.5),
        '\u{259E}' => {
            fill(0.5, 0.0, 1.0, 0.5);
            fill(0.0, 0.5, 0.5, 1.0);
        }
        '\u{259F}' => {
            fill(0.5, 0.0, 1.0, 0.5);
            fill(0.0, 0.5, 1.0, 1.0);
        }

        // ── Media controls (often missing from coding fonts) ──────────
        // U+23F4 BLACK MEDIUM LEFT-POINTING TRIANGLE
        '\u{23F4}' => fill_triangle_left(painter, cell, fg, 0.18),
        // U+23F5 BLACK MEDIUM RIGHT-POINTING TRIANGLE  ← Claude Code play icon
        '\u{23F5}' => fill_triangle_right(painter, cell, fg, 0.18),
        // U+23F6 / U+23F7 up / down
        '\u{23F6}' => fill_triangle_up(painter, cell, fg, 0.18),
        '\u{23F7}' => fill_triangle_down(painter, cell, fg, 0.18),
        // U+23F8 DOUBLE VERTICAL BAR (pause)
        '\u{23F8}' => {
            let bar_w = w * 0.14;
            let gap = w * 0.12;
            let x_mid = cell.center().x;
            let y0 = top + h * 0.18;
            let y1 = top + h * 0.82;
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(x_mid - gap - bar_w, y0), Pos2::new(x_mid - gap, y1)),
                0.0,
                fg,
            );
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(x_mid + gap, y0), Pos2::new(x_mid + gap + bar_w, y1)),
                0.0,
                fg,
            );
        }
        // U+23F9 BLACK SQUARE FOR STOP
        '\u{23F9}' => fill(0.22, 0.22, 0.78, 0.78),
        // U+23FA BLACK CIRCLE FOR RECORD
        '\u{23FA}' => fill_circle(painter, cell, fg, 0.55),

        // ── Geometric shapes ──────────────────────────────────────────
        // Black / white triangles and pointers
        '\u{25B2}' => fill_triangle_up(painter, cell, fg, 0.12), // ▲
        '\u{25B3}' => {
            // △ outline approx: draw smaller + leave hole is hard; use thin stroke triangle
            fill_triangle_up(painter, cell, fg, 0.12);
        }
        '\u{25B4}' => fill_triangle_up(painter, cell, fg, 0.22), // ▴ small
        '\u{25B5}' => fill_triangle_up(painter, cell, fg, 0.22),
        '\u{25B6}' => fill_triangle_right(painter, cell, fg, 0.12), // ▶
        '\u{25B7}' => fill_triangle_right(painter, cell, fg, 0.12), // ▷
        '\u{25B8}' => fill_triangle_right(painter, cell, fg, 0.22), // ▸
        '\u{25B9}' => fill_triangle_right(painter, cell, fg, 0.22), // ▹
        '\u{25BA}' => fill_triangle_right(painter, cell, fg, 0.10), // ►
        '\u{25BB}' => fill_triangle_right(painter, cell, fg, 0.10),
        '\u{25BC}' => fill_triangle_down(painter, cell, fg, 0.12), // ▼
        '\u{25BD}' => fill_triangle_down(painter, cell, fg, 0.12),
        '\u{25BE}' => fill_triangle_down(painter, cell, fg, 0.22),
        '\u{25BF}' => fill_triangle_down(painter, cell, fg, 0.22),
        '\u{25C0}' => fill_triangle_left(painter, cell, fg, 0.12), // ◀
        '\u{25C1}' => fill_triangle_left(painter, cell, fg, 0.12),
        '\u{25C2}' => fill_triangle_left(painter, cell, fg, 0.22),
        '\u{25C3}' => fill_triangle_left(painter, cell, fg, 0.22),
        '\u{25C4}' => fill_triangle_left(painter, cell, fg, 0.10), // ◄
        '\u{25C5}' => fill_triangle_left(painter, cell, fg, 0.10),

        // Squares
        '\u{25A0}' | '\u{25FE}' | '\u{2B1B}' => fill(0.15, 0.15, 0.85, 0.85), // ■ ◾ ⬛
        '\u{25A1}' | '\u{25FD}' | '\u{2B1C}' | '\u{2610}' => {
            // □ / white medium square / ballot box — hollow frame (TUIs use these
            // as selection markers; font glyphs often tofu without this path).
            let inset = Rect::from_min_max(
                Pos2::new(left + w * 0.18, top + h * 0.18),
                Pos2::new(left + w * 0.82, top + h * 0.82),
            );
            painter.rect_stroke(inset, 0.0, Stroke::new(1.6_f32, fg), egui::StrokeKind::Inside);
        }
        '\u{25A2}' => {
            // ▢ white square with rounded corners
            let inset = Rect::from_min_max(
                Pos2::new(left + w * 0.18, top + h * 0.18),
                Pos2::new(left + w * 0.82, top + h * 0.82),
            );
            painter.rect_stroke(inset, 2.0, Stroke::new(1.5_f32, fg), egui::StrokeKind::Inside);
        }
        '\u{25A3}' | '\u{2611}' => {
            // ▣ / ☑ — filled frame with inner mark
            let inset = Rect::from_min_max(
                Pos2::new(left + w * 0.18, top + h * 0.18),
                Pos2::new(left + w * 0.82, top + h * 0.82),
            );
            painter.rect_stroke(inset, 0.0, Stroke::new(1.5_f32, fg), egui::StrokeKind::Inside);
            // Check stroke
            let x0 = left + w * 0.30;
            let y0 = top + h * 0.52;
            let x1 = left + w * 0.45;
            let y1 = top + h * 0.70;
            let x2 = left + w * 0.72;
            let y2 = top + h * 0.32;
            painter.line_segment([Pos2::new(x0, y0), Pos2::new(x1, y1)], Stroke::new(1.6_f32, fg));
            painter.line_segment([Pos2::new(x1, y1), Pos2::new(x2, y2)], Stroke::new(1.6_f32, fg));
        }
        '\u{25FB}' | '\u{25FC}' => fill(0.22, 0.22, 0.78, 0.78), // ◻/◼ medium
        '\u{25AA}' => fill(0.30, 0.30, 0.70, 0.70),              // ▪
        '\u{25AB}' => {
            let inset = Rect::from_min_max(
                Pos2::new(left + w * 0.30, top + h * 0.30),
                Pos2::new(left + w * 0.70, top + h * 0.70),
            );
            painter.rect_stroke(inset, 0.0, Stroke::new(1.2_f32, fg), egui::StrokeKind::Inside);
        }
        '\u{2713}' | '\u{2714}' => {
            // ✓ ✔ check marks
            let x0 = left + w * 0.22;
            let y0 = top + h * 0.52;
            let x1 = left + w * 0.42;
            let y1 = top + h * 0.72;
            let x2 = left + w * 0.78;
            let y2 = top + h * 0.28;
            let sw = if c == '\u{2714}' { 2.0_f32 } else { 1.6_f32 };
            painter.line_segment([Pos2::new(x0, y0), Pos2::new(x1, y1)], Stroke::new(sw, fg));
            painter.line_segment([Pos2::new(x1, y1), Pos2::new(x2, y2)], Stroke::new(sw, fg));
        }
        '\u{2717}' | '\u{2718}' | '\u{2612}' => {
            // ✗ ✘ ☒ — X mark
            let pad = 0.25_f32;
            let x0 = left + w * pad;
            let y0 = top + h * pad;
            let x1 = left + w * (1.0 - pad);
            let y1 = top + h * (1.0 - pad);
            painter.line_segment([Pos2::new(x0, y0), Pos2::new(x1, y1)], Stroke::new(1.6_f32, fg));
            painter.line_segment([Pos2::new(x1, y0), Pos2::new(x0, y1)], Stroke::new(1.6_f32, fg));
        }

        // Circles / diamonds
        '\u{25CF}' => fill_circle(painter, cell, fg, 0.62), // ●
        '\u{25CB}' => stroke_circle(painter, cell, fg, 0.62, 1.5_f32), // ○
        '\u{25C9}' => {
            // ◉ fisheye
            stroke_circle(painter, cell, fg, 0.70, 1.5_f32);
            fill_circle(painter, cell, fg, 0.35);
        }
        '\u{25CE}' => {
            // ◎ bullseye
            stroke_circle(painter, cell, fg, 0.70, 1.4_f32);
            stroke_circle(painter, cell, fg, 0.40, 1.2_f32);
        }
        '\u{25C6}' => {
            // ◆ diamond
            let cx = cell.center().x;
            let cy = cell.center().y;
            let rx = w * 0.32;
            let ry = h * 0.38;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(cx, cy - ry),
                    Pos2::new(cx + rx, cy),
                    Pos2::new(cx, cy + ry),
                    Pos2::new(cx - rx, cy),
                ],
                fg,
                Stroke::NONE,
            ));
        }
        '\u{25C7}' => {
            // ◇ hollow diamond
            let cx = cell.center().x;
            let cy = cell.center().y;
            let rx = w * 0.32;
            let ry = h * 0.38;
            painter.add(egui::Shape::closed_line(
                vec![
                    Pos2::new(cx, cy - ry),
                    Pos2::new(cx + rx, cy),
                    Pos2::new(cx, cy + ry),
                    Pos2::new(cx - rx, cy),
                ],
                Stroke::new(1.4_f32, fg),
            ));
        }
        '\u{25C8}' => {
            // ◈ diamond with center
            let cx = cell.center().x;
            let cy = cell.center().y;
            let rx = w * 0.32;
            let ry = h * 0.38;
            painter.add(egui::Shape::closed_line(
                vec![
                    Pos2::new(cx, cy - ry),
                    Pos2::new(cx + rx, cy),
                    Pos2::new(cx, cy + ry),
                    Pos2::new(cx - rx, cy),
                ],
                Stroke::new(1.3_f32, fg),
            ));
            fill_circle(painter, cell, fg, 0.22);
        }

        // ── Powerline solid triangles (shell prompts) ─────────────────
        '\u{E0B0}' => fill_triangle_right(painter, cell, fg, 0.0), // 
        '\u{E0B2}' => fill_triangle_left(painter, cell, fg, 0.0),  // 
        '\u{E0B1}' | '\u{E0B3}' => {
            // hollow powerline — approximate as stroke triangle
            // fall through to font if possible; paint thin version
            if c == '\u{E0B1}' {
                fill_triangle_right(painter, cell, fg, 0.05);
            } else {
                fill_triangle_left(painter, cell, fg, 0.05);
            }
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
                    if is_special_shape(cell.c) {
                        // Explicit geometry for TUI shapes (Ghostty/cmux solid look).
                        paint_special_shape(painter, cell_rect, cell.c, cell.fg);
                    } else {
                        let font_id =
                            if cell.bold { font_bold.clone() } else { font_regular.clone() };

                        // General anti-tofu: if *no* font in the cascade has
                        // this codepoint, don't paint a hollow □ replacement —
                        // use geometry for symbol ranges instead.
                        let has_glyph = ui.fonts(|f| f.has_glyph(&font_id, cell.c));
                        if !has_glyph && is_symbol_range(cell.c) {
                            paint_missing_symbol_fallback(painter, cell_rect, cell.c, cell.fg);
                        } else {
                            let galley = ui
                                .fonts(|f| f.layout_no_wrap(cell.c.to_string(), font_id, cell.fg));
                            let gw = galley.size().x;
                            let gh = galley.size().y;

                            let x = if is_nerd_icon(cell.c) {
                                // Center nerd icons in the cell (Ghostty-style).
                                cell_rect.left() + (cell_rect.width() - gw) * 0.5
                            } else {
                                cell_rect.left()
                            };
                            // Slight top bias keeps the Latin baseline closer to
                            // native terminal metrics than pure vertical center,
                            // which made text look "floaty" vs cmux.
                            let y = cell_rect.top() + (cell_rect.height() - gh) * 0.35;

                            painter.galley(Pos2::new(x, y), galley, cell.fg);
                        }
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
        assert!(is_special_shape('█'));
        assert!(is_special_shape('▄'));
        assert!(is_special_shape('▀'));
        assert!(!is_special_shape('A'));
    }

    #[test]
    fn test_symbol_range_covers_common_tofu_sources() {
        // Geometric / dingbat / technical — general anti-tofu policy.
        assert!(is_symbol_range('⎇')); // U+2387 branch
        assert!(is_symbol_range('□'));
        assert!(is_symbol_range('✓'));
        assert!(is_symbol_range('→'));
        assert!(is_symbol_range('★'));
        // Normal text must still go through fonts.
        assert!(!is_symbol_range('A'));
        assert!(!is_symbol_range('中'));
        assert!(!is_symbol_range(' '));
    }

    #[test]
    fn test_media_play_triangle_is_special() {
        // Claude Code status line uses U+23F5 twice — must not fall through
        // to missing-glyph tofu boxes.
        assert!(is_special_shape('\u{23F5}'));
        assert!(is_special_shape('\u{23F4}'));
        assert!(is_special_shape('\u{23F8}'));
        assert!(is_special_shape('▶'));
        assert!(is_special_shape('●'));
    }

    #[test]
    fn test_nerd_icon_ranges() {
        assert!(is_nerd_icon('\u{f002}')); // search icon
        assert!(is_nerd_icon('\u{e0b0}')); // powerline
        assert!(!is_nerd_icon('A'));
    }
}
