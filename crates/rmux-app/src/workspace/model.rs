//! Workspace model — groups a pane tree with metadata.
//!
//! A workspace represents a collection of terminal panes arranged in a split layout.
//! Users can switch between workspaces via the sidebar.

#![allow(dead_code)]

use rmux_terminal::OscNotification;

use super::splits::{
    PaneId, PaneNode, PaneTreeError, SpatialDirection, SplitDirection, SplitId,
    find_pane_in_direction,
};
use crate::browser::BrowserPane;
use crate::ui::TerminalPane;

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
    /// Short git status summary (e.g. "clean", "modified", "untracked").
    /// Displayed on the sidebar card (set via `update_git_info`).
    pub git_status: Option<String>,
    /// TCP ports currently listening in this workspace's directory.
    /// Displayed as badges on the sidebar card (set via `update_ports`).
    pub ports: Vec<u16>,
    /// When `Some`, only this pane is rendered (zoomed/maximized mode).
    /// Toggled by `Cmd/Ctrl+Shift+Enter`.
    pub zoomed_pane: Option<PaneId>,
}

impl Workspace {
    /// Create a new workspace with a single default pane (no terminal yet).
    pub fn new(id: WorkspaceId, name: String, next_pane_id: &mut u64) -> Self {
        let pane_id = *next_pane_id;
        *next_pane_id += 1;
        let pane = PaneNode::new_leaf(PaneId(pane_id));
        Self {
            id,
            name,
            root: pane,
            active_pane: PaneId(pane_id),
            status: None,
            progress: None,
            git_branch: None,
            git_status: None,
            ports: Vec::new(),
            zoomed_pane: None,
        }
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

    /// Process PTY output for all panes in this workspace, collecting any
    /// OSC notifications (tagged with their pane id) into `notifications`.
    pub fn process_pty_outputs(&mut self, notifications: &mut Vec<(PaneId, OscNotification)>) {
        self.root.process_pty_outputs(notifications);
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
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
