//! Pane tree model — the recursive tree structure that describes split layouts.
//!
//! Each workspace contains a pane tree. Interior nodes are `Split` nodes
//! that divide their area among children. Leaf nodes contain terminal panes
//! (or placeholder panes during development).

#![allow(dead_code)]

use thiserror::Error;

/// Error type for pane tree operations.
#[derive(Error, Debug, PartialEq)]
pub enum PaneTreeError {
    /// The specified pane ID was not found in the tree.
    #[error("Pane not found: {0:?}")]
    PaneNotFound(PaneId),

    /// The specified split ID was not found in the tree.
    #[error("Split not found: {0:?}")]
    SplitNotFound(SplitId),

    /// Cannot close the last pane in a workspace.
    #[error("Cannot close the last pane")]
    CannotCloseLastPane,

    /// Cannot perform the operation because the target is not a leaf.
    #[error("Operation requires a leaf node")]
    NotALeaf,
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
    /// Children are arranged side by side (left to right).
    Horizontal,
    /// Children are arranged stacked (top to bottom).
    Vertical,
}

impl SplitDirection {
    /// Returns `true` if the split is horizontal.
    pub fn is_horizontal(&self) -> bool {
        matches!(self, Self::Horizontal)
    }

    /// Returns `true` if the split is vertical.
    pub fn is_vertical(&self) -> bool {
        matches!(self, Self::Vertical)
    }
}

/// A placeholder pane that simulates a terminal pane during development.
///
/// Will be replaced by real `TerminalPane` in Phase 1. Contains just enough
/// state to render a colored rectangle with a label in the workspace view.
#[derive(Debug, Clone)]
pub struct PlaceholderPane {
    /// Display name of the pane.
    pub name: String,
    /// Background color for the placeholder rectangle.
    pub color: egui::Color32,
}

impl PlaceholderPane {
    /// Create a new placeholder pane with a given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), color: Self::random_color() }
    }

    /// Generate a visually distinct random color for the placeholder.
    fn random_color() -> egui::Color32 {
        // Use a set of distinct, muted terminal-friendly colors
        let color_index = std::time::Instant::now().elapsed().as_nanos() as u8 % 6;
        match color_index {
            0 => egui::Color32::from_rgb(50, 50, 60), // dark blue-gray
            1 => egui::Color32::from_rgb(50, 60, 50), // dark green-gray
            2 => egui::Color32::from_rgb(60, 50, 50), // dark red-gray
            3 => egui::Color32::from_rgb(50, 55, 65), // dark slate
            4 => egui::Color32::from_rgb(55, 50, 55), // dark purple-gray
            _ => egui::Color32::from_rgb(50, 50, 55), // dark blue-gray alt
        }
    }
}

/// A node in the recursive pane tree.
///
/// Leaf nodes contain a terminal pane. Split nodes divide their area
/// among child nodes in either horizontal or vertical orientation.
#[derive(Debug, Clone)]
pub enum PaneNode {
    /// A leaf node containing a terminal (or placeholder) pane.
    Leaf {
        /// Unique identifier for the pane.
        id: PaneId,
        /// The pane itself.
        pane: PlaceholderPane,
    },
    /// An interior node that splits its area among children.
    Split {
        /// Unique identifier for the split container.
        id: SplitId,
        /// Direction of the split (horizontal or vertical).
        direction: SplitDirection,
        /// Child nodes.
        children: Vec<PaneNode>,
        /// Relative sizes of each child (must sum approximately to 1.0).
        sizes: Vec<f32>,
    },
}

impl PaneNode {
    /// Create a new leaf node with a placeholder pane.
    pub fn new_leaf(id: PaneId, name: impl Into<String>) -> Self {
        Self::Leaf { id, pane: PlaceholderPane::new(name) }
    }

    /// Create a new split node with the given children.
    pub fn new_split(id: SplitId, direction: SplitDirection, children: Vec<PaneNode>) -> Self {
        let count = children.len() as f32;
        let size = 1.0 / count;
        let sizes = vec![size; children.len()];
        Self::Split { id, direction, children, sizes }
    }

