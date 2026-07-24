//! Workspace management module.
//!
//! This module contains the pane tree model, workspace model, and workspace
//! manager that orchestrates multiple workspaces.
//!
//! # Structure
//!
//! - `splits` — `PaneNode` tree model and operations
//! - `model` — `Workspace` with pane tree and metadata
//! - `mod` (this file) — `WorkspaceManager` for multi-workspace management

pub mod model;
pub mod sidebar_snapshot;
pub mod splits;
pub mod surface;
pub mod title;

use std::collections::VecDeque;

use model::{Workspace, WorkspaceError, WorkspaceId};
use splits::PaneId;
use surface::{Surface, SurfaceId};

/// The maximum number of panes before a warning is emitted.
const MAX_PANES_BEFORE_WARN: usize = 50;

/// Upper bound on the closed-tabs stack. When the stack exceeds this,
/// the oldest entry is dropped to make room for the new one. Matches
/// the `Cmd+Shift+T` (Reopen Last Closed) UX in browsers/IDEs.
pub const MAX_CLOSED_TABS: usize = 16;

/// A surface captured at close-time so it can be restored by
/// `reopen_last_closed_tab`. Holds the owning workspace and pane ids
/// so the manager can find a home for the surface even if the
/// original location has been removed since.
///
/// `Surface` does not implement `Debug`/`Clone`/`PartialEq` (PTY
/// handles are not cloneable), so `ClosedTab` cannot derive those
/// either. A manual `Debug` impl exposes the surface's `id` and
/// `title` plus the workspace/pane ids for log readability.
pub struct ClosedTab {
    pub surface: Surface,
    pub workspace_id: WorkspaceId,
    pub pane_id: PaneId,
}

impl std::fmt::Debug for ClosedTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClosedTab")
            .field("surface_title", &self.surface.title)
            .field("surface_id", &self.surface.id)
            .field("workspace_id", &self.workspace_id)
            .field("pane_id", &self.pane_id)
            .finish_non_exhaustive()
    }
}

/// Panes and workspaces removed by [`WorkspaceManager::close_exited_panes`].
///
/// Ids are the raw inner values of `WorkspaceId` / `PaneId` so callers
/// can forward them directly as API event payloads.
#[derive(Debug, Default)]
pub struct ExitCleanup {
    /// `(workspace_id, pane_id)` for each pane that was closed.
    pub panes: Vec<(u64, u64)>,
    /// Ids of workspaces closed because their last pane exited.
    pub workspaces: Vec<u64>,
    /// `(workspace_id, pane_id)` leaves that were cleared after their last
    /// shell exited and need a fresh terminal attached (only workspace left).
    pub panes_needing_respawn: Vec<(u64, u64)>,
}

/// Manages multiple workspaces and tracks which is active.
///
/// The `WorkspaceManager` owns all workspaces and provides operations
/// for creating, closing, and switching between them. It also tracks
/// the total pane count across all workspaces for memory guardrails.
#[derive(Debug)]
pub struct WorkspaceManager {
    /// All workspaces, ordered by creation time.
    workspaces: Vec<Workspace>,
    /// Index into `workspaces` for the currently active workspace.
    active_index: usize,
    /// Monotonically increasing ID counter for workspaces.
    next_workspace_id: u64,
    /// Monotonically increasing ID counter for panes.
    next_pane_id: u64,
    /// Monotonically increasing ID counter for splits.
    next_split_id: u64,
    /// Bounded stack of recently-closed surfaces for `Reopen Last Closed`.
    /// Newest is at the back; oldest is at the front (drained first when
    /// the stack exceeds `MAX_CLOSED_TABS`).
    closed_tabs: VecDeque<ClosedTab>,
}

impl WorkspaceManager {
    /// Create a new `WorkspaceManager` with a single default workspace.
    pub fn new() -> Self {
        let mut manager = Self {
            workspaces: Vec::new(),
            active_index: 0,
            next_workspace_id: 1,
            next_pane_id: 1,
            next_split_id: 1,
            closed_tabs: VecDeque::new(),
        };
        // Seed title; real auto title arrives once the first terminal reports
        // cwd / process (see [`Self::refresh_auto_titles`]).
        manager.create_workspace("Terminal".to_string());
        manager
    }

    /// Create a new workspace with the given seed name (auto-title until renamed).
    ///
    /// The new workspace automatically becomes the active workspace.
    pub fn create_workspace(&mut self, name: String) -> WorkspaceId {
        let id = WorkspaceId(self.next_workspace_id);
        self.next_workspace_id += 1;

        let workspace = Workspace::new(id, name, &mut self.next_pane_id);
        self.workspaces.push(workspace);
        self.active_index = self.workspaces.len() - 1;

        tracing::info!(workspace_count = self.workspaces.len(), "Created workspace {:?}", id);

        id
    }

