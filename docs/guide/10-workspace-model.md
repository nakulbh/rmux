# 10. Workspace model

Workspace groups pane tree plus metadata.

File: `crates/rmux-app/src/workspace/model.rs`.

Top comment:

```rust
//! Workspace model, groups a pane tree with metadata.
//!
//! A workspace represents a collection of terminal panes arranged in a split layout.
//! Users can switch between workspaces via the sidebar.
```

Why workspace?

User may run separate projects.

Each workspace keeps own panes, status, git info, ports, focus.

Initial terminal size:

```rust
/// Initial size for freshly spawned terminal surfaces (columns).
const INITIAL_COLS: u16 = 80;
/// Initial size for freshly spawned terminal surfaces (rows).
const INITIAL_ROWS: u16 = 24;
```

Why 80x24?

Classic terminal default.

Good safe size before UI measures real pane.

Workspace errors:

```rust
pub enum WorkspaceError {
    /// The active pane is missing or not a `Leaf` (e.g. it's a
    /// `Browser` node, which doesn't host surfaces).
    #[error("No active leaf pane selected")]
    NoActivePane,
    /// The given surface index is out of range for the active leaf.
    #[error("Invalid surface index: {0}")]
    InvalidSurfaceIndex(usize),
    /// The requested operation would leave the active leaf with zero
    /// surfaces. A leaf must always have at least one tab.
    #[error("Cannot close the last surface")]
    CannotCloseLastSurface,
}
```

Why separate from `PaneTreeError`?

Tree errors describe layout problems.

Workspace errors describe tab and active pane problems.

Workspace ID:

```rust
pub struct WorkspaceId(pub u64);
```

Stable ID for sidebar and commands.

Name can change. ID should not.

Focus commands:

```rust
pub enum FocusDirection {
    /// Move focus to the next pane in DFS order.
    Next,
    /// Move focus to the previous pane in DFS order.
    Previous,
    /// Move focus spatially by delta (dx, dy).
    Spatial { dx: i32, dy: i32 },
}
```

Two focus styles.

Next and Previous follow tree order.

Spatial follows screen direction.

Workspace struct:

```rust
pub struct Workspace {
    /// Unique identifier for the workspace.
    pub id: WorkspaceId,
    /// Display name shown in the sidebar.
    pub name: String,
    /// The root of the pane tree.
    pub root: PaneNode,
    /// The currently focused (active) pane.
    pub active_pane: PaneId,
    /// Status text shown in the sidebar tab (set via `sidebar.set_status`).
    pub status: Option<String>,
    /// Progress in `0.0..=1.0` shown as a bar in the sidebar tab
    /// (set via `sidebar.set_progress`).
    pub progress: Option<f32>,
    /// Current git branch name for the workspace's directory, if known.
    /// Displayed on the sidebar card (set via `update_git_info`).
    pub git_branch: Option<String>,
}
```

Field map:

| Field | Job |
|---|---|
| `id` | stable identity |
| `name` | sidebar label |
| `root` | split tree |
| `active_pane` | keyboard target |
| `status` | sidebar text |
| `progress` | progress bar |
| `git_branch` | project context |

More metadata from same struct:

```rust
/// Short git status summary (e.g. "clean", "modified", "untracked").
/// Displayed on the sidebar card (set via `update_git_info`).
pub git_status: Option<String>,
/// TCP ports currently listening in this workspace's directory.
/// Displayed as badges on the sidebar card (set via `update_ports`).
pub ports: Vec<u16>,
/// When `Some`, only this pane is rendered (zoomed/maximized mode).
```

Why store git and ports in workspace?

Sidebar can show useful project state without scanning every frame.

Why `Option` for many fields?

Unknown is valid.

No git repo. No progress. No status.

Beginner mental model:

```text
WorkspaceManager owns many Workspace values
Workspace owns PaneNode root
PaneNode owns panes and surfaces
TerminalPane owns backend, state, renderer
```

When key typed, active workspace and active pane decide where bytes go.

[Prev: Workspace splits](09-workspace-splits.md) | [Next: Back to intro](00-intro.md)