    /// Returns `true` if this node is a leaf.
    pub fn is_leaf(&self) -> bool {
        matches!(self, Self::Leaf { .. })
    }

    /// Returns `true` if this node is a split.
    pub fn is_split(&self) -> bool {
        matches!(self, Self::Split { .. })
    }

    /// Get the pane ID if this node is a leaf.
    pub fn pane_id(&self) -> Option<PaneId> {
        match self {
            Self::Leaf { id, .. } => Some(*id),
            Self::Split { .. } => None,
        }
    }

    /// Collect all leaf pane IDs from the tree.
    pub fn pane_ids(&self) -> Vec<PaneId> {
        match self {
            Self::Leaf { id, .. } => vec![*id],
            Self::Split { children, .. } => children.iter().flat_map(|c| c.pane_ids()).collect(),
        }
    }

    /// Count the total number of leaf panes in the tree.
    pub fn pane_count(&self) -> usize {
        match self {
            Self::Leaf { .. } => 1,
            Self::Split { children, .. } => children.iter().map(|c| c.pane_count()).sum(),
        }
    }

    /// Find a leaf node by its pane ID. Returns `None` if not found.
    pub fn find_leaf(&self, target: PaneId) -> Option<&PlaceholderPane> {
        match self {
            Self::Leaf { id, pane } if *id == target => Some(pane),
            Self::Leaf { .. } => None,
            Self::Split { children, .. } => children.iter().find_map(|c| c.find_leaf(target)),
        }
    }

    /// Find a leaf node mutably by its pane ID. Returns `None` if not found.
    pub fn find_leaf_mut(&mut self, target: PaneId) -> Option<&mut PlaceholderPane> {
        match self {
            Self::Leaf { id, pane } if *id == target => Some(pane),
            Self::Leaf { .. } => None,
            Self::Split { children, .. } => {
                children.iter_mut().find_map(|c| c.find_leaf_mut(target))
            }
        }
    }

