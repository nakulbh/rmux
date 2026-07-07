//! Workspace view — renders the pane tree as a split layout.
//!
//! This module renders the recursive `PaneNode` tree into the central panel
//! of the application window. Split nodes divide the available area among their
//! children. Leaf nodes render terminal panes. When a pane is zoomed (maximized),
//! only that pane is rendered at full workspace size.

use crate::workspace::splits::{PaneId, PaneNode, SplitDirection};
use egui::{Color32, Rect, Vec2};

/// Border width (in pixels) between split panes.
const SPLIT_BORDER: f32 = 2.0;

/// Default background color for the workspace area.
const WORKSPACE_BG: Color32 = Color32::from_rgb(20, 22, 28);

/// Color for the zoom indicator label.
const ZOOM_LABEL_COLOR: Color32 = Color32::from_rgb(100, 120, 160);

/// Render the pane tree into the given `egui::Ui`, optionally zoomed to a
/// single pane.
///
/// When `zoomed_pane` is `Some(id)`, only that pane is rendered at full
/// workspace size. A small label in the top-right corner reminds the user
/// how to restore the layout.
pub fn render_pane_tree(
    ui: &mut egui::Ui,
    root: &mut PaneNode,
    active_pane: &mut PaneId,
    zoomed_pane: Option<PaneId>,
) {
    let available = ui.available_rect_before_wrap();

    if !ui.is_rect_visible(available) {
        return;
    }

    // Fill background
    ui.painter().rect_filled(available, 0.0, WORKSPACE_BG);

    // If a pane is zoomed, render only that pane
    if let Some(zoom_id) = zoomed_pane
        && let Some(terminal) = root.find_terminal_mut(zoom_id)
    {
        render_leaf(ui, zoom_id, terminal, available, zoom_id == *active_pane, active_pane);

        // Zoom indicator label in top-right corner
        let label_rect = Rect::from_min_size(
            available.right_top() - Vec2::new(220.0, -2.0),
            Vec2::new(216.0, 18.0),
        );
        let modifier = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };
        ui.painter().text(
            label_rect.left_top(),
            egui::Align2::LEFT_TOP,
            format!("Zoom: {modifier}+Shift+Enter to restore"),
            egui::FontId::proportional(10.0),
            ZOOM_LABEL_COLOR,
        );
        return;
    }
    // Zoomed pane not found in tree (e.g. was closed) — fall through
    // to render the full tree.

    // Render the tree recursively
    render_node(ui, root, available, active_pane);
}

/// Recursively render a single `PaneNode` within the given rectangle.
fn render_node(ui: &mut egui::Ui, node: &mut PaneNode, rect: Rect, active_pane: &mut PaneId) {
    match node {
        PaneNode::Leaf { id, terminal } => {
            let is_active = *id == *active_pane;
            render_leaf(ui, *id, terminal.as_mut(), rect, is_active, active_pane);
        }
        PaneNode::Split { direction, children, sizes, .. } => {
            render_split(ui, direction, children, sizes, rect, active_pane);
        }
    }
}

/// Render a leaf pane with its terminal.
fn render_leaf(
    ui: &mut egui::Ui,
    id: PaneId,
    terminal: &mut Option<crate::ui::TerminalPane>,
    rect: Rect,
    is_active: bool,
    active_pane: &mut PaneId,
) {
    let mut child_ui =
        ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::default()));

    if let Some(pane) = terminal {
        pane.show(&mut child_ui);
        // Update workspace-level active pane when this pane gains focus via click
        if pane.has_focus() && !is_active {
            *active_pane = id;
        }
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
    active_pane: &mut PaneId,
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
