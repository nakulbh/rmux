//! Workspace model — groups a pane tree with metadata.
//!
//! A workspace represents a collection of terminal panes arranged in a split layout.
//! Users can switch between workspaces via the sidebar.

#![allow(dead_code)]

use rmux_terminal::OscNotification;

use super::splits::{PaneId, PaneNode, PaneTreeError, SplitDirection, SplitId};
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
}

impl Workspace {
    /// Create a new workspace with a single default pane (no terminal yet).
    pub fn new(id: WorkspaceId, name: String, next_pane_id: &mut u64) -> Self {
        let pane_id = *next_pane_id;
        *next_pane_id += 1;
        let pane = PaneNode::new_leaf(PaneId(pane_id));
        Self { id, name, root: pane, active_pane: PaneId(pane_id) }
    }

    /// Set the terminal for a pane by its ID.
    pub fn set_terminal(&mut self, pane_id: PaneId, terminal: TerminalPane) {
        if let Some(slot) = self.root.find_terminal_mut(pane_id) {
            *slot = Some(terminal);
        }
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

    /// Move focus in a given direction.
    pub fn focus_direction(&mut self, direction: FocusDirection) {
        match direction {
            FocusDirection::Next => self.focus_next(),
            FocusDirection::Previous => self.focus_prev(),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_workspace(id: u64, name: &str, next_pane_id: &mut u64) -> Workspace {
        Workspace::new(WorkspaceId(id), name.to_string(), next_pane_id)
    }

    #[test]
    fn test_workspace_creation() {
        let mut pane_id = 1;
        let ws = make_workspace(1, "Test", &mut pane_id);
        assert_eq!(ws.pane_count(), 1);
        assert!(!ws.pane_ids().is_empty());
    }

    #[test]
    fn test_split_right() {
        let mut pane_id = 1;
        let mut split_id = 1;
        let mut ws = make_workspace(1, "Test", &mut pane_id);
        let original_active = ws.active_pane;
        let new_id = ws.split_right(original_active, &mut pane_id, &mut split_id).unwrap();
        assert_eq!(ws.pane_count(), 2);
        assert_ne!(new_id, original_active);
    }

    #[test]
    fn test_close_pane_updates_focus() {
        let mut pane_id = 1;
        let mut split_id = 1;
        let mut ws = make_workspace(1, "Test", &mut pane_id);
        let p1 = ws.active_pane;
        let p2 = ws.split_right(p1, &mut pane_id, &mut split_id).unwrap();

        ws.focus_pane(p2);
        ws.close_pane(p1).unwrap();
        assert_eq!(ws.pane_count(), 1);
        assert_eq!(ws.active_pane, p2);

        let result = ws.close_pane(p2);
        assert!(result.is_err());
    }

    #[test]
    fn test_focus_next_prev() {
        let mut pane_id = 1;
        let mut split_id = 1;
        let mut ws = make_workspace(1, "Test", &mut pane_id);
        let p1 = ws.active_pane;
        let p2 = ws.split_right(p1, &mut pane_id, &mut split_id).unwrap();
        let _p3 = ws.split_down(p2, &mut pane_id, &mut split_id).unwrap();

        let panes = ws.pane_ids();
        assert_eq!(panes.len(), 3);

        ws.focus_pane(p1);
        ws.focus_next();
        assert_eq!(ws.active_pane, panes[1]);
        ws.focus_next();
        assert_eq!(ws.active_pane, panes[2]);
        ws.focus_next();
        assert_eq!(ws.active_pane, panes[0]);

        ws.focus_prev();
        assert_eq!(ws.active_pane, panes[2]);
        ws.focus_prev();
        assert_eq!(ws.active_pane, panes[1]);
    }
}