    /// Close a workspace by its ID.
    ///
    /// If the closed workspace was active, switches to the first remaining workspace.
    /// Returns an error if the workspace is not found or if it's the last workspace.
    #[allow(dead_code)]
    pub fn close_workspace(&mut self, id: WorkspaceId) -> Result<(), anyhow::Error> {
        if self.workspaces.len() <= 1 {
            anyhow::bail!("Cannot close the last workspace");
        }

        let pos = self
            .workspaces
            .iter()
            .position(|w| w.id == id)
            .ok_or_else(|| anyhow::anyhow!("Workspace not found: {:?}", id))?;

        self.workspaces.remove(pos);

        // Adjust active index
        if pos < self.active_index {
            self.active_index = self.active_index.saturating_sub(1);
        } else if self.active_index >= self.workspaces.len() {
            self.active_index = self.workspaces.len() - 1;
        }

        tracing::info!(workspace_count = self.workspaces.len(), "Closed workspace {:?}", id);

        Ok(())
    }

    /// Switch to the workspace at the given index.
    ///
    /// Does nothing if the index is out of bounds.
    pub fn switch_to(&mut self, index: usize) {
        if index < self.workspaces.len() {
            self.active_index = index;
            tracing::debug!(index, "Switched to workspace");
        }
    }

    /// Switch to the next workspace (wraps around).
    pub fn switch_next(&mut self) {
        if self.workspaces.len() > 1 {
            self.active_index = (self.active_index + 1) % self.workspaces.len();
            tracing::debug!(index = self.active_index, "Switched to next workspace");
        }
    }

    /// Switch to the previous workspace (wraps around).
    pub fn switch_prev(&mut self) {
        if self.workspaces.len() > 1 {
            if self.active_index == 0 {
                self.active_index = self.workspaces.len() - 1;
            } else {
                self.active_index -= 1;
            }
            tracing::debug!(index = self.active_index, "Switched to previous workspace");
        }
    }

    /// Get an immutable reference to the active workspace.
    pub fn active(&self) -> &Workspace {
        &self.workspaces[self.active_index]
    }

    /// Get a mutable reference to the active workspace.
    pub fn active_mut(&mut self) -> &mut Workspace {
        &mut self.workspaces[self.active_index]
    }

