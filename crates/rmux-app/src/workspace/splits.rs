//! Pane tree model — the recursive tree structure that describes split layouts.
//!
//! Each workspace contains a pane tree. Interior nodes are `Split` nodes
//! that divide their area among children. Leaf nodes contain terminal panes.

#![allow(dead_code)]

use rmux_terminal::OscNotification;
use thiserror::Error;

use crate::browser::BrowserPane;
use crate::ui::TerminalPane;
use super::surface::Surface;

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

/// Spatial direction for pane focus movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialDirection {
    Left,
    Right,
    Up,
    Down,
}

/// A rectangle describing a pane's position in normalized coordinates (0.0..=1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaneRect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl PaneRect {
    /// Create a unit rectangle covering (0,0) to (1,1).
    pub fn unit() -> Self {
        Self { min_x: 0.0, min_y: 0.0, max_x: 1.0, max_y: 1.0 }
    }

    /// Compute the center point of this rectangle.
    pub fn center(&self) -> (f32, f32) {
        ((self.min_x + self.max_x) / 2.0, (self.min_y + self.max_y) / 2.0)
    }
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
    Leaf {
        id: PaneId,
        /// Legacy slot kept for backward compat with `find_terminal_mut` /
        /// `set_terminal`. Future waves will migrate these to operate on
        /// `surfaces` directly.
        terminal: Box<Option<TerminalPane>>,
        /// Index into `surfaces` of the focused surface. Stays 0 when
        /// `surfaces` is empty (the default for an uninitialized leaf).
        active_surface: usize,
        /// The list of surfaces (tabs) in this leaf. May be empty for an
        /// uninitialized leaf; in that case `terminal` is the source of
        /// truth and `terminal_count()` reports 1.
        surfaces: Vec<Surface>,
    },
    Browser { id: PaneId, browser: Box<BrowserPane> },
    Split { id: SplitId, direction: SplitDirection, children: Vec<PaneNode>, sizes: Vec<f32> },
}

