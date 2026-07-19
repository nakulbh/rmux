//! Workspace model — groups a pane tree with metadata.
//!
//! A workspace represents a collection of terminal panes arranged in a split layout.
//! Users can switch between workspaces via the sidebar.

#![allow(dead_code)]

use thiserror::Error;

use super::splits::{
    PaneId, PaneNode, PaneTreeError, SpatialDirection, SplitDirection, SplitId,
    find_pane_in_direction,
};
use super::surface::{Surface, SurfaceId};
use crate::browser::BrowserPane;
use crate::ui::{DEFAULT_FONT_SIZE, TerminalPane};

/// Initial size for freshly spawned terminal surfaces (columns).
const INITIAL_COLS: u16 = 80;
/// Initial size for freshly spawned terminal surfaces (rows).
const INITIAL_ROWS: u16 = 24;

/// Error type for workspace-level operations (tab/surface management).
///
/// Distinct from [`PaneTreeError`] which covers pane-tree structural
/// concerns. Surface operations surface here so the manager-level
/// dispatcher can match on the failure mode without inspecting
/// sub-trees.
#[derive(Error, Debug, PartialEq)]
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
    /// The PTY backend refused to spawn a terminal for a new surface.
    /// The wrapped string is the formatted `PtyError`.
    #[error("Surface spawn failed: {0}")]
    SurfaceSpawnFailed(String),
    /// `reopen_last_closed_tab` was called with an empty closed-tabs stack.
    #[error("No closed tabs to reopen")]
    NoClosedTabs,
    /// `reopen_last_closed_tab` was called for a captured tab whose
    /// original pane no longer exists in the tree AND there is no
    /// fallback leaf to restore to. Distinct from
    /// [`PaneTreeError::PaneNotFound`] which signals a missing pane
    /// during a tree operation — this one signals a missing pane at
    /// the manager level with no available recovery path.
    #[error("Pane not found: {0:?} and no fallback available")]
    PaneNotFound(PaneId),
}

/// A unique identifier for a workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkspaceId(pub u64);

/// Direction for focus movement commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    /// Move focus to the next pane in DFS order.
    Next,
    /// Move focus to the previous pane in DFS order.
    Previous,
    /// Move focus spatially by delta (dx, dy).
    Spatial { dx: i32, dy: i32 },
}

/// A workspace containing a pane tree and metadata.
#[derive(Debug)]
pub struct Workspace {
    /// Unique identifier for the workspace.
    pub id: WorkspaceId,
    /// Display name shown in the sidebar (custom or last auto title).
    pub name: String,
    /// When `true`, [`Self::name`] was set by the user (inline rename / API)
    /// and automatic process/cwd titles must not overwrite it — matching
    /// cmux `customTitle` + `CustomTitleSource::user`.
    pub name_is_custom: bool,
    /// Last auto-derived title from the focused pane (process or path).
    /// Restored when the user clears a custom name.
    pub process_title: String,
    /// Idle path context for the sidebar subtitle (`main · ~/proj`), independent
    /// of whether the primary title is currently a running command.
    pub path_context: Option<String>,
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
    /// Short git status summary (e.g. "clean", "modified", "untracked").
    /// Displayed on the sidebar card (set via `update_git_info`).
    pub git_status: Option<String>,
    /// TCP ports currently listening in this workspace's directory.
    /// Displayed as badges on the sidebar card (set via `update_ports`).
    pub ports: Vec<u16>,
    /// When `Some`, only this pane is rendered (zoomed/maximized mode).
    /// Toggled by `Cmd/Ctrl+Shift+Enter`.
    pub zoomed_pane: Option<PaneId>,
    /// Monotonic counter for `SurfaceId`s minted by this workspace.
    /// Starts at 1 and is bumped after each successful `new_surface` call.
    pub next_surface_id: u64,
}

impl Workspace {
    /// Create a new workspace with a single default pane (no terminal yet).
    pub fn new(id: WorkspaceId, name: String, next_pane_id: &mut u64) -> Self {
        let pane_id = *next_pane_id;
        *next_pane_id += 1;
        let pane = PaneNode::new_leaf(PaneId(pane_id));
        // Initial name is treated as a seed auto-title until the first
        // refresh (or as custom when the API/user passes an explicit name —
        // callers that want a lock should call [`Self::set_custom_name`]).
        let process_title = name.clone();
        Self {
            id,
            name,
            name_is_custom: false,
            process_title,
            path_context: None,
            root: pane,
            active_pane: PaneId(pane_id),
            status: None,
            progress: None,
            git_branch: None,
            git_status: None,
            ports: Vec::new(),
            zoomed_pane: None,
            next_surface_id: 1,
        }
    }