    /// All workspaces in display order.
    pub fn workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }

    /// Mutable access to all workspaces (for bulk operations like font resize).
    pub fn workspaces_mut(&mut self) -> &mut [Workspace] {
        &mut self.workspaces
    }

    /// Get a mutable reference to the workspace with the given id, if any.
    pub fn workspace_mut(&mut self, id: WorkspaceId) -> Option<&mut Workspace> {
        self.workspaces.iter_mut().find(|w| w.id == id)
    }

    /// Focus a pane anywhere in the application.
    ///
    /// Switches to the workspace containing the pane and focuses it.
    /// Returns `false` if no workspace contains the pane.
    pub fn focus_pane_global(&mut self, pane: PaneId) -> bool {
        // Use `find_leaf` (tree walk, zero-allocation) instead of `pane_ids()`
        // which allocates a full Vec<PaneId> per workspace.
        let Some(index) = self.workspaces.iter().position(|w| w.root.find_leaf(pane).is_some())
        else {
            return false;
        };
        self.active_index = index;
        self.workspaces[index].focus_pane(pane);
        true
    }

    /// Get the number of workspaces.
    pub fn workspace_count(&self) -> usize {
        self.workspaces.len()
    }

    /// Get the index of the active workspace.
    pub fn active_index(&self) -> usize {
        self.active_index
    }

    /// Get the total number of panes across all workspaces.
    pub fn total_pane_count(&self) -> usize {
        self.workspaces.iter().map(|w| w.pane_count()).sum()
    }

    /// Number of terminal surfaces in the active workspace's pane tree.
    ///
    /// Pass-through to [`Workspace::terminal_count`]. Used by the
    /// `CloseTab` dispatcher to disambiguate "close this tab" from
    /// "close this pane" when only one surface is open.
    pub fn terminal_count(&self) -> usize {
        self.active().terminal_count()
    }

    /// Check the pane count guardrail and warn if exceeded.
    ///
    /// This is called after any operation that creates a pane.
    /// Emits a `tracing::warn!` when the total exceeds `MAX_PANES_BEFORE_WARN`.
    pub fn check_pane_guardrail(&self) {
        let count = self.total_pane_count();
        if count > MAX_PANES_BEFORE_WARN {
            tracing::warn!(
                total_panes = count,
                max_recommended = MAX_PANES_BEFORE_WARN,
                "Pane count exceeds recommended limit — consider closing unused panes"
            );
        }
    }

    /// Split the active pane to the right in the active workspace.
    ///
    /// Returns the ID of the newly created pane.
    pub fn split_active_right(&mut self) -> Result<PaneId, splits::PaneTreeError> {
        let index = self.active_index;
        let ws = &mut self.workspaces[index];
        let active = ws.active_pane_id();
        let new_id = ws.split_right(active, &mut self.next_pane_id, &mut self.next_split_id)?;
        self.check_pane_guardrail();
        Ok(new_id)
    }

    /// Split the active pane downward in the active workspace.
    ///
    /// Returns the ID of the newly created pane.
    pub fn split_active_down(&mut self) -> Result<PaneId, splits::PaneTreeError> {
        let index = self.active_index;
        let ws = &mut self.workspaces[index];
        let active = ws.active_pane_id();
        let new_id = ws.split_down(active, &mut self.next_pane_id, &mut self.next_split_id)?;
        self.check_pane_guardrail();
        Ok(new_id)
    }

    /// Close the active pane in the active workspace.
    pub fn close_active_pane(&mut self) -> Result<(), splits::PaneTreeError> {
        let ws = self.active_mut();
        let active = ws.active_pane_id();
        ws.close_pane(active)
    }

    /// Close a pane by id in whichever workspace contains it.
    ///
    /// Used by the socket API (`surface.close`) and tmux-compat `kill-pane`.
    pub fn close_pane_global(&mut self, pane: PaneId) -> Result<(), splits::PaneTreeError> {
        let Some(index) = self.workspaces.iter().position(|w| w.root.find_leaf(pane).is_some())
        else {
            return Err(splits::PaneTreeError::PaneNotFound(pane));
        };
        self.workspaces[index].close_pane(pane)
    }

    /// Process PTY output for all panes across all workspaces.
    pub fn process_all_panes(&mut self) {
        for workspace in &mut self.workspaces {
            workspace.process_pty_outputs();
        }
    }

    /// Close terminals whose process has exited.
    ///
    /// Order of operations per workspace:
    /// 1. Drop individual dead **tabs** when the leaf still has live ones
    ///    (typing `exit` on one Cmd+T tab removes just that tab).
    /// 2. Close **panes** where every remaining shell is dead.
    /// 3. If that was the last pane of a non-last workspace, close the
    ///    workspace. The last workspace is never closed — its dead pane
    ///    is cleared instead so the app can respawn a live shell.
    ///
    /// Returns panes/workspaces removed so callers can publish events.
    /// `panes_needing_respawn` lists leaf ids that were cleared and need
    /// a fresh `TerminalPane` attached by the app.
    pub fn close_exited_panes(&mut self) -> ExitCleanup {
        let mut cleanup = ExitCleanup::default();
        let mut i = 0;
        while i < self.workspaces.len() {
            let workspace_id = self.workspaces[i].id;

            // 1. Individual dead tabs (multi-surface leaves with live peers).
            let removed_tabs = self.workspaces[i].root.close_exited_surfaces();
            if removed_tabs > 0 {
                tracing::debug!(
                    workspace_id = workspace_id.0,
                    removed_tabs,
                    "Closed exited terminal tabs"
                );
            }

            // 2. Panes where every shell is dead.
            let exited: Vec<PaneId> = self.workspaces[i].root.collect_exited_panes();
            if exited.is_empty() {
                i += 1;
                continue;
            }

            let mut workspace_removed = false;
            for pane_id in &exited {
                match self.workspaces[i].close_pane(*pane_id) {
                    Ok(()) => {
                        tracing::debug!(pane_id = pane_id.0, "Closed exited pane");
                        cleanup.panes.push((workspace_id.0, pane_id.0));
                    }
                    Err(splits::PaneTreeError::CannotCloseLastPane) => {
                        if self.workspaces.len() > 1 {
                            tracing::info!(
                                workspace_id = ?workspace_id,
                                "Last pane exited, closing workspace"
                            );
                            let _ = self.close_workspace(workspace_id);
                            cleanup.panes.push((workspace_id.0, pane_id.0));
                            cleanup.workspaces.push(workspace_id.0);
                            workspace_removed = true;
                            break;
                        }
                        // Last pane of the only workspace: clear the dead
                        // shell so the app can attach a fresh one.
                        if let Some(leaf) = self.workspaces[i].root.find_pane_mut(*pane_id) {
                            leaf.clear_terminals();
                            cleanup.panes_needing_respawn.push((workspace_id.0, pane_id.0));
                            tracing::info!(
                                pane_id = pane_id.0,
                                "Last shell exited; cleared pane for respawn"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!(pane_id = pane_id.0, error = %e, "Failed to close exited pane");
                    }
                }
            }

            if !workspace_removed {
                i += 1;
            }
        }
        cleanup
    }

    /// Rename a workspace by ID (marks the name as user-custom, cmux-style).
    pub fn rename_workspace(&mut self, id: model::WorkspaceId, new_name: String) {
        if let Some(ws) = self.workspaces.iter_mut().find(|w| w.id == id) {
            ws.set_custom_name(new_name);
            tracing::debug!(workspace_id = ?id, "Renamed workspace (custom)");
        }
    }

    /// Refresh automatic sidebar titles and multi-pane metadata.
    ///
    /// - **Title** still follows the focused surface (cmux `applyProcessTitle`)
    ///   unless the user set a custom name.
    /// - **Path lines / PR / agent** aggregate **all** terminals in the
    ///   workspace so the sidebar can show multiple `branch · dir` rows like
    ///   cmux (not only the focused pane).
    pub fn refresh_auto_titles(&mut self) {
        for ws in &mut self.workspaces {
            let meta = collect_workspace_sidebar_meta(ws);
            ws.path_contexts = meta.path_lines;
            ws.path_context = ws.path_contexts.first().cloned();
            if let Some(branch) = meta.focused_git_branch {
                ws.git_branch = Some(branch);
            }
            ws.pull_request = meta.pull_request;
            ws.shows_agent_activity = meta.shows_agent_activity;

            if ws.name_is_custom {
                continue;
            }
            let Some(title) = meta.focused_title else {
                continue;
            };
            let _ = ws.apply_automatic_title(title);
        }
    }

    /// Close the currently active workspace.
    ///
    /// Returns an error if it is the last workspace.
    pub fn close_active_workspace(&mut self) -> Result<WorkspaceId, anyhow::Error> {
        let active_id = self.active().id;
        self.close_workspace(active_id)?;
        Ok(active_id)
    }

    /// Toggle zoom (maximize/restore) on the active pane in the active workspace.
    ///
    /// When zooming in, the active pane fills the entire workspace area.
    /// When zooming out (restoring), the full split tree is shown again.
    pub fn toggle_zoom(&mut self) -> Option<PaneId> {
        let ws = self.active_mut();
        if let Some(zoomed) = ws.zoomed_pane.take() {
            tracing::debug!(pane_id = zoomed.0, "Zoom restored");
            None
        } else {
            let active = ws.active_pane;
            ws.zoomed_pane = Some(active);
            tracing::debug!(pane_id = active.0, "Pane zoomed");
            Some(active)
        }
    }

    /// Equalize all split ratios in the active workspace's pane tree.
    ///
    /// Every `Split` node gets equal child sizes. Useful after manual
    /// resizing to return to a balanced layout.
    pub fn equalize_splits(&mut self) {
        self.active_mut().root.equalize_splits();
        tracing::debug!("Split sizes equalized");
    }

    /// Count the surfaces in the active leaf, walking the pane tree.
    #[allow(dead_code)]
    fn active_leaf_surface_count(&self) -> usize {
        fn walk(node: &splits::PaneNode, target: PaneId) -> usize {
            match node {
                splits::PaneNode::Leaf { id, surfaces, .. } if *id == target => surfaces.len(),
                splits::PaneNode::Leaf { .. } | splits::PaneNode::Browser { .. } => 0,
                splits::PaneNode::Split { children, .. } => {
                    children.iter().map(|c| walk(c, target)).sum()
                }
            }
        }
        walk(
            &self.workspaces[self.active_index].root,
            self.workspaces[self.active_index].active_pane,
        )
    }

    /// Append a new surface (tab) to the active leaf in the active
    /// workspace. When `title` is `None`, a default of
    /// `"Terminal {n}"` is used where `n` is the *new* surface count.
    #[allow(dead_code)]
    pub fn new_surface_in_active(
        &mut self,
        title: Option<String>,
    ) -> Result<SurfaceId, WorkspaceError> {
        // Promote legacy terminal first so the numbering / tab list includes
        // the shell the user already had open.
        self.active_mut().ensure_surfaces_ready()?;
        let count = self.active_leaf_surface_count();
        let title = title.unwrap_or_else(|| format!("Terminal {}", count + 1));
        self.active_mut().new_surface(title)
    }

    /// Cycle to the next surface within the active workspace's active leaf.
    #[allow(dead_code)]
    pub fn next_surface_in_active(&mut self) -> Result<(), WorkspaceError> {
        self.active_mut().next_surface()
    }

    /// Cycle to the previous surface within the active workspace's active leaf.
    #[allow(dead_code)]
    pub fn previous_surface_in_active(&mut self) -> Result<(), WorkspaceError> {
        self.active_mut().previous_surface()
    }

    /// Focus the surface at `idx` in the active workspace's active leaf.
    #[allow(dead_code)]
    pub fn select_surface_in_active(&mut self, idx: usize) -> Result<(), WorkspaceError> {
        self.active_mut().select_surface(idx)
    }

    /// Close the surface at `idx` in the active workspace's active leaf.
    /// `None` targets the currently focused surface.
    #[allow(dead_code)]
    pub fn close_surface_in_active(
        &mut self,
        idx: Option<usize>,
    ) -> Result<Surface, WorkspaceError> {
        let target = match idx {
            Some(i) => i,
            None => self.workspaces[self.active_index].active_surface_index(),
        };
        self.active_mut().close_surface(target)
    }

    /// Rename the surface at `idx` in the active workspace's active leaf.
    #[allow(dead_code)]
    pub fn rename_surface_in_active(
        &mut self,
        idx: usize,
        title: String,
    ) -> Result<(), WorkspaceError> {
        self.active_mut().rename_surface(idx, title)
    }

    /// Close every surface in the active workspace's active leaf except
    /// the currently focused one. Returns the closed surfaces in their
    /// original order.
    #[allow(dead_code)]
    pub fn close_other_surfaces_in_active(&mut self) -> Result<Vec<Surface>, WorkspaceError> {
        self.active_mut().close_other_surfaces()
    }

    /// Close the surface at `idx` in the active workspace's active leaf
    /// and capture it on the closed-tabs stack. `None` targets the
    /// currently focused surface.
    ///
    /// `Surface` is not `Clone` (PTY handles aren't cloneable), so the
    /// closed surface moves into the stack and is not returned. Use
    /// `reopen_last_closed_tab` to observe the captured state.
    #[allow(dead_code)]
    pub fn close_surface_in_active_with_capture(
        &mut self,
        idx: Option<usize>,
    ) -> Result<(), WorkspaceError> {
        let workspace_id = self.active().id;
        let pane_id = self.active().active_pane;
        let surface = self.close_surface_in_active(idx)?;
        self.closed_tabs.push_back(ClosedTab { surface, workspace_id, pane_id });
        while self.closed_tabs.len() > MAX_CLOSED_TABS {
            self.closed_tabs.pop_front();
        }
        Ok(())
    }

    /// Reopen the most-recently closed surface, restoring it to the
    /// pane that owned it at close-time. If that workspace or pane
    /// no longer exists, falls back to the active workspace and
    /// active pane respectively. Errors with
    /// [`WorkspaceError::NoClosedTabs`] when the stack is empty.
    #[allow(dead_code)]
    pub fn reopen_last_closed_tab(&mut self) -> Result<(), WorkspaceError> {
        let entry = self.closed_tabs.pop_back().ok_or(WorkspaceError::NoClosedTabs)?;
        let ClosedTab { surface, workspace_id, pane_id } = entry;

        let ws_idx =
            self.workspaces.iter().position(|w| w.id == workspace_id).unwrap_or(self.active_index);

        if ws_idx != self.active_index {
            self.active_index = ws_idx;
        }

        let ws = &mut self.workspaces[ws_idx];
        let target_pane =
            if ws.root.find_pane_mut(pane_id).is_some() { pane_id } else { ws.active_pane };
        ws.add_surface_to_pane(target_pane, surface)
            .map_err(|_| WorkspaceError::PaneNotFound(pane_id))?;

        tracing::info!(
            surface_id = self.workspaces[ws_idx]
                .active_surface()
                .map(|s| s.id.0)
                .unwrap_or(0),
            workspace_id = ?self.workspaces[ws_idx].id,
            pane_id = ?target_pane,
            "Reopened closed tab"
        );

        Ok(())
    }
}

/// Aggregated sidebar metadata from every terminal in a workspace.
struct WorkspaceSidebarMeta {
    focused_title: Option<String>,
    focused_git_branch: Option<String>,
    path_lines: Vec<String>,
    pull_request: Option<sidebar_snapshot::PullRequestDisplay>,
    shows_agent_activity: bool,
}

/// Walk all panes: unique path lines (focused first), any-agent flag, best PR.
fn collect_workspace_sidebar_meta(ws: &Workspace) -> WorkspaceSidebarMeta {
    let focused_term = ws.root.find_pane(ws.active_pane).and_then(|n| n.active_terminal());

    let focused_title = focused_term.map(|t| t.auto_workspace_title());
    let focused_git_branch = focused_term.and_then(|t| t.cached_git_branch().map(str::to_string));
    let focused_cwd = focused_term.and_then(|t| t.cached_cwd().map(|p| p.to_path_buf()));
    let focused_branch_ref = focused_git_branch.as_deref();

    let mut others: Vec<(Option<std::path::PathBuf>, Option<String>)> = Vec::new();
    let mut any_agent = false;
    let mut open_pr: Option<sidebar_snapshot::PullRequestDisplay> = None;
    let mut any_pr: Option<sidebar_snapshot::PullRequestDisplay> = None;
    let focused_ptr = focused_term.map(|t| t as *const _);

    ws.root.for_each_terminal(&mut |term| {
        // Skip the focused terminal in the "others" list — it is injected first.
        let is_focused = focused_ptr.is_some_and(|p| std::ptr::eq(term, p));
        if !is_focused {
            others.push((
                term.cached_cwd().map(|p| p.to_path_buf()),
                term.cached_git_branch().map(str::to_string),
            ));
        }

        if term.cached_fg_title().is_some_and(sidebar_snapshot::is_coding_agent_command) {
            any_agent = true;
        }

        if let Some(pr) = term.cached_pull_request() {
            if pr.is_open && open_pr.is_none() {
                open_pr = Some(pr.clone());
            } else if any_pr.is_none() {
                any_pr = Some(pr.clone());
            }
        }
    });

    // Prefer focused terminal's PR when it has one; else first open PR; else any.
    let pull_request =
        focused_term.and_then(|t| t.cached_pull_request().cloned()).or(open_pr).or(any_pr);

    let path_lines = sidebar_snapshot::unique_path_lines(
        Some((focused_cwd.as_deref(), focused_branch_ref)),
        others,
        sidebar_snapshot::MAX_PATH_LINES,
    );

    WorkspaceSidebarMeta {
        focused_title,
        focused_git_branch,
        path_lines,
        pull_request,
        shows_agent_activity: any_agent,
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use splits::PaneNode;
    use surface::{Surface, SurfaceId};

    #[test]
    fn test_workspace_manager_creation() {
        let manager = WorkspaceManager::new();
        assert_eq!(manager.workspace_count(), 1);
        assert_eq!(manager.active_index(), 0);
        assert_eq!(manager.total_pane_count(), 1);
    }

    #[test]
    fn test_create_multiple_workspaces() {
        let mut manager = WorkspaceManager::new();
        manager.create_workspace("WS 2".to_string());
        manager.create_workspace("WS 3".to_string());
        assert_eq!(manager.workspace_count(), 3);
        // Newly created workspace becomes active
        assert_eq!(manager.active_index(), 2);
    }

    #[test]
    fn test_switch_workspaces() {
        let mut manager = WorkspaceManager::new();
        manager.create_workspace("WS 2".to_string());
        manager.create_workspace("WS 3".to_string());

        manager.switch_to(0);
        assert_eq!(manager.active_index(), 0);

        manager.switch_next();
        assert_eq!(manager.active_index(), 1);

        manager.switch_next();
        assert_eq!(manager.active_index(), 2);

        // Wrap around
        manager.switch_next();
        assert_eq!(manager.active_index(), 0);

        manager.switch_prev();
        assert_eq!(manager.active_index(), 2); // wrapped back
    }

    #[test]
    fn test_close_workspace() {
        let mut manager = WorkspaceManager::new();
        let ws2 = manager.create_workspace("WS 2".to_string());
        manager.create_workspace("WS 3".to_string());

        assert_eq!(manager.workspace_count(), 3);

        // Close workspace 2 (not the last one)
        manager.close_workspace(ws2).unwrap();
        assert_eq!(manager.workspace_count(), 2);
    }

    #[test]
    fn test_close_last_workspace_errors() {
        let mut manager = WorkspaceManager::new();
        let result = manager.close_workspace(WorkspaceId(1));
        assert!(result.is_err());
    }

    #[test]
    fn test_close_active_workspace_switches() {
        let mut manager = WorkspaceManager::new();
        let ws1 = WorkspaceId(1);
        manager.create_workspace("WS 2".to_string());
        // Now active is index 1 (WS 2)
        assert_eq!(manager.active_index(), 1);

        // Close WS 1
        manager.close_workspace(ws1).unwrap();
        assert_eq!(manager.workspace_count(), 1);
        assert_eq!(manager.active_index(), 0);
    }

    #[test]
    fn test_split_active_right() {
        let mut manager = WorkspaceManager::new();
        assert_eq!(manager.total_pane_count(), 1);

        let new_id = manager.split_active_right().unwrap();
        assert_eq!(manager.total_pane_count(), 2);
        assert!(new_id.0 > 0);
    }

    #[test]
    fn test_split_active_down() {
        let mut manager = WorkspaceManager::new();
        assert_eq!(manager.total_pane_count(), 1);

        let new_id = manager.split_active_down().unwrap();
        assert_eq!(manager.total_pane_count(), 2);
        assert!(new_id.0 > 0);
    }

    #[test]
    fn test_close_active_pane() {
        let mut manager = WorkspaceManager::new();
        let _new_pane = manager.split_active_right().unwrap();
        assert_eq!(manager.total_pane_count(), 2);

        let result = manager.close_active_pane();
        assert!(result.is_ok());
        assert_eq!(manager.total_pane_count(), 1);
    }

    #[test]
    fn test_close_last_pane_errors() {
        let mut manager = WorkspaceManager::new();
        let result = manager.close_active_pane();
        assert!(result.is_err());
    }

    #[test]
    fn test_pane_guardrail_warning() {
        // This test just verifies the guardrail doesn't panic.
        // In a real app, we'd check the tracing log output.
        let mut manager = WorkspaceManager::new();

        // Manually set up many panes to test the guardrail
        for _ in 1..=49 {
            let _ = manager.split_active_right();
        }
        // Should be at 50, which is <= MAX_PANES_BEFORE_WARN
        manager.check_pane_guardrail();

        // One more puts us over
        let _ = manager.split_active_down();
        // Should now be over 50 — this calls check_pane_guardrail internally
        assert!(manager.total_pane_count() > 50);
    }

    #[test]
    fn test_switch_to_out_of_bounds_is_safe() {
        let mut manager = WorkspaceManager::new();
        // Switching to an out-of-bounds index should not panic
        manager.switch_to(999);
        assert_eq!(manager.active_index(), 0);
    }

    #[test]
    fn test_close_active_workspace() {
        let mut manager = WorkspaceManager::new();
        manager.create_workspace("WS 2".to_string());
        assert_eq!(manager.workspace_count(), 2);
        let closed_id = manager.close_active_workspace().unwrap();
        assert_eq!(closed_id.0, 2); // WS 2 was active
        assert_eq!(manager.workspace_count(), 1);
    }

    #[test]
    fn test_close_active_workspace_last_errors() {
        let mut manager = WorkspaceManager::new();
        assert_eq!(manager.workspace_count(), 1);
        let result = manager.close_active_workspace();
        assert!(result.is_err());
    }

    #[test]
    fn test_toggle_zoom() {
        let mut manager = WorkspaceManager::new();
        // Initially not zoomed
        assert!(manager.active().zoomed_pane.is_none());

        // Zoom in → returns Some(active_pane_id)
        let zoomed = manager.toggle_zoom();
        assert!(zoomed.is_some());
        assert_eq!(manager.active().zoomed_pane, zoomed);

        // Zoom out → returns None
        let restored = manager.toggle_zoom();
        assert!(restored.is_none());
        assert!(manager.active().zoomed_pane.is_none());
    }

    #[test]
    fn test_equalize_splits_via_manager() {
        let mut manager = WorkspaceManager::new();
        // Create a split layout
        manager.split_active_right().unwrap();
        manager.split_active_down().unwrap();
        assert_eq!(manager.total_pane_count(), 3);

        // Unequalize some sizes by tweaking the root's sizes
        if let splits::PaneNode::Split { sizes, .. } = &mut manager.active_mut().root {
            sizes[0] = 0.2;
            sizes[1] = 0.8;
        }

        // Equalize
        manager.equalize_splits();

        if let splits::PaneNode::Split { sizes, .. } = &manager.active().root {
            assert!((sizes[0] - 0.5).abs() < f32::EPSILON);
            assert!((sizes[1] - 0.5).abs() < f32::EPSILON);
        }
    }

    // ----- W2.2: WorkspaceManager surface pass-throughs -----

    fn leaf_surfaces_of(ws: &Workspace) -> &Vec<Surface> {
        fn walk(node: &PaneNode, target: PaneId) -> Option<&Vec<Surface>> {
            match node {
                PaneNode::Leaf { id, surfaces, .. } if *id == target => Some(surfaces),
                PaneNode::Leaf { .. } | PaneNode::Browser { .. } => None,
                PaneNode::Split { children, .. } => children.iter().find_map(|c| walk(c, target)),
            }
        }
        walk(&ws.root, ws.active_pane).expect("active pane is a Leaf")
    }

    #[test]
    fn test_manager_new_surface_in_active_default_title() {
        let mut manager = WorkspaceManager::new();
        let id = manager.new_surface_in_active(None).unwrap();
        assert_eq!(id, SurfaceId(1));
        let surfaces = leaf_surfaces_of(manager.active());
        assert_eq!(surfaces.len(), 1);
        assert_eq!(surfaces[0].title, "Terminal 1");

        manager.new_surface_in_active(None).unwrap();
        manager.new_surface_in_active(None).unwrap();
        let surfaces = leaf_surfaces_of(manager.active());
        assert_eq!(surfaces[0].title, "Terminal 1");
        assert_eq!(surfaces[1].title, "Terminal 2");
        assert_eq!(surfaces[2].title, "Terminal 3");
    }

    #[test]
    fn test_manager_new_surface_in_active_custom_title() {
        let mut manager = WorkspaceManager::new();
        let id = manager.new_surface_in_active(Some("My Tab".to_string())).unwrap();
        assert_eq!(id, SurfaceId(1));
        let surfaces = leaf_surfaces_of(manager.active());
        assert_eq!(surfaces[0].title, "My Tab");
    }

    #[test]
    fn test_manager_next_surface_in_active() {
        let mut manager = WorkspaceManager::new();
        manager.new_surface_in_active(None).unwrap();
        manager.new_surface_in_active(None).unwrap();
        manager.new_surface_in_active(None).unwrap();
        assert_eq!(manager.active().active_surface_index(), 2);

        manager.next_surface_in_active().unwrap();
        assert_eq!(manager.active().active_surface_index(), 0);
        manager.next_surface_in_active().unwrap();
        assert_eq!(manager.active().active_surface_index(), 1);
    }

    #[test]
    fn test_manager_close_surface_in_active_none_means_active() {
        let mut manager = WorkspaceManager::new();
        manager.new_surface_in_active(None).unwrap();
        manager.new_surface_in_active(None).unwrap();
        manager.new_surface_in_active(None).unwrap();
        // `new_surface_in_active` makes the new surface the active one,
        // so after 3 inserts the focus is on idx 2 (Terminal 3).
        assert_eq!(manager.active().active_surface_index(), 2);

        let closed = manager.close_surface_in_active(None).unwrap();
        assert_eq!(closed.title, "Terminal 3");
        assert_eq!(closed.id, SurfaceId(3));

        let surfaces = leaf_surfaces_of(manager.active());
        assert_eq!(surfaces.len(), 2);
        assert_eq!(surfaces[0].title, "Terminal 1");
        assert_eq!(surfaces[1].title, "Terminal 2");
    }

    #[test]
    fn test_manager_close_surface_in_active_specific_index() {
        let mut manager = WorkspaceManager::new();
        manager.new_surface_in_active(None).unwrap();
        manager.new_surface_in_active(None).unwrap();
        manager.new_surface_in_active(None).unwrap();

        let closed = manager.close_surface_in_active(Some(0)).unwrap();
        assert_eq!(closed.title, "Terminal 1");

        let surfaces = leaf_surfaces_of(manager.active());
        assert_eq!(surfaces.len(), 2);
        assert_eq!(surfaces[0].title, "Terminal 2");
        assert_eq!(surfaces[1].title, "Terminal 3");
    }

    #[test]
    fn test_terminal_count_tracks_active_leaf() {
        let mut manager = WorkspaceManager::new();
        // Brand-new workspace: uninitialized leaf → counts as 1.
        assert_eq!(manager.terminal_count(), 1);

        manager.new_surface_in_active(None).unwrap();
        assert_eq!(manager.terminal_count(), 1);

        manager.new_surface_in_active(None).unwrap();
        manager.new_surface_in_active(None).unwrap();
        assert_eq!(manager.terminal_count(), 3);

        manager.close_surface_in_active_with_capture(None).expect("close should succeed");
        assert_eq!(manager.terminal_count(), 2);
    }

    // ----- W3.1: Bounded closed-tabs stack for ReopenLastClosed -----

    /// Drain the closed-tabs stack by repeatedly reopening, returning the
    /// number of tabs that were on the stack. Uses `NoClosedTabs` as the
    /// terminator to avoid exposing `closed_tabs` directly.
    fn closed_tabs_len_via_drain(manager: &mut WorkspaceManager) -> usize {
        let mut count = 0;
        while manager.reopen_last_closed_tab().is_ok() {
            count += 1;
        }
        count
    }

    #[test]
    fn test_close_surface_pushes_to_stack() {
        let mut manager = WorkspaceManager::new();
        manager.new_surface_in_active(None).unwrap();
        manager.new_surface_in_active(None).unwrap();

        manager.close_surface_in_active_with_capture(None).expect("close should succeed");

        let count = closed_tabs_len_via_drain(&mut manager);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_reopen_last_closed_restores_to_original_pane() {
        let mut manager = WorkspaceManager::new();
        let pane_id = manager.active().active_pane;
        manager.new_surface_in_active(None).unwrap();
        manager.new_surface_in_active(None).unwrap();

        manager.close_surface_in_active_with_capture(None).expect("close should succeed");

        manager.reopen_last_closed_tab().expect("reopen should succeed");

        let surfaces = leaf_surfaces_of(manager.active());
        assert_eq!(surfaces.len(), 2);
        let active = manager.active().active_surface().expect("active surface");
        assert_eq!(active.title, "Terminal 2");
        assert_eq!(manager.active().active_pane, pane_id);
    }

    #[test]
    fn test_reopen_last_closed_no_closed_tabs_errors() {
        let mut manager = WorkspaceManager::new();
        manager.new_surface_in_active(None).unwrap();
        let result = manager.reopen_last_closed_tab();
        assert!(matches!(result, Err(WorkspaceError::NoClosedTabs)));
    }

    #[test]
    fn test_stack_trims_to_max_16() {
        let mut manager = WorkspaceManager::new();
        for _ in 0..17 {
            manager.new_surface_in_active(None).unwrap();
        }
        for _ in 0..16 {
            manager.close_surface_in_active_with_capture(Some(0)).expect("close should succeed");
        }
        let count = closed_tabs_len_via_drain(&mut manager);
        assert_eq!(count, 16, "stack should be trimmed to MAX_CLOSED_TABS");
    }

    #[test]
    fn test_reopen_after_workspace_removed_goes_to_active_workspace() {
        let mut manager = WorkspaceManager::new();
        let ws1_id = WorkspaceId(1);
        manager.create_workspace("WS 2".to_string());
        manager.new_surface_in_active(None).unwrap();
        manager.new_surface_in_active(None).unwrap();
        manager.close_surface_in_active_with_capture(None).expect("close should succeed");

        manager.close_workspace(WorkspaceId(2)).expect("close ws2");
        assert_eq!(manager.active().id, ws1_id);

        manager.reopen_last_closed_tab().expect("reopen should succeed");

        let surfaces = leaf_surfaces_of(manager.active());
        assert!(surfaces.iter().any(|s| s.title == "Terminal 2"));
    }

    #[test]
    fn test_reopen_after_pane_removed_goes_to_active_pane() {
        let mut manager = WorkspaceManager::new();
        let original_pane = manager.active().active_pane;
        manager.split_active_right().expect("split ok");
        manager.new_surface_in_active(None).unwrap();
        manager.new_surface_in_active(None).unwrap();
        manager.close_surface_in_active_with_capture(None).expect("close should succeed");

        let new_pane = manager
            .active()
            .pane_ids()
            .into_iter()
            .find(|id| *id != original_pane)
            .expect("split pane");
        manager.focus_pane_global(new_pane);
        manager.new_surface_in_active(None).unwrap();

        manager.active_mut().close_pane(original_pane).expect("close original pane");

        manager.reopen_last_closed_tab().expect("reopen should succeed");

        let surfaces = leaf_surfaces_of(manager.active());
        assert_eq!(surfaces.len(), 2);
    }

    #[test]
    fn test_close_then_reopen_preserves_surface_data() {
        let mut manager = WorkspaceManager::new();
        manager.new_surface_in_active(None).unwrap();
        manager.new_surface_in_active(Some("Renamed".to_string())).unwrap();

        manager.close_surface_in_active_with_capture(None).expect("close should succeed");

        manager.reopen_last_closed_tab().expect("reopen should succeed");

        let surfaces = leaf_surfaces_of(manager.active());
        let restored = surfaces
            .iter()
            .find(|s| s.title == "Renamed")
            .expect("restored surface should be in the active leaf");
        assert_eq!(restored.title, "Renamed");
    }

    #[test]
    fn test_multiple_closes_reopen_in_reverse_order() {
        let mut manager = WorkspaceManager::new();
        manager.new_surface_in_active(None).unwrap();
        manager.new_surface_in_active(None).unwrap();
        manager.new_surface_in_active(None).unwrap();

        manager.close_surface_in_active_with_capture(Some(2)).expect("close T3");
        manager.close_surface_in_active_with_capture(Some(0)).expect("close T1");

        manager.reopen_last_closed_tab().expect("reopen #1");
        assert_eq!(manager.active().active_surface().unwrap().title, "Terminal 1");

        manager.reopen_last_closed_tab().expect("reopen #2");
        assert_eq!(manager.active().active_surface().unwrap().title, "Terminal 3");

        assert!(matches!(manager.reopen_last_closed_tab(), Err(WorkspaceError::NoClosedTabs)));
    }
}
