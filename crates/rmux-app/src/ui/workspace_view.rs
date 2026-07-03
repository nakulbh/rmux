//! Workspace view — renders the pane tree as a split layout.
//!
//! This module renders the recursive `PaneNode` tree into the central panel
//! of the application window. Split nodes divide the available area among their
//! children. Leaf nodes render terminal panes.

use crate::workspace::splits::{PaneId, PaneNode, SplitDirection};
use egui::{Color32, Rect, Vec2};

/// Border width (in pixels) between split panes.
const SPLIT_BORDER: f32 = 2.0;

/// Default background color for the workspace area.
const WORKSPACE_BG: Color32 = Color32::from_rgb(20, 22, 28);

/// Render the pane tree into the given `egui::Ui`.
///
/// This function recursively traverses the pane tree and renders each leaf
/// as a terminal pane. Split nodes distribute the available area among their
/// children according to their size ratios.
pub fn render_pane_tree(ui: &mut egui::Ui, root: &mut PaneNode, active_pane: PaneId) {
    let available = ui.available_rect_before_wrap();

    if !ui.is_rect_visible(available) {
        return;
    }

    // Fill background
    ui.painter().rect_filled(available, 0.0, WORKSPACE_BG);

    // Render the tree recursively
    render_node(ui, root, available, active_pane);
}

/// Recursively render a single `PaneNode` within the given rectangle.
fn render_node(ui: &mut egui::Ui, node: &mut PaneNode, rect: Rect, active_pane: PaneId) {
    match node {
        PaneNode::Leaf { id, terminal } => {
            render_leaf(ui, *id, terminal.as_mut(), rect, *id == active_pane);
        }
        PaneNode::Split { direction, children, sizes, .. } => {
            render_split(ui, direction, children, sizes, rect, active_pane);
        }
    }
}

/// Render a leaf pane with its terminal.
fn render_leaf(
    ui: &mut egui::Ui,
    _id: PaneId,
    terminal: &mut Option<crate::ui::TerminalPane>,
    rect: Rect,
    _is_active: bool,
) {
    // Allocate a child UI for the terminal pane's region
    let mut child_ui =
        ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::default()));

    if let Some(pane) = terminal {
        pane.show(&mut child_ui);
    } else {
        // Show a loading placeholder if terminal hasn't been spawned yet
        let painter = child_ui.painter();
        painter.rect_filled(rect, 0.0, Color32::from_rgb(30, 32, 40));
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Spawning terminal...",
            egui::FontId::monospace(14.0),
            Color32::from_rgb(150, 150, 160),
        );
    }
}

/// Render a split node by dividing the rect among children.
fn render_split(
    ui: &mut egui::Ui,
    direction: &SplitDirection,
    children: &mut [PaneNode],
    sizes: &[f32],
    rect: Rect,
    active_pane: PaneId,
) {
    let is_horizontal = direction.is_horizontal();
    let available_dimension = if is_horizontal { rect.width() } else { rect.height() };
    let num_children = children.len();
    let total_borders = SPLIT_BORDER * (num_children.saturating_sub(1)) as f32;
    let usable_space = available_dimension - total_borders;

    let mut offset = 0.0f32;

    for (i, child) in children.iter_mut().enumerate() {
        let ratio = sizes.get(i).copied().unwrap_or(1.0 / num_children as f32);
        let child_size = usable_space * ratio;

        let child_rect = if is_horizontal {
            Rect::from_min_size(
                rect.left_top() + Vec2::new(offset, 0.0),
                Vec2::new(child_size, rect.height()),
            )
        } else {
            Rect::from_min_size(
                rect.left_top() + Vec2::new(0.0, offset),
                Vec2::new(rect.width(), child_size),
            )
        };

        render_node(ui, child, child_rect, active_pane);

        offset += child_size + SPLIT_BORDER;
    }
}