    /// Split a target leaf node in the given direction.
    ///
    /// Returns the ID of the newly created pane.
    pub fn split_at(
        &mut self,
        target_pane: PaneId,
        direction: SplitDirection,
        new_pane_id: PaneId,
        new_split_id: SplitId,
    ) -> Result<PaneId, PaneTreeError> {
        // Check if this node IS the target leaf
        if let Self::Leaf { id, .. } = self {
            if *id == target_pane {
                // Replace self with a split containing the original and a new leaf
                let old_pane = std::mem::replace(self, Self::new_leaf(PaneId(0), "temp"));
                *self = Self::Split {
                    id: new_split_id,
                    direction,
                    children: vec![
                        old_pane,
                        Self::new_leaf(new_pane_id, format!("Pane {}", new_pane_id.0)),
                    ],
                    sizes: vec![0.5, 0.5],
                };
                return Ok(new_pane_id);
            }
            return Err(PaneTreeError::PaneNotFound(target_pane));
        }

        // Search children for the target
        if let Self::Split { children, .. } = self {
            for child in children.iter_mut() {
                if let Self::Leaf { id, .. } = child {
                    if *id == target_pane {
                        let old_pane = std::mem::replace(child, Self::new_leaf(PaneId(0), "temp"));
                        *child = Self::Split {
                            id: new_split_id,
                            direction,
                            children: vec![
                                old_pane,
                                Self::new_leaf(new_pane_id, format!("Pane {}", new_pane_id.0)),
                            ],
                            sizes: vec![0.5, 0.5],
                        };
                        return Ok(new_pane_id);
                    }
                } else {
                    // Recurse into nested splits
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

    /// Close (remove) a leaf pane from the tree.
    ///
    /// If the parent split has only one child remaining, the split is collapsed
    /// and the remaining child takes its place.
    ///
    /// Returns an error if this is the last pane in the tree.
    pub fn close_pane(&mut self, target_pane: PaneId) -> Result<(), PaneTreeError> {
        // Cannot close if this is the only leaf
        if self.pane_count() <= 1 {
            return Err(PaneTreeError::CannotCloseLastPane);
        }

        let needs_collapse = self.close_pane_impl(target_pane)?;

        // After closing, check if this node is a split with only one child
        if needs_collapse {
            self.collapse_if_single_child();
        }

        Ok(())
    }

    /// Internal implementation of close_pane.
    ///
    /// Returns `true` if the parent split may need collapsing after removal.
    fn close_pane_impl(&mut self, target_pane: PaneId) -> Result<bool, PaneTreeError> {
        match self {
            Self::Split { children, sizes, .. } => {
                // Check if any direct child is the target
                for (i, child) in children.iter().enumerate() {
                    if child.is_leaf() && child.pane_id() == Some(target_pane) {
                        // Remove the target child
                        children.remove(i);
                        // Normalize sizes
                        let count = children.len() as f32;
                        *sizes = vec![1.0 / count; children.len()];
                        // Signal that the parent may need collapsing
                        return Ok(true);
                    }
                }

                // Didn't find as direct child, recurse
                for child in children.iter_mut() {
                    if child.is_split() {
                        let needs_collapse = child.close_pane_impl(target_pane);
                        match needs_collapse {
                            Ok(true) => {
                                // The child split may have collapsed already (done inside the recursion)
                                // But we also need to check if *this* split now has one child
                                // We'll check at the top level
                                return Ok(true);
                            }
                            Ok(false) => continue,
                            Err(PaneTreeError::PaneNotFound(_)) => continue,
                            Err(e) => return Err(e),
                        }
                    }
                }
            }
            Self::Leaf { .. } => {
                if self.pane_id() == Some(target_pane) {
                    // This shouldn't happen — the guardrail prevents removing the last pane
                    return Err(PaneTreeError::CannotCloseLastPane);
                }
            }
        }

        Err(PaneTreeError::PaneNotFound(target_pane))
    }

    /// If this node is a split with only one child, collapse it.
    fn collapse_if_single_child(&mut self) {
        // First, recursively collapse children
        if let Self::Split { children, .. } = self {
            for child in children.iter_mut() {
                child.collapse_if_single_child();
            }
        }

        // Then collapse self if applicable
        if let Self::Split { children, .. } = self
            && children.len() == 1
        {
            let sole = children.remove(0);
            *self = sole;
        }
    }

    /// Resize a split by adjusting the size of a specific child.
    ///
    /// `delta` is the change in relative size (positive = larger, negative = smaller).
    pub fn resize_split(
        &mut self,
        split_id: SplitId,
        child_index: usize,
        delta: f32,
    ) -> Result<(), PaneTreeError> {
        match self {
            Self::Split { id, sizes, children, .. } if *id == split_id => {
                if child_index >= sizes.len() {
                    return Err(PaneTreeError::PaneNotFound(PaneId(0)));
                }
                let new_size = (sizes[child_index] + delta).clamp(0.1, 0.9);
                let diff = new_size - sizes[child_index];

                // Take from siblings proportionally
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
                // Re-normalize
                let total: f32 = sizes.iter().sum();
                if total > 0.0 {
                    for size in sizes.iter_mut() {
                        *size /= total;
                    }
                }
                Ok(())
            }
            Self::Split { children, .. } => {
                // Recurse into children
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

    /// Iterate over all leaf panes in the tree, in depth-first order.
    pub fn leaf_panes(&self) -> Vec<(PaneId, &PlaceholderPane)> {
        match self {
            Self::Leaf { id, pane } => vec![(*id, pane)],
            Self::Split { children, .. } => children.iter().flat_map(|c| c.leaf_panes()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_tree() -> PaneNode {
        let leaf1 = PaneNode::new_leaf(PaneId(1), "Pane 1");
        let leaf2 = PaneNode::new_leaf(PaneId(2), "Pane 2");
        PaneNode::new_split(SplitId(10), SplitDirection::Horizontal, vec![leaf1, leaf2])
    }

    #[test]
    fn test_pane_node_leaf_creation() {
        let leaf = PaneNode::new_leaf(PaneId(1), "test");
        assert!(leaf.is_leaf());
        assert!(!leaf.is_split());
        assert_eq!(leaf.pane_id(), Some(PaneId(1)));
        assert_eq!(leaf.pane_count(), 1);
    }

    #[test]
    fn test_pane_node_split_creation() {
        let leaf1 = PaneNode::new_leaf(PaneId(1), "left");
        let leaf2 = PaneNode::new_leaf(PaneId(2), "right");
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
    fn test_split_at_not_found() {
        let mut root = make_test_tree();
        let result = root.split_at(PaneId(99), SplitDirection::Horizontal, PaneId(3), SplitId(11));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PaneTreeError::PaneNotFound(PaneId(99)));
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
    fn test_close_pane_not_found() {
        let mut root = make_test_tree();
        let result = root.close_pane(PaneId(99));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PaneTreeError::PaneNotFound(PaneId(99)));
    }

    #[test]
    fn test_close_last_pane_errors() {
        let mut root = PaneNode::new_leaf(PaneId(1), "only");
        let result = root.close_pane(PaneId(1));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PaneTreeError::CannotCloseLastPane);
    }

    #[test]
    fn test_find_leaf() {
        let root = make_test_tree();
        let found = root.find_leaf(PaneId(2));
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Pane 2");

        let not_found = root.find_leaf(PaneId(99));
        assert!(not_found.is_none());
    }

    #[test]
    fn test_split_direction() {
        assert!(SplitDirection::Horizontal.is_horizontal());
        assert!(!SplitDirection::Horizontal.is_vertical());
        assert!(SplitDirection::Vertical.is_vertical());
        assert!(!SplitDirection::Vertical.is_horizontal());
    }

    #[test]
    fn test_pane_count_deep_tree() {
        let leaf1 = PaneNode::new_leaf(PaneId(1), "a");
        let leaf2 = PaneNode::new_leaf(PaneId(2), "b");
        let left = PaneNode::new_split(SplitId(10), SplitDirection::Vertical, vec![leaf1, leaf2]);
        let leaf3 = PaneNode::new_leaf(PaneId(3), "c");
        let root = PaneNode::new_split(SplitId(20), SplitDirection::Horizontal, vec![left, leaf3]);
        assert_eq!(root.pane_count(), 3);
    }

    #[test]
    fn test_resize_split() {
        let leaf1 = PaneNode::new_leaf(PaneId(1), "left");
        let leaf2 = PaneNode::new_leaf(PaneId(2), "right");
        let mut root =
            PaneNode::new_split(SplitId(10), SplitDirection::Horizontal, vec![leaf1, leaf2]);

        let result = root.resize_split(SplitId(10), 0, 0.2);
        assert!(result.is_ok());

        if let PaneNode::Split { sizes, .. } = &root {
            assert!(sizes[0] > sizes[1]);
            let total: f32 = sizes.iter().sum();
            assert!((total - 1.0).abs() < 0.001);
        } else {
            panic!("Expected split node");
        }
    }

    #[test]
    fn test_resize_split_not_found() {
        let mut root = make_test_tree();
        let result = root.resize_split(SplitId(99), 0, 0.1);
        assert!(result.is_err());
    }

    #[test]
    fn test_nested_close_collapses_correctly() {
        let leaf1 = PaneNode::new_leaf(PaneId(1), "1");
        let leaf2 = PaneNode::new_leaf(PaneId(2), "2");
        let leaf3 = PaneNode::new_leaf(PaneId(3), "3");
        let inner = PaneNode::new_split(SplitId(10), SplitDirection::Vertical, vec![leaf2, leaf3]);
        let mut root =
            PaneNode::new_split(SplitId(20), SplitDirection::Horizontal, vec![leaf1, inner]);

        assert_eq!(root.pane_count(), 3);

        let result = root.close_pane(PaneId(2));
        assert!(result.is_ok());
        assert_eq!(root.pane_count(), 2);

        if let PaneNode::Split { children, .. } = &root {
            assert_eq!(children[0].pane_id(), Some(PaneId(1)));
            assert!(children[1].is_leaf());
            assert_eq!(children[1].pane_id(), Some(PaneId(3)));
        } else {
            panic!("Expected split node");
        }
    }
}
