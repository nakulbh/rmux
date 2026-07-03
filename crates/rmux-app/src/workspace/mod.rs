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
pub mod splits;

use model::{Workspace, WorkspaceId};
use splits::PaneId;

/// The maximum number of panes before a warning is emitted.
const MAX_PANES_BEFORE_WARN: usize = 50;

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
        };
        manager.create_workspace("Workspace 1".to_string());
        manager
    }

    /// Create a new workspace with the given name.
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

    /// Get all workspace IDs and names.
    pub fn list(&self) -> Vec<(WorkspaceId, &str, usize)> {
        self.workspaces.iter().map(|w| (w.id, w.name.as_str(), w.pane_count())).collect()
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
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
