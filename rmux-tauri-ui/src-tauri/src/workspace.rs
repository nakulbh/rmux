//! Workspace management for the Tauri backend.
//!
//! A workspace represents a collection of terminal panes arranged in a
//! split layout. Users can switch between workspaces.

use std::collections::HashMap;

use crate::pane::{PaneId, PaneNode, PaneTreeError, SplitDirection, SplitId};
use crate::pty::PtySession;

/// A unique identifier for a workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceId(pub u64);

/// A workspace containing a pane tree and metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    pub root: PaneNode,
    pub active_pane: PaneId,
}

/// Manages all workspaces and their PTY sessions.
pub struct WorkspaceManager {
    workspaces: HashMap<WorkspaceId, Workspace>,
    sessions: HashMap<PaneId, PtySession>,
    next_workspace_id: u64,
    next_pane_id: u64,
    next_split_id: u64,
    active_workspace: Option<WorkspaceId>,
}

impl WorkspaceManager {
    pub fn new() -> Self {
        Self {
            workspaces: HashMap::new(),
            sessions: HashMap::new(),
            next_workspace_id: 1,
            next_pane_id: 1,
            next_split_id: 1,
            active_workspace: None,
        }
    }

    fn alloc_workspace_id(&mut self) -> WorkspaceId {
        let id = WorkspaceId(self.next_workspace_id);
        self.next_workspace_id += 1;
        id
    }

    fn alloc_pane_id(&mut self) -> PaneId {
        let id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;
        id
    }

    fn alloc_split_id(&mut self) -> SplitId {
        let id = SplitId(self.next_split_id);
        self.next_split_id += 1;
        id
    }

    /// Create a new workspace with a single default pane.
    pub fn create_workspace(&mut self, name: String) -> WorkspaceId {
        let ws_id = self.alloc_workspace_id();
        let pane_id = self.alloc_pane_id();
        let pane = PaneNode::new_leaf(pane_id);
        let ws = Workspace {
            id: ws_id,
            name,
            root: pane,
            active_pane: pane_id,
        };
        self.workspaces.insert(ws_id, ws);
        if self.active_workspace.is_none() {
            self.active_workspace = Some(ws_id);
        }
        ws_id
    }

    /// Switch the active workspace.
    pub fn switch_workspace(&mut self, ws_id: WorkspaceId) -> bool {
        if self.workspaces.contains_key(&ws_id) {
            self.active_workspace = Some(ws_id);
            true
        } else {
            false
        }
    }

    /// Close a workspace and all its PTY sessions.
    pub fn close_workspace(&mut self, ws_id: WorkspaceId) -> bool {
        if let Some(ws) = self.workspaces.remove(&ws_id) {
            for pane_id in ws.root.pane_ids() {
                if let Some(mut session) = self.sessions.remove(&pane_id) {
                    let _ = session.close();
                }
            }
            if self.active_workspace == Some(ws_id) {
                self.active_workspace = self.workspaces.keys().next().copied();
            }
            true
        } else {
            false
        }
    }

    /// List all workspaces.
    pub fn list_workspaces(&self) -> Vec<&Workspace> {
        self.workspaces.values().collect()
    }

    /// Rename a workspace.
    pub fn rename_workspace(&mut self, ws_id: WorkspaceId, name: String) -> bool {
        if let Some(ws) = self.workspaces.get_mut(&ws_id) {
            ws.name = name;
            true
        } else {
            false
        }
    }

    /// Get a workspace by ID.
    #[allow(dead_code)]
    pub fn get_workspace(&self, ws_id: WorkspaceId) -> Option<&Workspace> {
        self.workspaces.get(&ws_id)
    }

    /// Get a mutable workspace by ID.
    #[allow(dead_code)]
    pub fn get_workspace_mut(&mut self, ws_id: WorkspaceId) -> Option<&mut Workspace> {
        self.workspaces.get_mut(&ws_id)
    }

    /// Get the active workspace ID.
    #[allow(dead_code)]
    pub fn active_workspace(&self) -> Option<WorkspaceId> {
        self.active_workspace
    }

    /// Spawn a PTY session for a pane in a workspace.
    pub fn spawn_terminal(
        &mut self,
        _ws_id: WorkspaceId,
        pane_id: PaneId,
        cols: u16,
        rows: u16,
    ) -> anyhow::Result<()> {
        let session = PtySession::spawn(cols, rows)
            .map_err(|e| anyhow::anyhow!("Failed to spawn PTY: {e}"))?;
        self.sessions.insert(pane_id, session);
        Ok(())
    }

