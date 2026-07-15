# 09. Workspace splits

Split tree describes pane layout.

Leaves are panes. Interior nodes divide space.

File: `crates/rmux-app/src/workspace/splits.rs`.

Top comment:

```rust
//! Pane tree model, the recursive tree structure that describes split layouts.
//!
//! Each workspace contains a pane tree. Interior nodes are `Split` nodes
//! that divide their area among children. Leaf nodes contain terminal panes.
```

Why tree?

Splits nest.

Example:

```text
root vertical split
  left pane
  right horizontal split
    top pane
    bottom pane
```

Errors:

```rust
pub enum PaneTreeError {
    #[error("Pane not found: {0:?}")]
    PaneNotFound(PaneId),
    #[error("Split not found: {0:?}")]
    SplitNotFound(SplitId),
    #[error("Cannot close the last pane")]
    CannotCloseLastPane,
    #[error("Operation requires a leaf node")]
    NotALeaf,
    #[error("Invalid child index: {0}")]
    InvalidChildIndex(usize),
}
```

Why explicit errors?

UI can show correct message.

Closing last pane is user action problem, not crash.

IDs:

```rust
pub struct PaneId(pub u64);

pub struct SplitId(pub u64);
```

Why IDs instead of indexes?

Tree changes shape.

Indexes shift after split or close.

Stable IDs survive layout edits.

Split direction:

```rust
pub enum SplitDirection {
    Horizontal,
    Vertical,
}
```

Meaning:

| Direction | Effect |
|---|---|
| `Horizontal` | split left and right |
| `Vertical` | split top and bottom |

Spatial focus:

```rust
pub enum SpatialDirection {
    Left,
    Right,
    Up,
    Down,
}
```

Used for keyboard focus movement.

Pane rectangle:

```rust
pub struct PaneRect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}
```

Coordinates are normalized `0.0..=1.0`.

Why normalized?

Layout math works before knowing pixel size.

UI converts to pixels later.

Helpers:

```rust
pub fn unit() -> Self {
    Self { min_x: 0.0, min_y: 0.0, max_x: 1.0, max_y: 1.0 }
}

pub fn center(&self) -> (f32, f32) {
    ((self.min_x + self.max_x) / 2.0, (self.min_y + self.max_y) / 2.0)
}
```

`unit()` means whole workspace.

`center()` helps find nearest pane in direction.

Pane node starts like this:

```rust
pub enum PaneNode {
    Leaf {
        id: PaneId,
        /// Legacy slot kept for backward compat with `find_terminal_mut` /
        /// `set_terminal`. Future waves will migrate these to operate on
        /// `surfaces` directly.
        terminal: Box<Option<TerminalPane>>,
        /// Index into `surfaces` of the focused surface. Stays 0 when
        /// `surfaces` is empty (the default for an uninitialized leaf).
        active_surface: usize,
        /// The list of surfaces (tabs) in this leaf. May be empty for an
        /// uninitialized leaf; in that case `terminal` is the source of
        /// truth and `terminal_count()` reports 1.
        surfaces: Vec<Surface>,
    },
```

Why `Leaf` has surfaces?

One split pane can hold tabs.

Active surface is selected tab inside pane.

Big idea:

```text
Workspace -> PaneNode tree -> Leaf -> Surface -> TerminalPane or BrowserPane
```

[Prev: Input mapper](08-input-mapper.md) | [Next: Workspace model](10-workspace-model.md)
