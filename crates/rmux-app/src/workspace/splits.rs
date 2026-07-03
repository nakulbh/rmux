//! Pane tree model — the recursive tree structure that describes split layouts.
//!
//! Each workspace contains a pane tree. Interior nodes are `Split` nodes
//! that divide their area among children. Leaf nodes contain terminal panes.

#![allow(dead_code)]

use thiserror::Error;

use crate::ui::TerminalPane;

/// Error type for pane tree operations.
#[derive(Error, Debug, PartialEq)]
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

/// A unique identifier for a terminal pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneId(pub u64);

/// A unique identifier for a split container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SplitId(pub u64);

/// The direction of a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

impl SplitDirection {
    pub fn is_horizontal(&self) -> bool {
        matches!(self, Self::Horizontal)
    }

    pub fn is_vertical(&self) -> bool {
        matches!(self, Self::Vertical)
    }
}

/// A node in the recursive pane tree.
pub enum PaneNode {
    Leaf { id: PaneId, terminal: Box<Option<TerminalPane>> },
    Split { id: SplitId, direction: SplitDirection, children: Vec<PaneNode>, sizes: Vec<f32> },
}

impl std::fmt::Debug for PaneNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Leaf { id, terminal } => f
                .debug_struct("Leaf")
                .field("id", id)
                .field("has_terminal", &terminal.is_some())
                .finish(),
            Self::Split { id, direction, children, sizes } => f
                .debug_struct("Split")
                .field("id", id)
                .field("direction", direction)
                .field("children", children)
                .field("sizes", sizes)
                .finish(),
        }
    }
}

impl PaneNode {
    pub fn new_leaf(id: PaneId) -> Self {
        Self::Leaf { id, terminal: Box::new(None) }
    }

    pub fn new_leaf_with_terminal(id: PaneId, terminal: TerminalPane) -> Self {
        Self::Leaf { id, terminal: Box::new(Some(terminal)) }
    }

    pub fn new_split(id: SplitId, direction: SplitDirection, children: Vec<PaneNode>) -> Self {
        let count = children.len() as f32;
        let size = 1.0 / count;
        let sizes = vec![size; children.len()];
        Self::Split { id, direction, children, sizes }
    }

    pub fn is_leaf(&self) -> bool {
        matches!(self, Self::Leaf { .. })
    }

    pub fn is_split(&self) -> bool {
        matches!(self, Self::Split { .. })
    }

    pub fn pane_id(&self) -> Option<PaneId> {
        match self {
            Self::Leaf { id, .. } => Some(*id),
            Self::Split { .. } => None,
        }
    }

    pub fn find_terminal_mut(&mut self, target: PaneId) -> Option<&mut Option<TerminalPane>> {
        match self {
            Self::Leaf { id, terminal } if *id == target => Some(terminal.as_mut()),
            Self::Leaf { .. } => None,
            Self::Split { children, .. } => {
                children.iter_mut().find_map(|c| c.find_terminal_mut(target))
            }
        }
    }

    pub fn get_terminal(&mut self, target: PaneId) -> Option<&mut TerminalPane> {
        self.find_terminal_mut(target).and_then(|opt| opt.as_mut())
    }

    pub fn process_pty_outputs(&mut self) {
        match self {
            Self::Leaf { terminal, .. } => {
                if let Some(t) = terminal.as_mut() {
                    t.process_pty_output();
                }
            }
            Self::Split { children, .. } => {
                for child in children.iter_mut() {
                    child.process_pty_outputs();
                }
            }
        }
    }

    pub fn pane_ids(&self) -> Vec<PaneId> {
        match self {
            Self::Leaf { id, .. } => vec![*id],
            Self::Split { children, .. } => children.iter().flat_map(|c| c.pane_ids()).collect(),
        }
    }

    pub fn pane_count(&self) -> usize {
        match self {
            Self::Leaf { .. } => 1,
            Self::Split { children, .. } => children.iter().map(|c| c.pane_count()).sum(),
        }
    }

    pub fn find_leaf(&self, target: PaneId) -> Option<&Option<TerminalPane>> {
        match self {
            Self::Leaf { id, terminal } if *id == target => Some(terminal.as_ref()),
            Self::Leaf { .. } => None,
            Self::Split { children, .. } => children.iter().find_map(|c| c.find_leaf(target)),
        }
    }

