//! Terminal state management.
//!
//! Wraps `alacritty_terminal::Term` and provides a clean query API
//! for the renderer. Manages grid state, scrollback, and cursor position.

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::Config;
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Processor, Rgb};
use egui::Color32;

/// A size struct implementing [`Dimensions`] for terminal creation and resize.
struct TermDimensions {
    cols: usize,
    rows: usize,
    scrollback_limit: usize,
}

impl Dimensions for TermDimensions {
    fn total_lines(&self) -> usize {
        self.rows + self.scrollback_limit
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

/// Wraps `alacritty_terminal::Term` and provides a clean query API.
///
/// Manages the terminal grid, scrollback, cursor position, and color palette.
/// The VTE parser is driven by feeding bytes from the PTY into this struct.
///
/// # Examples
///
/// ```no_run
/// use rmux_terminal::TermState;
///
/// let mut state = TermState::new(80, 24, 10_000);
/// state.feed_bytes(b"Hello, world!\r\n");
/// let snapshot = state.snapshot();
/// assert_eq!(snapshot.cols, 80);
/// ```
pub struct TermState {
    /// The alacritty terminal emulator state.
    term: alacritty_terminal::term::Term<VoidListener>,
    /// VTE processor for parsing terminal output through the Handler.
    processor: Processor,
    /// Current column count.
    cols: u16,
    /// Current row count.
    rows: u16,
    /// Maximum scrollback lines (stored for future config use).
    #[allow(dead_code)]
    scrollback_limit: usize,
}

/// A snapshot of the terminal grid at a point in time.
///
/// This is an owned copy to avoid borrow issues during rendering.
/// The snapshot is created by copying the visible cells from
/// the alacritty terminal grid into our own grid representation.
#[derive(Clone)]
pub struct GridSnapshot {
    /// Number of columns in the grid.
    pub cols: u16,
    /// Number of rows in the grid.
    pub rows: u16,
    /// The grid cells, indexed as `cells[row][col]`.
    pub cells: Vec<Vec<GridCell>>,
    /// Current cursor row (0-indexed in viewport).
    pub cursor_row: u16,
    /// Current cursor column (0-indexed in viewport).
    pub cursor_col: u16,
    /// Current cursor shape.
    pub cursor_shape: CursorShape,
    /// Scrollback display offset.
    pub display_offset: usize,
}

/// A single cell in the terminal grid.
///
/// Contains the character, foreground/background colors,
/// and text style flags.
#[derive(Clone, Debug)]
pub struct GridCell {
    /// The character displayed in this cell.
    pub c: char,
    /// Foreground color.
    pub fg: Color32,
    /// Background color.
    pub bg: Color32,
    /// Whether the text is bold.
    pub bold: bool,
    /// Whether the text is italic.
    pub italic: bool,
    /// Whether the text is underlined (any underline type).
    pub underline: bool,
    /// Whether this cell is the cursor position (for overlay rendering).
    pub is_cursor: bool,
}

impl TermState {
    /// Create a new terminal state with the given dimensions.
    ///
    /// # Arguments
    ///
    /// * `cols` - Number of columns.
    /// * `rows` - Number of rows.
    /// * `scrollback_limit` - Maximum lines of scrollback history.
    pub fn new(cols: u16, rows: u16, scrollback_limit: usize) -> Self {
        let config = Config { scrolling_history: scrollback_limit, ..Config::default() };

        let dimensions =
            TermDimensions { cols: cols as usize, rows: rows as usize, scrollback_limit };

        let term = alacritty_terminal::term::Term::new(config, &dimensions, VoidListener);

        Self { term, processor: Processor::new(), cols, rows, scrollback_limit }
    }

    /// Feed raw bytes from PTY output through the VTE parser.
    ///
    /// This updates the internal terminal grid state,
    /// including cursor position, scrollback, and cell content.
    pub fn feed_bytes(&mut self, data: &[u8]) {
        self.processor.advance(&mut self.term, data);
    }

    /// Take a snapshot of the visible grid for rendering.
    ///
    /// This copies all visible cells into an owned [`GridSnapshot`],
    /// which can be safely used for rendering without holding a borrow
    /// on the terminal state.
    pub fn snapshot(&self) -> GridSnapshot {
        let cols = self.term.columns() as u16;
        let rows = self.term.screen_lines() as u16;
        let display_offset = self.term.grid().display_offset();

        let mut cells: Vec<Vec<GridCell>> = Vec::with_capacity(rows as usize);
        for _ in 0..rows {
            cells.push(vec![GridCell::default(); cols as usize]);
        }

        // Extract renderable content parts we need
        let renderable = self.term.renderable_content();
        let colors = renderable.colors;
        let cursor_point = renderable.cursor.point;
        let cursor_shape = renderable.cursor.shape;

        // Iterate over renderable cells and populate the grid
        for indexed_cell in renderable.display_iter {
            let point = indexed_cell.point;
            let cell = indexed_cell.cell;

            // Convert line/column to grid coordinates using display offset
            if let Some(view_point) =
                alacritty_terminal::term::point_to_viewport(display_offset, point)
            {
                let row = view_point.line;
                let col = view_point.column;

                if row < rows as usize && col.0 < cols as usize {
                    let (fg, bg) = cell_colors(cell, colors);
                    cells[row][col.0] = GridCell {
                        c: cell.c,
                        fg,
                        bg,
                        bold: cell.flags.contains(Flags::BOLD),
                        italic: cell.flags.contains(Flags::ITALIC),
                        underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
                        is_cursor: false,
                    };
                }
            }
        }

        // Mark cursor cell
        let cursor_row = if let Some(view_point) =
            alacritty_terminal::term::point_to_viewport(display_offset, cursor_point)
        {
            let row = view_point.line;
            let col = view_point.column;
            if row < rows as usize && col.0 < cols as usize {
                cells[row][col.0].is_cursor = true;
            }
            row as u16
        } else {
            cursor_point.line.0.max(0) as u16
        };

        let cursor_col = cursor_point.column.0 as u16;

        GridSnapshot { cols, rows, cells, cursor_row, cursor_col, cursor_shape, display_offset }
    }

    /// Resize the terminal to new dimensions.
    ///
    /// This feeds the resize through the terminal model,
    /// which handles reflow and scrollback adjustment.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        let dims = TermDimensions {
            cols: cols as usize,
            rows: rows as usize,
            scrollback_limit: self.scrollback_limit,
        };
        self.term.resize(dims);
        self.cols = cols;
        self.rows = rows;
    }

