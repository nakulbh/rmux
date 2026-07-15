# 05. Terminal renderer

Renderer draws snapshot to `egui`.

State knows cells. Renderer knows pixels.

File: `crates/rmux-terminal/src/renderer.rs`.

Imports show dependencies:

```rust
use crate::state::GridSnapshot;
use alacritty_terminal::vte::ansi::CursorShape;
use egui::{Color32, Pos2, Rect, Ui, Vec2};
```

Meaning:

| Import | Role |
|---|---|
| `GridSnapshot` | terminal cells to draw |
| `CursorShape` | block, beam, underline cursor |
| `egui::Ui` | drawing context |
| `Rect`, `Pos2`, `Vec2` | geometry |
| `Color32` | RGBA color |

Cursor alpha constants:

```rust
const CURSOR_BLOCK_ALPHA: u8 = 128;
const CURSOR_LINE_ALPHA: u8 = 200;

fn cursor_color(alpha: u8, theme_color: Color32) -> Color32 {
    Color32::from_rgba_unmultiplied(theme_color.r(), theme_color.g(), theme_color.b(), alpha)
}
```

Why alpha?

Cursor sits over text.

Partial opacity keeps glyph visible.

Renderer state:

```rust
pub struct TerminalRenderer {
    pub font_size: f32,
    cell_size: Vec2,
    cell_size_measured: bool,
}
```

Why cache cell size?

Terminal grid uses fixed cells.

Draw loop needs width and height many times.

Measuring fonts every frame costs work.

Constructor estimates first:

```rust
pub fn new(font_size: f32) -> Self {
    let cell_size = Self::estimate_cell_size(font_size);
    Self { font_size, cell_size, cell_size_measured: false }
}
```

Then real font measurement runs once:

```rust
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
```

Why `M`?

Monospace font. One glyph width represents cell width.

Draw starts with visibility guard:

```rust
pub fn draw(&mut self, ui: &mut Ui, rect: Rect, snapshot: &GridSnapshot, cursor_visible: bool) {
    if !ui.is_rect_visible(rect) {
        return;
    }

    self.ensure_cell_size_measured(ui);
```

Why return early?

If pane outside visible area, skip paint work.

Background fill handles extra space:

```rust
let used_height = snapshot.rows as f32 * cell_h;
if used_height < rect.height() {
    let fill = Rect::from_min_max(
        Pos2::new(rect.left(), rect.top() + used_height),
        Pos2::new(rect.right(), rect.bottom()),
    );
    painter.rect_filled(fill, 0.0, snapshot.terminal_bg);
}
```

Why fill unused rows?

Pane can be taller than terminal grid.

Empty space must still look like terminal background.

Visible grid bounds:

```rust
let visible_cols = ((rect.width() / cell_w).floor() as u16).min(snapshot.cols);
let visible_rows = ((rect.height() / cell_h).floor() as u16).min(snapshot.rows);
```

Why min?

Never draw past snapshot cells.

Never draw outside UI rect.

[Prev: Terminal state](04-terminal-state.md) | [Next: Terminal theme](06-terminal-theme.md)
