//! Workspace view — renders the pane tree as a split layout.
//!
//! This module renders the recursive `PaneNode` tree into the central panel
//! of the application window. Split nodes divide the available area among their
//! children. Leaf nodes render colored placeholder rectangles representing
//! terminal panes.

use crate::workspace::splits::{PaneId, PaneNode, PlaceholderPane, SplitDirection};
use egui::{Color32, Rect, Vec2};

/// Border width (in pixels) between split panes.
const SPLIT_BORDER: f32 = 2.0;

/// Default background color for the workspace area.
const WORKSPACE_BG: Color32 = Color32::from_rgb(20, 22, 28);

/// Border color between panes.
const BORDER_COLOR: Color32 = Color32::from_rgb(35, 40, 50);

/// Highlight color for the active pane border.
const ACTIVE_BORDER_COLOR: Color32 = Color32::from_rgb(70, 130, 250);

/// Active pane indicator border width.
const ACTIVE_BORDER_WIDTH: f32 = 2.0;

/// Render the pane tree into the given `egui::Ui`.
///
/// This function recursively traverses the pane tree and renders each leaf
/// as a colored rectangle with its label. Split nodes distribute the available
/// area among their children according to their size ratios.
pub fn render_pane_tree(ui: &mut egui::Ui, root: &PaneNode, active_pane: PaneId) {
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
fn render_node(ui: &mut egui::Ui, node: &PaneNode, rect: Rect, active_pane: PaneId) {
    match node {
        PaneNode::Leaf { id, pane } => {
            render_leaf(ui, *id, pane, rect, *id == active_pane);
        }
        PaneNode::Split { direction, children, sizes, .. } => {
            render_split(ui, direction, children, sizes, rect, active_pane);
        }
    }
}

/// Render a leaf pane as a colored placeholder rectangle.
fn render_leaf(ui: &mut egui::Ui, id: PaneId, pane: &PlaceholderPane, rect: Rect, is_active: bool) {
    let painter = ui.painter();

    // Fill with the pane's assigned color
    painter.rect_filled(rect, 4.0, pane.color);

    // Active border
    if is_active {
        let border_stroke = egui::Stroke::new(ACTIVE_BORDER_WIDTH, ACTIVE_BORDER_COLOR);
        painter.rect_stroke(rect, 4.0, border_stroke, egui::StrokeKind::Middle);
    } else {
        let border_stroke = egui::Stroke::new(1.0, BORDER_COLOR);
        painter.rect_stroke(rect, 4.0, border_stroke, egui::StrokeKind::Middle);
    }

    // Pane label: ID and name
    let label = format!("#{}: {}", id.0, pane.name);
    let text_color = if is_active { Color32::WHITE } else { Color32::from_rgb(180, 180, 190) };

    let label_pos = rect.center() - Vec2::new(0.0, 8.0);
    painter.text(
        label_pos,
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(14.0),
        text_color,
    );

    // Subtitle hint for future terminal placement
    let hint = "[terminal pane]";
    let hint_pos = rect.center() + Vec2::new(0.0, 12.0);
    painter.text(
        hint_pos,
        egui::Align2::CENTER_CENTER,
        hint,
        egui::FontId::proportional(10.0),
        Color32::from_rgb(100, 100, 110),
    );
}

/// Render a split node by dividing the rect among children.
fn render_split(
    ui: &mut egui::Ui,
    direction: &SplitDirection,
    children: &[PaneNode],
    sizes: &[f32],
    rect: Rect,
    active_pane: PaneId,
) {
    let is_horizontal = direction.is_horizontal();
    let available_dimension = if is_horizontal { rect.width() } else { rect.height() };
    let total_borders = SPLIT_BORDER * (children.len().saturating_sub(1)) as f32;
    let usable_space = available_dimension - total_borders;

    let mut offset = 0.0f32;

    for (i, child) in children.iter().enumerate() {
        let ratio = sizes.get(i).copied().unwrap_or(1.0 / children.len() as f32);
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

/// Render a drag handle between split panes.
///
/// This is a stub for future resize-by-drag functionality.
#[allow(dead_code)]
fn render_drag_handle(
    ui: &mut egui::Ui,
    rect: Rect,
    _split_id: crate::workspace::splits::SplitId,
    _child_index: usize,
) {
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, BORDER_COLOR);

    // Draw subtle grip dots
    let center = rect.center();
    let dot_color = Color32::from_rgb(60, 65, 75);
    let dot_radius = 1.5;
    let dot_spacing = 6.0;

    if rect.width() > rect.height() {
        // Horizontal divider — dots along x
        for i in -1..=1 {
            let pos = center + Vec2::new(i as f32 * dot_spacing, 0.0);
            painter.circle_filled(pos, dot_radius, dot_color);
        }
    } else {
        // Vertical divider — dots along y
        for i in -1..=1 {
            let pos = center + Vec2::new(0.0, i as f32 * dot_spacing);
            painter.circle_filled(pos, dot_radius, dot_color);
        }
    }
}