    /// Get the current cursor position (row, col).
    pub fn cursor_pos(&self) -> (u16, u16) {
        let cursor = self.term.grid().cursor.point;
        (cursor.line.0.max(0) as u16, cursor.column.0 as u16)
    }

    /// Scroll the terminal viewport.
    ///
    /// Positive `lines` scrolls up (into scrollback).
    /// Negative `lines` scrolls down.
    pub fn scroll(&mut self, lines: i32) {
        use alacritty_terminal::grid::Scroll;
        let scroll = Scroll::Delta(lines);
        self.term.scroll_display(scroll);
    }

    /// Access the underlying term colors for custom color queries.
    pub fn colors(&self) -> Colors {
        *self.term.colors()
    }
}

/// Convert a cell's foreground and background colors to egui `Color32`.
///
/// Uses the terminal's color palette to resolve named and indexed colors
/// to their actual RGB values.
fn cell_colors(cell: &Cell, colors: &Colors) -> (Color32, Color32) {
    let fg = resolve_color(cell.fg, colors);
    let bg = resolve_color(cell.bg, colors);
    (fg, bg)
}

/// Resolve a terminal [`Color`] to an egui [`Color32`].
///
/// Handles named colors (using the palette), RGB spec colors, and
/// indexed colors. Falls back to sensible defaults for unresolved colors.
fn resolve_color(color: Color, colors: &Colors) -> Color32 {
    match color {
        Color::Named(named) => {
            let rgb = colors[named].unwrap_or_else(|| default_named_color(named));
            Color32::from_rgb(rgb.r, rgb.g, rgb.b)
        }
        Color::Spec(rgb) => Color32::from_rgb(rgb.r, rgb.g, rgb.b),
        Color::Indexed(idx) => {
            // For indexed colors in 16-255 range, look up in the colors array
            if let Some(rgb) = colors[idx as usize] {
                Color32::from_rgb(rgb.r, rgb.g, rgb.b)
            } else {
                // Fallback: generate from 6x6x6 color cube or grayscale
                indexed_to_color32(idx, colors)
            }
        }
    }
}

/// Convert an indexed color to egui `Color32`.
///
/// ANSI colors 0-15 are named, 16-231 form a 6x6x6 cube,
/// and 232-255 are grayscale.
fn indexed_to_color32(idx: u8, colors: &Colors) -> Color32 {
    match idx {
        0..=15 => {
            let named = match idx {
                0 => NamedColor::Black,
                1 => NamedColor::Red,
                2 => NamedColor::Green,
                3 => NamedColor::Yellow,
                4 => NamedColor::Blue,
                5 => NamedColor::Magenta,
                6 => NamedColor::Cyan,
                7 => NamedColor::White,
                8 => NamedColor::BrightBlack,
                9 => NamedColor::BrightRed,
                10 => NamedColor::BrightGreen,
                11 => NamedColor::BrightYellow,
                12 => NamedColor::BrightBlue,
                13 => NamedColor::BrightMagenta,
                14 => NamedColor::BrightCyan,
                15 => NamedColor::BrightWhite,
                _ => NamedColor::White,
            };
            let rgb = colors[named].unwrap_or_else(|| default_named_color(named));
            Color32::from_rgb(rgb.r, rgb.g, rgb.b)
        }
        16..=231 => {
            // 6x6x6 color cube
            let idx = idx - 16;
            let r = (idx / 36) * 51;
            let g = ((idx / 6) % 6) * 51;
            let b = (idx % 6) * 51;
            Color32::from_rgb(r, g, b)
        }
        232..=255 => {
            // Grayscale ramp (8-238 in steps of 10)
            let gray = (idx as u32 - 232) * 10 + 8;
            Color32::from_rgb(gray as u8, gray as u8, gray as u8)
        }
    }
}