    /// Write input to a pane's PTY session.
    pub fn write_terminal(&mut self, pane_id: PaneId, data: &[u8]) -> anyhow::Result<()> {
        let session = self.sessions.get_mut(&pane_id)
            .ok_or_else(|| anyhow::anyhow!("No session for pane {pane_id:?}"))?;
        session.write(data)
            .map_err(|e| anyhow::anyhow!("Write failed: {e}"))
    }

    /// Read buffered output from a pane's PTY session.
    pub fn read_terminal(&mut self, pane_id: PaneId) -> Vec<u8> {
        self.sessions.get_mut(&pane_id)
            .map_or_else(Vec::new, |s| s.read())
    }

    /// Resize a pane's PTY session.
    pub fn resize_terminal(&mut self, pane_id: PaneId, cols: u16, rows: u16) -> anyhow::Result<()> {
        let session = self.sessions.get_mut(&pane_id)
            .ok_or_else(|| anyhow::anyhow!("No session for pane {pane_id:?}"))?;
        session.resize(cols, rows)
            .map_err(|e| anyhow::anyhow!("Resize failed: {e}"))
    }

    /// Close a pane's PTY session.
    pub fn close_terminal(&mut self, pane_id: PaneId) -> anyhow::Result<()> {
        if let Some(mut session) = self.sessions.remove(&pane_id) {
            session.close()
                .map_err(|e| anyhow::anyhow!("Close failed: {e}"))?;
        }
        Ok(())
    }

    /// Split a pane to the right (horizontal split).
    pub fn split_pane_right(&mut self, ws_id: WorkspaceId, pane_id: PaneId) -> Result<PaneId, PaneTreeError> {
        let new_pane = self.alloc_pane_id();
        let new_split = self.alloc_split_id();
        let ws = self.workspaces.get_mut(&ws_id)
            .ok_or(PaneTreeError::PaneNotFound(pane_id))?;
        ws.root.split_at(pane_id, SplitDirection::Horizontal, new_pane, new_split)
    }

    /// Split a pane downward (vertical split).
    pub fn split_pane_down(&mut self, ws_id: WorkspaceId, pane_id: PaneId) -> Result<PaneId, PaneTreeError> {
        let new_pane = self.alloc_pane_id();
        let new_split = self.alloc_split_id();
        let ws = self.workspaces.get_mut(&ws_id)
            .ok_or(PaneTreeError::PaneNotFound(pane_id))?;
        ws.root.split_at(pane_id, SplitDirection::Vertical, new_pane, new_split)
    }

    /// Close a pane in a workspace.
    pub fn close_pane(&mut self, ws_id: WorkspaceId, pane_id: PaneId) -> Result<(), PaneTreeError> {
        let ws = self.workspaces.get_mut(&ws_id)
            .ok_or(PaneTreeError::PaneNotFound(pane_id))?;
        ws.root.close_pane(pane_id)?;

        // Clean up PTY session
        if let Some(mut session) = self.sessions.remove(&pane_id) {
            let _ = session.close();
        }

        // Update active pane if needed
        let remaining = ws.root.pane_ids();
        if ws.active_pane == pane_id {
            if let Some(new_focus) = remaining.first() {
                ws.active_pane = *new_focus;
            }
        }

        Ok(())
    }

    /// Set the active (focused) pane in a workspace.
    pub fn focus_pane(&mut self, ws_id: WorkspaceId, pane_id: PaneId) -> bool {
        if let Some(ws) = self.workspaces.get_mut(&ws_id) {
            if ws.root.pane_ids().contains(&pane_id) {
                ws.active_pane = pane_id;
                return true;
            }
        }
        false
    }

    /// Get all pane IDs in a workspace.
    #[allow(dead_code)]
    pub fn pane_ids(&self, ws_id: WorkspaceId) -> Vec<PaneId> {
        self.workspaces.get(&ws_id)
            .map_or_else(Vec::new, |ws| ws.root.pane_ids())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_close_workspace() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace("test".to_string());
        assert_eq!(mgr.list_workspaces().len(), 1);
        assert!(mgr.close_workspace(ws));
        assert!(mgr.list_workspaces().is_empty());
    }

    #[test]
    fn test_switch_workspace() {
        let mut mgr = WorkspaceManager::new();
        let ws1 = mgr.create_workspace("ws1".to_string());
        let ws2 = mgr.create_workspace("ws2".to_string());
        assert_eq!(mgr.active_workspace(), Some(ws1));
        assert!(mgr.switch_workspace(ws2));
        assert_eq!(mgr.active_workspace(), Some(ws2));
    }

    #[test]
    fn test_rename_workspace() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace("old".to_string());
        assert!(mgr.rename_workspace(ws, "new".to_string()));
        assert_eq!(mgr.get_workspace(ws).unwrap().name, "new");
    }
}