impl std::fmt::Debug for PaneNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Leaf { id, terminal, surfaces, active_surface, .. } => f
                .debug_struct("Leaf")
                .field("id", id)
                .field("has_terminal", &terminal.is_some())
                .field("surfaces", &surfaces.len())
                .field("active_surface", active_surface)
                .finish(),
            Self::Browser { id, browser } => {
                f.debug_struct("Browser").field("id", id).field("url", &browser.url()).finish()
            }
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
        Self::Leaf {
            id,
            terminal: Box::new(None),
            active_surface: 0,
            surfaces: Vec::new(),
        }
    }

    pub fn new_leaf_with_terminal(id: PaneId, terminal: TerminalPane) -> Self {
        Self::Leaf {
            id,
            terminal: Box::new(Some(terminal)),
            active_surface: 0,
            surfaces: Vec::new(),
        }
    }

    /// Build a Leaf with a single surface wrapping `terminal`.
    pub fn leaf_with_surfaces(id: PaneId, terminal: TerminalPane) -> Self {
        let surface = Surface::new(super::surface::SurfaceId(1), "Terminal 1", terminal);
        Self::Leaf {
            id,
            terminal: Box::new(None),
            active_surface: 0,
            surfaces: vec![surface],
        }
    }

    pub fn new_browser(id: PaneId, browser: BrowserPane) -> Self {
        Self::Browser { id, browser: Box::new(browser) }
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

    pub fn is_browser(&self) -> bool {
        matches!(self, Self::Browser { .. })
    }

    pub fn is_split(&self) -> bool {
        matches!(self, Self::Split { .. })
    }

    pub fn pane_id(&self) -> Option<PaneId> {
        match self {
            Self::Leaf { id, .. } | Self::Browser { id, .. } => Some(*id),
            Self::Split { .. } => None,
        }
    }

    pub fn find_terminal_mut(&mut self, target: PaneId) -> Option<&mut Option<TerminalPane>> {
        match self {
            Self::Leaf { id, terminal, .. } if *id == target => Some(terminal.as_mut()),
            Self::Leaf { .. } => None,
            Self::Browser { .. } => None,
            Self::Split { children, .. } => {
                children.iter_mut().find_map(|c| c.find_terminal_mut(target))
            }
        }
    }

    pub fn get_terminal(&mut self, target: PaneId) -> Option<&mut TerminalPane> {
        self.find_terminal_mut(target).and_then(|opt| opt.as_mut())
    }

    pub fn find_browser_mut(&mut self, target: PaneId) -> Option<&mut BrowserPane> {
        match self {
            Self::Browser { id, browser } if *id == target => Some(browser.as_mut()),
            Self::Browser { .. } => None,
            Self::Leaf { .. } => None,
            Self::Split { children, .. } => {
                children.iter_mut().find_map(|c| c.find_browser_mut(target))
            }
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

    /// Find an immutable reference to a pane node by ID anywhere in the
    /// tree. Mirror of [`Self::find_pane_mut`] for `&self` access.
    pub fn find_pane(&self, target: PaneId) -> Option<&PaneNode> {
        if self.pane_id() == Some(target) {
            return Some(self);
        }
        if let Self::Split { children, .. } = self {
            for child in children.iter() {
                let found = child.find_pane(target);
                if found.is_some() {
                    return found;
                }
            }
        }
        None
    }

    /// Replace the pane at `target` with `new_node` in the tree.
    ///
    /// Uses `find_pane_mut` to locate the target without creating
    /// sacrificial probe nodes. Returns `true` if replacement succeeded.
    pub fn replace_pane(&mut self, target: PaneId, new_node: PaneNode) -> bool {
        if let Some(node) = self.find_pane_mut(target) {
            *node = new_node;
            true
        } else {
            false
        }
    }

    /// Check if the node at `target` is a browser pane.
    pub fn is_browser_pane(&self, target: PaneId) -> bool {
        match self {
            Self::Browser { id, .. } => *id == target,
            Self::Leaf { .. } => false,
            Self::Split { children, .. } => children.iter().any(|c| c.is_browser_pane(target)),
        }
    }

    /// Process PTY output for every pane in this subtree, collecting any
    /// OSC notifications (tagged with their pane id) into `notifications`.
    pub fn process_pty_outputs(&mut self, notifications: &mut Vec<(PaneId, OscNotification)>) {
        match self {
            Self::Leaf { id, terminal, .. } => {
                if let Some(t) = terminal.as_mut() {
                    t.process_pty_output();
                    notifications.extend(t.take_notifications().into_iter().map(|n| (*id, n)));
                }
            }
            Self::Browser { .. } => {}
            Self::Split { children, .. } => {
                for child in children.iter_mut() {
                    child.process_pty_outputs(notifications);
                }
            }
        }
    }

    pub fn pane_ids(&self) -> Vec<PaneId> {
        match self {
            Self::Leaf { id, .. } | Self::Browser { id, .. } => vec![*id],
            Self::Split { children, .. } => children.iter().flat_map(|c| c.pane_ids()).collect(),
        }
    }

    pub fn pane_count(&self) -> usize {
        match self {
            Self::Leaf { .. } | Self::Browser { .. } => 1,
            Self::Split { children, .. } => children.iter().map(|c| c.pane_count()).sum(),
        }
    }

    pub fn find_leaf(&self, target: PaneId) -> Option<&Option<TerminalPane>> {
        match self {
            Self::Leaf { id, terminal, .. } if *id == target => Some(terminal.as_ref()),
            Self::Leaf { .. } => None,
            Self::Browser { .. } => None,
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
        if let Self::Leaf { id, .. } | Self::Browser { id, .. } = self {
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
                if let Self::Leaf { id, .. } | Self::Browser { id, .. } = child {
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
            Self::Leaf { .. } | Self::Browser { .. } => {
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
            Self::Leaf { id, terminal, .. } => vec![(*id, terminal.as_ref().as_ref())],
            Self::Browser { .. } => vec![],
            Self::Split { children, .. } => children.iter().flat_map(|c| c.leaf_panes()).collect(),
        }
    }

    /// Collect all leaf and browser pane rectangles into `out`.
    ///
    /// `rect` is the area allocated to this node in normalized coordinates.
    /// Split children are assigned sub-rectangles proportional to their sizes.
    pub fn collect_pane_rects(&self, rect: &PaneRect, out: &mut Vec<(PaneId, PaneRect)>) {
        match self {
            Self::Leaf { id, .. } | Self::Browser { id, .. } => {
                out.push((*id, *rect));
            }
            Self::Split { direction, children, sizes, .. } => {
                let is_horizontal = direction.is_horizontal();
                let available =
                    if is_horizontal { rect.max_x - rect.min_x } else { rect.max_y - rect.min_y };
                let num_children = children.len();
                let mut offset = 0.0_f32;

                for (i, child) in children.iter().enumerate() {
                    let ratio = sizes.get(i).copied().unwrap_or(1.0_f32 / num_children as f32);
                    let child_size = available * ratio;

                    let child_rect = if is_horizontal {
                        PaneRect {
                            min_x: rect.min_x + offset,
                            min_y: rect.min_y,
                            max_x: rect.min_x + offset + child_size,
                            max_y: rect.max_y,
                        }
                    } else {
                        PaneRect {
                            min_x: rect.min_x,
                            min_y: rect.min_y + offset,
                            max_x: rect.max_x,
                            max_y: rect.min_y + offset + child_size,
                        }
                    };

                    child.collect_pane_rects(&child_rect, out);
                    offset += child_size;
                }
            }
        }
    }
}

impl PaneNode {
    pub fn leaf_panes_mut(&mut self) -> Vec<(PaneId, &mut Option<TerminalPane>)> {
        match self {
            Self::Leaf { id, terminal, .. } => vec![(*id, terminal.as_mut())],
            Self::Browser { .. } => vec![],
            Self::Split { children, .. } => {
                children.iter_mut().flat_map(|c| c.leaf_panes_mut()).collect()
            }
        }
    }

    /// Recursively equalize all split ratios in the tree.
    ///
    /// For every `Split` node, each child gets an equal share of the
    /// available space. Leaf nodes are unaffected.
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

    /// Collect pane IDs whose terminal process has exited.
    pub fn collect_exited_panes(&self) -> Vec<PaneId> {
        match self {
            Self::Leaf { id, terminal, .. } => {
                if terminal.as_ref().as_ref().is_some_and(|t| t.is_exited()) {
                    vec![*id]
                } else {
                    vec![]
                }
            }
            Self::Browser { .. } => vec![],
            Self::Split { children, .. } => {
                children.iter().flat_map(|c| c.collect_exited_panes()).collect()
            }
        }
    }

    /// The list of surfaces (tabs) in this leaf, or an empty slice for
    /// non-leaf nodes.
    pub fn leaf_surfaces(&self) -> &[Surface] {
        match self {
            Self::Leaf { surfaces, .. } => surfaces,
            _ => &[],
        }
    }

    /// Mutable access to the leaf's surface list. Panics if called on a
    /// non-leaf node — callers should gate with `is_leaf()` first.
    pub fn leaf_surfaces_mut(&mut self) -> &mut Vec<Surface> {
        match self {
            Self::Leaf { surfaces, .. } => surfaces,
            _ => panic!("leaf_surfaces_mut called on non-leaf node"),
        }
    }

    /// Index of the currently focused surface. Returns 0 for non-leaf
    /// nodes; use [`Self::active_surface`] to check for `Some`.
    pub fn active_surface_index(&self) -> usize {
        match self {
            Self::Leaf { active_surface, .. } => *active_surface,
            _ => 0,
        }
    }

    /// Set the focused surface index without bounds checking. The caller
    /// is responsible for keeping `idx < leaf_surfaces().len()`.
    pub fn set_active_surface_index(&mut self, idx: usize) {
        if let Self::Leaf { active_surface, .. } = self {
            *active_surface = idx;
        }
    }

    /// Append a new surface to the leaf. No-op for non-leaf nodes.
    pub fn add_surface(&mut self, surface: Surface) {
        if let Self::Leaf { surfaces, .. } = self {
            surfaces.push(surface);
        }
    }

    /// Remove the surface at `idx` and adjust `active_surface` so focus
    /// stays close to the removed slot. Clamps to `len - 1` when the
    /// active index was past the new end. Returns `None` for out-of-bounds
    /// `idx` or non-leaf nodes.
    pub fn remove_surface(&mut self, idx: usize) -> Option<Surface> {
        let Self::Leaf { surfaces, active_surface, .. } = self else {
            return None;
        };
        if idx >= surfaces.len() {
            return None;
        }
        let removed = surfaces.remove(idx);
        let new_len = surfaces.len();
        if *active_surface >= new_len {
            *active_surface = new_len.saturating_sub(1);
        } else if *active_surface == idx {
            *active_surface = (*active_surface).min(new_len.saturating_sub(1));
        }
        Some(removed)
    }

    /// The currently focused surface, or `None` for a leaf with no
    /// surfaces (or for a non-leaf node).
    pub fn active_surface(&self) -> Option<&Surface> {
        match self {
            Self::Leaf { surfaces, active_surface, .. } => surfaces.get(*active_surface),
            _ => None,
        }
    }

    /// Mutable reference to the currently focused surface.
    pub fn active_surface_mut(&mut self) -> Option<&mut Surface> {
        match self {
            Self::Leaf { surfaces, active_surface, .. } => surfaces.get_mut(*active_surface),
            _ => None,
        }
    }

    /// The active surface's terminal, falling back to the legacy
    /// `terminal` slot when the surfaces list is empty. This keeps
    /// uninitialized leaves (legacy `set_terminal` flow) working until
    /// callers are migrated to operate on surfaces directly.
    pub fn active_terminal(&self) -> Option<&TerminalPane> {
        match self {
            Self::Leaf { surfaces, active_surface, terminal, .. } => surfaces
                .get(*active_surface)
                .map(|s| &s.terminal)
                .or_else(|| terminal.as_ref().as_ref()),
            _ => None,
        }
    }

    /// Mutable reference to the active surface's terminal. Same fallback
    /// rules as [`Self::active_terminal`].
    pub fn active_terminal_mut(&mut self) -> Option<&mut TerminalPane> {
        match self {
            Self::Leaf { surfaces, active_surface, terminal, .. } => {
                if surfaces.is_empty() {
                    terminal.as_mut().as_mut()
                } else {
                    surfaces.get_mut(*active_surface).map(|s| &mut s.terminal)
                }
            }
            _ => None,
        }
    }

    /// Number of terminals in this leaf. Returns `max(surfaces.len(), 1)`
    /// so an uninitialized leaf (empty `surfaces`) still counts as 1,
    /// matching the pre-existing `pane_count()` semantics.
    pub fn terminal_count(&self) -> usize {
        match self {
            Self::Leaf { surfaces, .. } => surfaces.len().max(1),
            _ => 0,
        }
    }
}

/// Find the spatially nearest pane in `direction` from `from`.
///
/// Walks the pane tree to collect normalized rectangles for every leaf/browser
/// pane, then scores candidates by alignment and distance.
pub fn find_pane_in_direction(
    root: &PaneNode,
    from: PaneId,
    direction: SpatialDirection,
) -> Option<PaneId> {
    let mut rects = Vec::new();
    root.collect_pane_rects(&PaneRect::unit(), &mut rects);

    let from_rect = rects.iter().find(|(id, _)| *id == from).map(|(_, r)| *r)?;
    let from_center = from_rect.center();

    let mut best: Option<(PaneId, f32)> = None;

    for (id, rect) in rects {
        if id == from {
            continue;
        }

        let center = rect.center();
        let delta_x = center.0 - from_center.0;
        let delta_y = center.1 - from_center.1;

        let in_direction = match direction {
            SpatialDirection::Left => delta_x < 0.0,
            SpatialDirection::Right => delta_x > 0.0,
            SpatialDirection::Up => delta_y < 0.0,
            SpatialDirection::Down => delta_y > 0.0,
        };

        if !in_direction {
            continue;
        }

        let score = match direction {
            SpatialDirection::Left | SpatialDirection::Right => delta_y.abs() * 2.0 + delta_x.abs(),
            SpatialDirection::Up | SpatialDirection::Down => delta_x.abs() * 2.0 + delta_y.abs(),
        };

        if best.map(|(_, s)| score < s).unwrap_or(true) {
            best = Some((id, score));
        }
    }

    best.map(|(id, _)| id)
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

    #[test]
    fn test_equalize_splits_single_leaf() {
        let mut root = PaneNode::new_leaf(PaneId(1));
        // Should not panic on a leaf
        root.equalize_splits();
    }

    #[test]
    fn test_equalize_splits_already_equal() {
        let leaf1 = PaneNode::new_leaf(PaneId(1));
        let leaf2 = PaneNode::new_leaf(PaneId(2));
        let mut root =
            PaneNode::new_split(SplitId(10), SplitDirection::Horizontal, vec![leaf1, leaf2]);
        // Already 0.5 / 0.5
        root.equalize_splits();
        if let PaneNode::Split { sizes, .. } = &root {
            assert_eq!(sizes.len(), 2);
            assert!((sizes[0] - 0.5).abs() < f32::EPSILON);
            assert!((sizes[1] - 0.5).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn test_equalize_splits_unequal() {
        let leaf1 = PaneNode::new_leaf(PaneId(1));
        let leaf2 = PaneNode::new_leaf(PaneId(2));
        let mut root = PaneNode::Split {
            id: SplitId(10),
            direction: SplitDirection::Horizontal,
            children: vec![leaf1, leaf2],
            sizes: vec![0.3, 0.7],
        };
        root.equalize_splits();
        if let PaneNode::Split { sizes, .. } = &root {
            assert_eq!(sizes.len(), 2);
            assert!((sizes[0] - 0.5).abs() < f32::EPSILON);
            assert!((sizes[1] - 0.5).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn test_equalize_splits_nested() {
        // Create a 3-pane layout via split
        let mut root = make_test_tree(); // pane 1, 2
        root.split_at(PaneId(1), SplitDirection::Vertical, PaneId(3), SplitId(11)).unwrap();
        assert_eq!(root.pane_count(), 3);

        // Unequalize some split
        if let PaneNode::Split { sizes, .. } = &mut root {
            sizes[0] = 0.2;
            sizes[1] = 0.8;
        }
        root.equalize_splits();
        // Top-level: 2 children -> each 0.5
        if let PaneNode::Split { sizes, .. } = &root {
            assert_eq!(sizes.len(), 2);
            assert!((sizes[0] - 0.5).abs() < f32::EPSILON);
            assert!((sizes[1] - 0.5).abs() < f32::EPSILON);
        }
    }

    // ----- W2.1: Surface accessors on PaneNode::Leaf -----

    use crate::ui::TerminalPane;
    use super::super::surface::{Surface, SurfaceId};

    fn make_surface(id: u64, title: &str) -> Surface {
        let term = TerminalPane::spawn(1, 1, 14.0).expect("dummy terminal spawn");
        Surface::new(SurfaceId(id), title, term)
    }

    #[test]
    fn test_leaf_holds_multiple_surfaces() {
        let mut leaf = PaneNode::new_leaf(PaneId(1));
        assert_eq!(leaf.terminal_count(), 1);
        assert_eq!(leaf.leaf_surfaces().len(), 0);

        leaf.add_surface(make_surface(1, "Terminal 1"));
        leaf.add_surface(make_surface(2, "Terminal 2"));
        leaf.add_surface(make_surface(3, "Terminal 3"));

        assert_eq!(leaf.leaf_surfaces().len(), 3);
        assert_eq!(leaf.terminal_count(), 3);
        assert_eq!(leaf.leaf_surfaces()[0].title, "Terminal 1");
        assert_eq!(leaf.leaf_surfaces()[2].title, "Terminal 3");
    }

    #[test]
    fn test_active_surface_default_is_zero() {
        let leaf = PaneNode::new_leaf(PaneId(1));
        assert_eq!(leaf.active_surface_index(), 0);
        assert!(leaf.active_surface().is_none());
        assert!(leaf.active_terminal().is_none());
    }

    #[test]
    fn test_remove_surface_clamps_active_index() {
        let mut leaf = PaneNode::new_leaf(PaneId(1));
        leaf.add_surface(make_surface(1, "A"));
        leaf.add_surface(make_surface(2, "B"));
        leaf.add_surface(make_surface(3, "C"));
        leaf.set_active_surface_index(1);
        assert_eq!(leaf.active_surface_index(), 1);

        // Remove the active surface. Focus stays at idx 1, which now
        // holds "C" (was idx 2). min(removed_idx, new_len-1) = min(1, 1) = 1.
        let removed = leaf.remove_surface(1);
        assert!(removed.is_some());
        assert_eq!(leaf.leaf_surfaces().len(), 2);
        assert_eq!(leaf.active_surface_index(), 1);
        assert_eq!(leaf.leaf_surfaces()[1].title, "C");

        // Active was beyond the new len — clamp to last valid index.
        leaf.set_active_surface_index(99);
        let _ = leaf.remove_surface(1);
        assert_eq!(leaf.leaf_surfaces().len(), 1);
        assert_eq!(leaf.active_surface_index(), 0);

        // Removing the only remaining surface leaves an empty leaf.
        // saturating_sub on an empty vec is 0.
        let _ = leaf.remove_surface(0);
        assert_eq!(leaf.leaf_surfaces().len(), 0);
        assert_eq!(leaf.active_surface_index(), 0);
    }

    #[test]
    fn test_active_terminal_returns_active_surface_terminal() {
        let mut leaf = PaneNode::new_leaf(PaneId(1));
        leaf.add_surface(make_surface(1, "First"));
        leaf.add_surface(make_surface(2, "Second"));

        assert!(leaf.active_terminal().is_some());
        assert_eq!(leaf.active_surface().unwrap().title, "First");
        let first_name = leaf.leaf_surfaces()[0].terminal.name();
        assert_eq!(leaf.active_terminal().unwrap().name(), first_name);

        leaf.set_active_surface_index(1);
        assert_eq!(leaf.active_surface().unwrap().title, "Second");
        let second_name = leaf.leaf_surfaces()[1].terminal.name();
        assert_eq!(leaf.active_terminal().unwrap().name(), second_name);

        let term_mut = leaf.active_terminal_mut().expect("active terminal");
        assert!(!term_mut.has_focus());
    }

    #[test]
    fn test_add_surface_does_not_change_active() {
        let mut leaf = PaneNode::new_leaf(PaneId(1));
        leaf.add_surface(make_surface(1, "A"));
        leaf.set_active_surface_index(0);
        assert_eq!(leaf.active_surface_index(), 0);

        leaf.add_surface(make_surface(2, "B"));
        assert_eq!(leaf.active_surface_index(), 0);
        assert_eq!(leaf.active_surface().unwrap().title, "A");

        leaf.add_surface(make_surface(3, "C"));
        leaf.add_surface(make_surface(4, "D"));
        assert_eq!(leaf.active_surface_index(), 0);
        assert_eq!(leaf.active_surface().unwrap().title, "A");
    }
}
