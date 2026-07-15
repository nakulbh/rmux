# 04. Terminal state

Backend gives bytes.

State turns bytes into screen cells.

File: `crates/rmux-terminal/src/state.rs`.

Top comment:

```rust
//! Terminal state management.
//!
//! Wraps `alacritty_terminal::Term` and provides a clean query API
//! for the renderer. Manages grid state, scrollback, and cursor position.
```

Why wrapper?

`alacritty_terminal` is powerful but deep.

rmux needs smaller API for renderer.

Size type implements terminal dimensions:

```rust
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
```

Why `total_lines` bigger than `screen_lines`?

Visible screen is rows.

Scrollback is extra history above screen.

Together they form terminal buffer.

Main state struct:

```rust
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
    /// Terminal color theme (ANSI palette, cursor, selection, fg/bg).
    pub theme: TerminalTheme,
}
```

Field map:

| Field | Job |
|---|---|
| `term` | holds parsed terminal grid |
| `processor` | parses escape sequences |
| `cols`, `rows` | current size |
| `scrollback_limit` | history cap |
| `theme` | resolves colors |

Snapshot type:

```rust
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
}
```

Why snapshot?

Renderer needs stable owned copy.

Borrowing live terminal while drawing gets messy.

Snapshot avoids borrow fight.

One cell:

```rust
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
}
```

Terminal is not text lines only.

Each character has colors and style.

Constructor creates Alacritty term:

```rust
pub fn new(cols: u16, rows: u16, scrollback_limit: usize) -> Self {
    let config = Config { scrolling_history: scrollback_limit, ..Config::default() };

    let dimensions =
        TermDimensions { cols: cols as usize, rows: rows as usize, scrollback_limit };

    let term = alacritty_terminal::term::Term::new(config, &dimensions, VoidListener);
```

Why `VoidListener`?

rmux does not need Alacritty event callbacks here.

It pulls snapshots instead.

[Prev: Terminal backend](03-terminal-backend.md) | [Next: Terminal renderer](05-terminal-renderer.md)
