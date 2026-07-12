//! Pane tree model — recursive split layout.
//!
//! Each workspace contains a pane tree. Interior nodes are `Split` nodes
//! that divide their area among children. Leaf nodes contain terminal panes.

use thiserror::Error;

/// Error type for pane tree operations.
#[derive(Error, Debug, PartialEq)]
pub enum PaneTreeError {
    #[error("Pane not found: {0:?}")]
    PaneNotFound(PaneId),
    #[error("Cannot close the last pane")]
    CannotCloseLastPane,
    #[error("Invalid child index: {0}")]
    #[allow(dead_code)]
    InvalidChildIndex(usize),
}

/// A unique identifier for a terminal pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PaneId(pub u64);

/// A unique identifier for a split container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SplitId(pub u64);

/// The direction of a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[allow(dead_code)]
impl SplitDirection {
    pub fn is_horizontal(&self) -> bool {
        matches!(self, Self::Horizontal)
    }

    pub fn is_vertical(&self) -> bool {
        matches!(self, Self::Vertical)
    }
}

/// A node in the recursive pane tree.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PaneNode {
    Leaf { id: PaneId },
    Split { id: SplitId, direction: SplitDirection, children: Vec<PaneNode>, sizes: Vec<f32> },
}

#[allow(dead_code)]
impl PaneNode {
    pub fn new_leaf(id: PaneId) -> Self {
        Self::Leaf { id }
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

    /// Find a mutable reference to a pane node by ID anywhere in the tree.
    pub fn find_pane_mut(&mut self, target: PaneId) -> Option<&mut PaneNode> {
        if self.pane_id() == Some(target) {
            return Some(self);
        }
        if let Self::Split { children, .. } = self {
            for child in children.iter_mut() {
                let found = child.find_pane_mut(target);
                if found.is_some() {
                    return found;
                }
            }
        }
        None
    }

    /// Replace the pane at `target` with `new_node`.
    pub fn replace_pane(&mut self, target: PaneId, new_node: PaneNode) -> bool {
        if let Some(node) = self.find_pane_mut(target) {
            *node = new_node;
            true
        } else {
            false
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

    /// Split the target pane, creating a new leaf next to it.
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

    /// Close a pane and collapse its parent split if only one child remains.
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
                let target_idx = children.iter().position(|child| {
                    let id = child.pane_id();
                    !child.is_split() && id == Some(target_pane)
                });
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

        let should_collapse = matches!(self, Self::Split { children, .. } if children.len() == 1);
        if should_collapse {
            if let Self::Split { children, .. } = self {
                let sole = children.remove(0);
                *self = sole;
            }
        }
    }

    /// Recursively equalize all split ratios in the tree.
    pub fn equalize_splits(&mut self) {
        if let Self::Split { children, sizes, .. } = self {
            let count = children.len();
            if count > 0 {
                let equal = 1.0 / count as f32;
                for size in sizes.iter_mut() {
                    *size = equal;
                }
            }
            for child in children.iter_mut() {
                child.equalize_splits();
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
        let split = PaneNode::new_split(SplitId(10), SplitDirection::Horizontal, vec![leaf1, leaf2]);
        assert!(split.is_split());
        assert!(!split.is_leaf());
        assert_eq!(split.pane_count(), 2);
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