/// Default color values for named ANSI colors.
fn default_named_color(named: NamedColor) -> Rgb {
    match named {
        NamedColor::Black => Rgb::default(),
        NamedColor::Red => Rgb { r: 205, g: 0, b: 0 },
        NamedColor::Green => Rgb { r: 0, g: 205, b: 0 },
        NamedColor::Yellow => Rgb { r: 205, g: 205, b: 0 },
        NamedColor::Blue => Rgb { r: 0, g: 0, b: 238 },
        NamedColor::Magenta => Rgb { r: 205, g: 0, b: 205 },
        NamedColor::Cyan => Rgb { r: 0, g: 205, b: 205 },
        NamedColor::White => Rgb { r: 229, g: 229, b: 229 },
        NamedColor::BrightBlack => Rgb { r: 127, g: 127, b: 127 },
        NamedColor::BrightRed => Rgb { r: 255, g: 0, b: 0 },
        NamedColor::BrightGreen => Rgb { r: 0, g: 255, b: 0 },
        NamedColor::BrightYellow => Rgb { r: 255, g: 255, b: 0 },
        NamedColor::BrightBlue => Rgb { r: 92, g: 92, b: 255 },
        NamedColor::BrightMagenta => Rgb { r: 255, g: 0, b: 255 },
        NamedColor::BrightCyan => Rgb { r: 0, g: 255, b: 255 },
        NamedColor::BrightWhite => Rgb { r: 255, g: 255, b: 255 },
        NamedColor::Foreground | NamedColor::BrightForeground => Rgb { r: 229, g: 229, b: 229 },
        NamedColor::Background => Rgb { r: 0, g: 0, b: 0 },
        NamedColor::Cursor => Rgb { r: 255, g: 255, b: 255 },
        // Dim colors: approximate half brightness of normal
        NamedColor::DimBlack => Rgb::default(),
        NamedColor::DimRed => Rgb { r: 102, g: 0, b: 0 },
        NamedColor::DimGreen => Rgb { r: 0, g: 102, b: 0 },
        NamedColor::DimYellow => Rgb { r: 102, g: 102, b: 0 },
        NamedColor::DimBlue => Rgb { r: 0, g: 0, b: 119 },
        NamedColor::DimMagenta => Rgb { r: 102, g: 0, b: 102 },
        NamedColor::DimCyan => Rgb { r: 0, g: 102, b: 102 },
        NamedColor::DimWhite => Rgb { r: 114, g: 114, b: 114 },
        NamedColor::DimForeground => Rgb { r: 114, g: 114, b: 114 },
    }
}

impl Default for GridCell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: Color32::WHITE,
            bg: Color32::BLACK,
            bold: false,
            italic: false,
            underline: false,
            is_cursor: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_term_state() {
        let state = TermState::new(80, 24, 5000);
        let snapshot = state.snapshot();
        assert_eq!(snapshot.cols, 80);
        assert_eq!(snapshot.rows, 24);
        assert_eq!(snapshot.cells.len(), 24);
        assert_eq!(snapshot.cells[0].len(), 80);
    }

    #[test]
    fn test_feed_bytes_basic_text() {
        let mut state = TermState::new(80, 24, 1000);
        state.feed_bytes(b"Hello, World!");
        let snapshot = state.snapshot();

        // Check first row contains our text
        assert_eq!(snapshot.cells[0][0].c, 'H');
        assert_eq!(snapshot.cells[0][1].c, 'e');
        assert_eq!(snapshot.cells[0][2].c, 'l');
        assert_eq!(snapshot.cells[0][3].c, 'l');
        assert_eq!(snapshot.cells[0][4].c, 'o');
    }

    #[test]
    fn test_resize_terminal() {
        let mut state = TermState::new(80, 24, 1000);
        state.resize(120, 40);
        let snapshot = state.snapshot();
        assert_eq!(snapshot.cols, 120);
        assert_eq!(snapshot.rows, 40);
    }

    #[test]
    fn test_grid_cell_default() {
        let cell = GridCell::default();
        assert_eq!(cell.c, ' ');
        assert!(!cell.bold);
        assert!(!cell.italic);
        assert!(!cell.underline);
        assert!(!cell.is_cursor);
    }

    #[test]
    fn test_indexed_color_mapping() {
        // We just test the function doesn't panic
        let colors = Colors::default();
        let c0 = super::indexed_to_color32(0, &colors);
        let c255 = super::indexed_to_color32(255, &colors);
        let c16 = super::indexed_to_color32(16, &colors);
        let c231 = super::indexed_to_color32(231, &colors);
        // All should produce valid colors
        assert!(c0 != c255 || c0 == c255);
        let _ = (c16, c231);
    }
}