    pub fn split_at(
        &mut self,
        target_pane: PaneId,
        direction: SplitDirection,
        new_pane_id: PaneId,
        new_split_id: SplitId,
    ) -> Result<PaneId, PaneTreeError> {
        if let Self::Leaf { id, .. } = self {
            if *id == target_pane {
                let old = std::mem::replace(self, Self::new_leaf(PaneId(0)));
                *self = Self::Split {
                    id: new_split_id,
                    direction,
                    children: vec![old, Self::new_leaf(new_pane_id)],
                    sizes: vec![0.5, 0.5],
                };
                return Ok(new_pane_id);
            }
            return Err(PaneTreeError::PaneNotFound(target_pane));
        }

        if let Self::Split { children, .. } = self {
            for child in children.iter_mut() {
                if let Self::Leaf { id, .. } = child {
                    if *id == target_pane {
                        let old = std::mem::replace(child, Self::new_leaf(PaneId(0)));
                        *child = Self::Split {
                            id: new_split_id,
                            direction,
                            children: vec![old, Self::new_leaf(new_pane_id)],
                            sizes: vec![0.5, 0.5],
                        };
                        return Ok(new_pane_id);
                    }
                } else {
                    match child.split_at(target_pane, direction, new_pane_id, new_split_id) {
                        Ok(id) => return Ok(id),
                        Err(PaneTreeError::PaneNotFound(_)) => continue,
                        Err(e) => return Err(e),
                    }
                }
            }
        }

        Err(PaneTreeError::PaneNotFound(target_pane))
    }

    pub fn close_pane(&mut self, target_pane: PaneId) -> Result<(), PaneTreeError> {
        if self.pane_count() <= 1 {
            return Err(PaneTreeError::CannotCloseLastPane);
        }

        let needs_collapse = self.close_pane_impl(target_pane)?;

        if needs_collapse {
            self.collapse_if_single_child();
        }

        Ok(())
    }

    fn close_pane_impl(&mut self, target_pane: PaneId) -> Result<bool, PaneTreeError> {
        match self {
            Self::Split { children, sizes, .. } => {
                // Find the index of the target leaf pane
                let target_idx = children
                    .iter()
                    .position(|child| child.is_leaf() && child.pane_id() == Some(target_pane));
                if let Some(i) = target_idx {
                    children.remove(i);
                    let count = children.len() as f32;
                    *sizes = vec![1.0 / count; children.len()];
                    return Ok(true);
                }

                for child in children.iter_mut() {
                    if child.is_split() {
                        match child.close_pane_impl(target_pane) {
                            Ok(true) => return Ok(true),
                            Ok(false) => continue,
                            Err(PaneTreeError::PaneNotFound(_)) => continue,
                            Err(e) => return Err(e),
                        }
                    }
                }
            }
            Self::Leaf { .. } => {
                if self.pane_id() == Some(target_pane) {
                    return Err(PaneTreeError::CannotCloseLastPane);
                }
            }
        }

        Err(PaneTreeError::PaneNotFound(target_pane))
    }

    fn collapse_if_single_child(&mut self) {
        if let Self::Split { children, .. } = self {
            for child in children.iter_mut() {
                child.collapse_if_single_child();
            }
        }

        if let Self::Split { children, .. } = self
            && children.len() == 1
        {
            let sole = children.remove(0);
            *self = sole;
        }
    }