    /// Lock the display name to a user-chosen string (cmux custom title).
    pub fn set_custom_name(&mut self, name: String) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return;
        }
        self.name = trimmed.to_string();
        self.name_is_custom = true;
    }

    /// Drop a custom name and resume automatic titles from `process_title`.
    pub fn clear_custom_name(&mut self) {
        self.name_is_custom = false;
        if !self.process_title.is_empty() {
            self.name = self.process_title.clone();
        }
    }

    /// Apply an automatic title from the focused pane (cmux `applyProcessTitle`).
    ///
    /// No-op when the user has set a custom name. Returns whether `name` changed.
    pub fn apply_automatic_title(&mut self, title: String) -> bool {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            return false;
        }
        if self.process_title == trimmed && (self.name_is_custom || self.name == trimmed) {
            return false;
        }
        self.process_title = trimmed.to_string();
        if self.name_is_custom {
            return false;
        }
        if self.name == trimmed {
            return false;
        }
        self.name = trimmed.to_string();
        true
    }

    /// Set the terminal for a pane by its ID.
    pub fn set_terminal(&mut self, pane_id: PaneId, terminal: TerminalPane) {
        if let Some(slot) = self.root.find_terminal_mut(pane_id) {
            *slot = Some(terminal);
        }
    }

    /// Replace the leaf pane at `pane_id` with a browser pane.
    pub fn set_browser(&mut self, pane_id: PaneId, browser: BrowserPane) {
        let browser_node = PaneNode::new_browser(pane_id, browser);
        self.root.replace_pane(pane_id, browser_node);
    }

    /// Process PTY output for all panes in this workspace.
    pub fn process_pty_outputs(&mut self) {
        self.root.process_pty_outputs();
    }

    /// Split the specified pane to the right (horizontal split).
    pub fn split_right(
        &mut self,
        pane_id: PaneId,
        next_pane_id: &mut u64,
        next_split_id: &mut u64,
    ) -> Result<PaneId, PaneTreeError> {
        let new_pane = PaneId(*next_pane_id);
        let new_split = SplitId(*next_split_id);
        *next_pane_id += 1;
        *next_split_id += 1;
        let result =
            self.root.split_at(pane_id, SplitDirection::Horizontal, new_pane, new_split)?;
        Ok(result)
    }

    /// Split the specified pane downward (vertical split).
    pub fn split_down(
        &mut self,
        pane_id: PaneId,
        next_pane_id: &mut u64,
        next_split_id: &mut u64,
    ) -> Result<PaneId, PaneTreeError> {
        let new_pane = PaneId(*next_pane_id);
        let new_split = SplitId(*next_split_id);
        *next_pane_id += 1;
        *next_split_id += 1;
        let result = self.root.split_at(pane_id, SplitDirection::Vertical, new_pane, new_split)?;
        Ok(result)
    }

    /// Close a pane and collapse its parent split if only one child remains.
    pub fn close_pane(&mut self, pane_id: PaneId) -> Result<(), PaneTreeError> {
        self.root.close_pane(pane_id)?;

        let remaining = self.pane_ids();
        if self.active_pane == pane_id
            && let Some(new_focus) = remaining.first()
        {
            self.active_pane = *new_focus;
        }

        Ok(())
    }

    /// Set the active (focused) pane.
    pub fn focus_pane(&mut self, pane_id: PaneId) {
        self.active_pane = pane_id;
    }

    /// Move focus to the next pane in depth-first order (wraps around).
    pub fn focus_next(&mut self) {
        let panes = self.pane_ids();
        if panes.len() <= 1 {
            return;
        }
        if let Some(pos) = panes.iter().position(|&id| id == self.active_pane) {
            let next = (pos + 1) % panes.len();
            self.active_pane = panes[next];
        }
    }

    /// Move focus to the previous pane in depth-first order (wraps around).
    pub fn focus_prev(&mut self) {
        let panes = self.pane_ids();
        if panes.len() <= 1 {
            return;
        }
        if let Some(pos) = panes.iter().position(|&id| id == self.active_pane) {
            let prev = if pos == 0 { panes.len() - 1 } else { pos - 1 };
            self.active_pane = panes[prev];
        }
    }

    /// Move focus to the pane on the left.
    pub fn focus_left(&mut self) {
        if let Some(id) =
            find_pane_in_direction(&self.root, self.active_pane, SpatialDirection::Left)
        {
            self.active_pane = id;
        }
    }

    /// Move focus to the pane on the right.
    pub fn focus_right(&mut self) {
        if let Some(id) =
            find_pane_in_direction(&self.root, self.active_pane, SpatialDirection::Right)
        {
            self.active_pane = id;
        }
    }

    /// Move focus to the pane above.
    pub fn focus_up(&mut self) {
        if let Some(id) = find_pane_in_direction(&self.root, self.active_pane, SpatialDirection::Up)
        {
            self.active_pane = id;
        }
    }

    /// Move focus to the pane below.
    pub fn focus_down(&mut self) {
        if let Some(id) =
            find_pane_in_direction(&self.root, self.active_pane, SpatialDirection::Down)
        {
            self.active_pane = id;
        }
    }

    /// Move focus in a given direction.
    pub fn focus_direction(&mut self, direction: FocusDirection) {
        match direction {
            FocusDirection::Next => self.focus_next(),
            FocusDirection::Previous => self.focus_prev(),
            FocusDirection::Spatial { dx, dy } => {
                let spatial = match (dx, dy) {
                    (-1, 0) => SpatialDirection::Left,
                    (1, 0) => SpatialDirection::Right,
                    (0, -1) => SpatialDirection::Up,
                    (0, 1) => SpatialDirection::Down,
                    _ => return,
                };
                if let Some(id) = find_pane_in_direction(&self.root, self.active_pane, spatial) {
                    self.active_pane = id;
                }
            }
        }
    }

    /// Get all pane IDs in this workspace.
    pub fn pane_ids(&self) -> Vec<PaneId> {
        self.root.pane_ids()
    }

    /// Get the total number of panes in this workspace.
    pub fn pane_count(&self) -> usize {
        self.root.pane_count()
    }

    /// Number of terminal surfaces hosted in this workspace.
    ///
    /// Pass-through to [`PaneNode::terminal_count`]. A workspace with no
    /// leaves (theoretically possible if the root is replaced) returns
    /// `0`; an uninitialized leaf still counts as `1` so the
    /// `CloseTab` vs `ClosePane` disambiguation in the dispatcher stays
    /// consistent with the user's mental model of "one tab open".
    pub fn terminal_count(&self) -> usize {
        self.root.terminal_count()
    }

    /// Get the ID of the currently active pane.
    pub fn active_pane_id(&self) -> PaneId {
        self.active_pane
    }

    /// Get a mutable reference to the active pane's terminal, if it exists.
    pub fn active_terminal(&mut self) -> Option<&mut TerminalPane> {
        self.root.get_terminal(self.active_pane)
    }

    /// Update the git branch and status for this workspace's sidebar card.
    /// Either field may be `None` to indicate "not yet known / not in a repo".
    pub fn update_git_info(&mut self, branch: Option<String>, status: Option<String>) {
        self.git_branch = branch;
        self.git_status = status;
    }

    /// Replace the list of listening ports displayed on the sidebar card.
    pub fn update_ports(&mut self, ports: Vec<u16>) {
        self.ports = ports;
    }

    /// Current git branch name, if known.
    pub fn git_branch(&self) -> Option<&str> {
        self.git_branch.as_deref()
    }

    /// Current git status summary, if known.
    pub fn git_status(&self) -> Option<&str> {
        self.git_status.as_deref()
    }

    /// Listening ports associated with this workspace.
    pub fn ports(&self) -> &[u16] {
        &self.ports
    }

    /// Index of the focused surface within the active leaf, or 0 if the
    /// active pane isn't a `Leaf` (e.g. it's a `Browser`).
    pub fn active_surface_index(&self) -> usize {
        surface_index_in(&self.root, self.active_pane).unwrap_or(0)
    }

    /// The currently focused surface in the active leaf, or `None` if
    /// the active pane isn't a `Leaf` or has no surfaces.
    pub fn active_surface(&self) -> Option<&Surface> {
        let leaf = self.root.find_pane(self.active_pane)?;
        leaf.active_surface()
    }

    /// Borrow the active leaf node, returning [`WorkspaceError::NoActivePane`]
    /// when the active pane is missing or not a `Leaf` (e.g. a `Browser`).
    fn active_leaf_mut(&mut self) -> Result<&mut PaneNode, WorkspaceError> {
        match self.root.find_pane_mut(self.active_pane) {
            Some(pane) if pane.is_leaf() => Ok(pane),
            _ => Err(WorkspaceError::NoActivePane),
        }
    }

    /// Mint a fresh `SurfaceId`, spawn a backing `TerminalPane`, and append
    /// it to the active leaf's surface list. The new surface becomes focused.
    ///
    /// The new shell inherits the focused terminal's current working
    /// directory when it can be resolved (so Cmd+T after `cd` stays put).
    /// If the leaf still only has a legacy `set_terminal` slot, that
    /// terminal is promoted into a surface first so Cmd+T does not orphan it.
    /// Returns the new id.
    pub fn new_surface(&mut self, title: String) -> Result<SurfaceId, WorkspaceError> {
        self.promote_legacy_terminal_if_needed()?;

        // Resolve cwd from the focused terminal *before* we mutably borrow
        // the leaf for `add_surface`.
        let cwd = self
            .root
            .find_pane(self.active_pane)
            .and_then(PaneNode::active_terminal)
            .and_then(TerminalPane::working_directory);

        let id = SurfaceId(self.next_surface_id);
        self.next_surface_id += 1;

        let mut terminal = TerminalPane::spawn_with_cwd(
            INITIAL_COLS,
            INITIAL_ROWS,
            DEFAULT_FONT_SIZE,
            cwd.as_deref(),
        )
        .map_err(|e| WorkspaceError::SurfaceSpawnFailed(e.to_string()))?;
        // Match the app-wide theme immediately so Cmd+T tabs don't flash
        // (or stick on) the default palette after the user changed themes.
        // Font size is refined by `RmuxApp::new_surface_with_terminal`.
        let named = crate::ui::theme::current_named_theme();
        terminal.set_theme(rmux_terminal::TerminalTheme::default().named(named));
        let surface = Surface::new(id, title, terminal);

        let leaf = self.active_leaf_mut()?;
        leaf.add_surface(surface);
        let new_idx = leaf.leaf_surfaces().len() - 1;
        leaf.set_active_surface_index(new_idx);

        Ok(id)
    }

    /// Ensure the active leaf's shells live in `surfaces` (promote the
    /// legacy `set_terminal` slot if present). Safe to call repeatedly.
    pub(crate) fn ensure_surfaces_ready(&mut self) -> Result<(), WorkspaceError> {
        self.promote_legacy_terminal_if_needed()
    }

    /// Move a legacy `terminal` slot into `surfaces` so multi-tab APIs
    /// don't hide the original shell when the first Cmd+T is pressed.
    fn promote_legacy_terminal_if_needed(&mut self) -> Result<(), WorkspaceError> {
        let leaf = self.active_leaf_mut()?;
        if !leaf.leaf_surfaces().is_empty() {
            return Ok(());
        }
        let legacy = match leaf {
            PaneNode::Leaf { terminal, .. } => terminal.take(),
            _ => None,
        };
        let Some(term) = legacy else {
            return Ok(());
        };
        let id = SurfaceId(self.next_surface_id);
        self.next_surface_id += 1;
        let surface = Surface::new(id, "Terminal 1".to_owned(), term);
        let leaf = self.active_leaf_mut()?;
        leaf.add_surface(surface);
        leaf.set_active_surface_index(0);
        Ok(())
    }

    /// Cycle to the next surface within the active leaf (wraps). No-op when
    /// the leaf holds 0 or 1 surfaces.
    pub fn next_surface(&mut self) -> Result<(), WorkspaceError> {
        let leaf = self.active_leaf_mut()?;
        let len = leaf.leaf_surfaces().len();
        if len > 1 {
            let current = leaf.active_surface_index();
            let next = (current + 1) % len;
            leaf.set_active_surface_index(next);
        }
        Ok(())
    }

    /// Cycle to the previous surface within the active leaf (wraps). No-op
    /// when the leaf holds 0 or 1 surfaces.
    pub fn previous_surface(&mut self) -> Result<(), WorkspaceError> {
        let leaf = self.active_leaf_mut()?;
        let len = leaf.leaf_surfaces().len();
        if len > 1 {
            let current = leaf.active_surface_index();
            let prev = if current == 0 { len - 1 } else { current - 1 };
            leaf.set_active_surface_index(prev);
        }
        Ok(())
    }

    /// Focus the surface at `idx` within the active leaf. Returns
    /// [`WorkspaceError::InvalidSurfaceIndex`] when out of range.
    pub fn select_surface(&mut self, idx: usize) -> Result<(), WorkspaceError> {
        let leaf = self.active_leaf_mut()?;
        let len = leaf.leaf_surfaces().len();
        if idx >= len {
            return Err(WorkspaceError::InvalidSurfaceIndex(idx));
        }
        leaf.set_active_surface_index(idx);
        Ok(())
    }

    /// Remove the surface at `idx` and return it. Refuses to remove the
    /// last remaining surface (returns
    /// [`WorkspaceError::CannotCloseLastSurface`]). The leaf's
    /// `active_surface` is adjusted by [`PaneNode::remove_surface`].
    pub fn close_surface(&mut self, idx: usize) -> Result<Surface, WorkspaceError> {
        let leaf = self.active_leaf_mut()?;
        let len = leaf.leaf_surfaces().len();
        if len == 0 {
            return Err(WorkspaceError::InvalidSurfaceIndex(idx));
        }
        if len == 1 {
            return Err(WorkspaceError::CannotCloseLastSurface);
        }
        if idx >= len {
            return Err(WorkspaceError::InvalidSurfaceIndex(idx));
        }
        leaf.remove_surface(idx).ok_or(WorkspaceError::InvalidSurfaceIndex(idx))
    }

    /// Rename the surface at `idx`. Out-of-range indices surface
    /// [`WorkspaceError::InvalidSurfaceIndex`].
    pub fn rename_surface(&mut self, idx: usize, title: String) -> Result<(), WorkspaceError> {
        let leaf = self.active_leaf_mut()?;
        let len = leaf.leaf_surfaces().len();
        if idx >= len {
            return Err(WorkspaceError::InvalidSurfaceIndex(idx));
        }
        leaf.leaf_surfaces_mut()[idx].title = title;
        Ok(())
    }

    /// Remove every surface in the active leaf except the currently focused
    /// one, returning the closed surfaces in their original order. With 0 or
    /// 1 surfaces this is a no-op (returns an empty `Vec`).
    pub fn close_other_surfaces(&mut self) -> Result<Vec<Surface>, WorkspaceError> {
        let leaf = self.active_leaf_mut()?;
        let active_idx = leaf.active_surface_index();
        let len = leaf.leaf_surfaces().len();
        if len <= 1 {
            return Ok(Vec::new());
        }

        let mut all: Vec<Surface> = std::mem::take(leaf.leaf_surfaces_mut());
        let mut closed = Vec::with_capacity(len - 1);
        let mut active_surface = None;
        for (i, surface) in all.drain(..).enumerate() {
            if i == active_idx {
                active_surface = Some(surface);
            } else {
                closed.push(surface);
            }
        }
        let active_surface = active_surface.expect("active_idx < len");
        leaf.leaf_surfaces_mut().push(active_surface);
        leaf.set_active_surface_index(0);
        Ok(closed)
    }

    /// Append `surface` to the leaf with id `pane_id` and make it the
    /// focused surface. Returns [`PaneTreeError::PaneNotFound`] when no
    /// leaf with that id exists (or when the id matches a `Browser`
    /// node, since browsers don't host surfaces). Used by the
    /// closed-tabs stack to restore a captured tab.
    pub fn add_surface_to_pane(
        &mut self,
        pane_id: PaneId,
        surface: Surface,
    ) -> Result<(), PaneTreeError> {
        let node = self.root.find_pane_mut(pane_id).ok_or(PaneTreeError::PaneNotFound(pane_id))?;
        if !node.is_leaf() {
            return Err(PaneTreeError::PaneNotFound(pane_id));
        }
        node.add_surface(surface);
        let new_idx = node.leaf_surfaces().len() - 1;
        node.set_active_surface_index(new_idx);
        Ok(())
    }
}

/// Walk the pane tree looking for the leaf matching `target`; return its
/// `active_surface` index. Returns `None` if no such leaf exists.
fn surface_index_in(node: &PaneNode, target: PaneId) -> Option<usize> {
    match node {
        PaneNode::Leaf { id, active_surface, .. } if *id == target => Some(*active_surface),
        PaneNode::Leaf { .. } | PaneNode::Browser { .. } => None,
        PaneNode::Split { children, .. } => {
            children.iter().find_map(|c| surface_index_in(c, target))
        }
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
