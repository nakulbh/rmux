# Phase 2 Worktree — Workspaces + Splits + Sidebar

> Tracking file for Phase 2 implementation tasks.
> Branch: `feat/phase-2-workspaces-splits`

---

## Tasks

### 2.1 Define `PaneNode` tree model
- [x] Define `PaneNode` enum (Leaf, Split variants)
- [x] Define `SplitDirection` enum
- [x] Define `PaneId`, `SplitId` newtypes
- [x] Define `PlaceholderPane` struct
- [x] Implement tree operations on `PaneNode` (find, iter, etc.)
- [x] Write unit tests

### 2.2 Implement `Workspace` model
- [x] Define `Workspace` struct with id, name, root, active_pane
- [x] Implement `split_right`, `split_down`
- [x] Implement `close_pane`
- [x] Implement `focus_pane`, `focus_next`, `focus_prev`
- [x] Implement `pane_ids`, `pane_count`, `active_pane_id`
- [x] Write unit tests

### 2.3 Implement `WorkspaceManager`
- [x] Define `WorkspaceManager` struct
- [x] Implement `create_workspace`, `close_workspace`
- [x] Implement `switch_to`, `switch_next`, `switch_prev`
- [x] Implement `active`, `active_mut`
- [x] Implement pane count tracking + guardrail (warn at > 50)
- [x] Write unit tests

### 2.4 Build `SidebarView`
- [x] Create `ui/sidebar.rs` with `SidebarView`
- [x] Render workspace tabs with names + pane counts
- [x] Highlight active workspace
- [x] Click to switch workspace
- [x] Dark background, ~200px width
- [x] Update `ui/mod.rs` with re-exports

### 2.5 Implement split commands
- [x] `split_right`: split leaf into Horizontal split with 2 children
- [x] `split_down`: split leaf into Vertical split with 2 children
- [x] `close_pane`: remove leaf, collapse parent if 1 child remains
- [x] `focus_pane`: set active pane
- [x] `focus_next`/`focus_prev`: move between sibling panes
- [x] `resize_split`: adjust child sizes
- [x] Write unit tests

### 2.6 Implement keyboard shortcuts
- [x] `Cmd/Ctrl+N`: new workspace
- [x] `Cmd/Ctrl+D`: split right
- [x] `Cmd/Ctrl+Shift+D`: split down
- [x] `Cmd/Ctrl+W`: close active pane
- [x] `Cmd/Ctrl+1..9`: switch workspace
- [x] `Cmd/Ctrl+B`: toggle sidebar
- [x] `Cmd/Ctrl+Shift+[`/`]`: prev/next workspace
- [x] Write unit tests

### 2.7 Add pane count guardrail
- [x] Track total pane count across all workspaces in WorkspaceManager
- [x] Warn via `tracing::warn!` when pane count exceeds 50
- [x] Not a hard limit, just a warning

### UI: Workspace View Rendering
- [x] Create `ui/workspace_view.rs`
- [x] Render pane tree as colored rectangles with labels
- [x] Support recursive split layout (horizontal/vertical)
- [x] Each placeholder pane shows ID + name
- [x] Highlight the active pane

### UI: Update app.rs with full layout
- [x] Sidebar on the left (egui::SidePanel)
- [x] Workspace view in the center (egui::CentralPanel)
- [x] Wire WorkspaceManager into RmuxApp state
- [x] Handle keyboard shortcuts
- [x] Toggle sidebar visibility

---

## Verification

- [x] `cargo check --workspace` passes
- [x] `cargo fmt --all -- --check` passes
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [x] `cargo test --workspace` passes
- [x] `cargo doc --no-deps --workspace` passes
- [x] All tasks in `docs/PLAN.md` Phase 2 marked `[x]`