    pub fn resize_split(
        &mut self,
        split_id: SplitId,
        child_index: usize,
        delta: f32,
    ) -> Result<(), PaneTreeError> {
        match self {
            Self::Split { id, sizes, children, .. } if *id == split_id => {
                if child_index >= sizes.len() {
                    return Err(PaneTreeError::InvalidChildIndex(child_index));
                }
                let new_size = (sizes[child_index] + delta).clamp(0.1, 0.9);
                let diff = new_size - sizes[child_index];
                let sibling_count = sizes.len() - 1;
                if sibling_count > 0 {
                    let per_sibling = -diff / sibling_count as f32;
                    sizes[child_index] = new_size;
                    for (i, size) in sizes.iter_mut().enumerate() {
                        if i != child_index {
                            *size = (*size + per_sibling).clamp(0.05, 0.9);
                        }
                    }
                }
                let total: f32 = sizes.iter().sum();
                if total > 0.0 {
                    for size in sizes.iter_mut() {
                        *size /= total;
                    }
                }
                Ok(())
            }
            Self::Split { children, .. } => {
                for child in children.iter_mut() {
                    match child.resize_split(split_id, child_index, delta) {
                        Ok(()) => return Ok(()),
                        Err(PaneTreeError::SplitNotFound(_)) => continue,
                        Err(e) => return Err(e),
                    }
                }
                Err(PaneTreeError::SplitNotFound(split_id))
            }
            _ => Err(PaneTreeError::SplitNotFound(split_id)),
        }
    }

    pub fn leaf_panes(&self) -> Vec<(PaneId, Option<&TerminalPane>)> {
        match self {
            Self::Leaf { id, terminal } => vec![(*id, terminal.as_ref().as_ref())],
            Self::Split { children, .. } => children.iter().flat_map(|c| c.leaf_panes()).collect(),
        }
    }
}

impl PaneNode {
    pub fn leaf_panes_mut(&mut self) -> Vec<(PaneId, &mut Option<TerminalPane>)> {
        match self {
            Self::Leaf { id, terminal } => vec![(*id, terminal.as_mut())],
            Self::Split { children, .. } => {
                children.iter_mut().flat_map(|c| c.leaf_panes_mut()).collect()
            }
        }
    }

    /// Collect pane IDs whose terminal process has exited.
    pub fn collect_exited_panes(&self) -> Vec<PaneId> {
        match self {
            Self::Leaf { id, terminal } => {
                if terminal.as_ref().as_ref().is_some_and(|t| t.is_exited()) {
                    vec![*id]
                } else {
                    vec![]
                }
            }
            Self::Split { children, .. } => {
                children.iter().flat_map(|c| c.collect_exited_panes()).collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_tree() -> PaneNode {
        let leaf1 = PaneNode::new_leaf(PaneId(1));
        let leaf2 = PaneNode::new_leaf(PaneId(2));
        PaneNode::new_split(SplitId(10), SplitDirection::Horizontal, vec![leaf1, leaf2])
    }

    #[test]
    fn test_pane_node_leaf_creation() {
        let leaf = PaneNode::new_leaf(PaneId(1));
        assert!(leaf.is_leaf());
        assert!(!leaf.is_split());
        assert_eq!(leaf.pane_id(), Some(PaneId(1)));
        assert_eq!(leaf.pane_count(), 1);
    }

    #[test]
    fn test_pane_node_split_creation() {
        let leaf1 = PaneNode::new_leaf(PaneId(1));
        let leaf2 = PaneNode::new_leaf(PaneId(2));
        let split =
            PaneNode::new_split(SplitId(10), SplitDirection::Horizontal, vec![leaf1, leaf2]);
        assert!(split.is_split());
        assert!(!split.is_leaf());
        assert_eq!(split.pane_count(), 2);
    }

    #[test]
    fn test_pane_ids_collection() {
        let root = make_test_tree();
        let ids = root.pane_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&PaneId(1)));
        assert!(ids.contains(&PaneId(2)));
    }

    #[test]
    fn test_split_at_leaf() {
        let mut root = make_test_tree();
        let result = root.split_at(PaneId(1), SplitDirection::Vertical, PaneId(3), SplitId(11));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PaneId(3));
        assert_eq!(root.pane_count(), 3);
        assert!(root.pane_ids().contains(&PaneId(3)));
    }

    #[test]
    fn test_close_pane() {
        let mut root = make_test_tree();
        assert_eq!(root.pane_count(), 2);
        let result = root.close_pane(PaneId(1));
        assert!(result.is_ok());
        assert_eq!(root.pane_count(), 1);
        assert!(root.is_leaf());
        assert_eq!(root.pane_id(), Some(PaneId(2)));
    }

    #[test]
    fn test_close_last_pane_errors() {
        let mut root = PaneNode::new_leaf(PaneId(1));
        let result = root.close_pane(PaneId(1));
        assert!(result.is_err());
    }
}
