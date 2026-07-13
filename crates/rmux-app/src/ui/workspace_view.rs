//! Workspace view — renders the pane tree as a split layout.
//!
//! This module renders the recursive `PaneNode` tree into the central panel
//! of the application window. Split nodes divide the available area among their
//! children. Leaf nodes render terminal panes. When a pane is zoomed (maximized),
//! only that pane is rendered at full workspace size.

use crate::ui::theme;
use crate::workspace::splits::{PaneId, PaneNode, SplitDirection, SplitId};
use egui::{Rect, Vec2};

/// Border width (in pixels) between split panes.
const SPLIT_BORDER: f32 = 1.0;

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
    let palette = theme::palette();
    ui.painter().rect_filled(available, 0.0_f32, palette.app_bg);

    // If a pane is zoomed, render only that pane
    if let Some(zoom_id) = zoomed_pane {
        if let Some(terminal) = root.find_terminal_mut(zoom_id) {
            render_leaf(ui, zoom_id, terminal, available, zoom_id == *active_pane, active_pane);
        } else if let Some(browser) = root.find_browser_mut(zoom_id) {
            render_browser(ui, zoom_id, browser, available, zoom_id == *active_pane, active_pane);
        }

        // Zoom indicator: chrome pill in the top-right corner
        let label_rect = Rect::from_min_size(
            available.right_top() - Vec2::new(220.0_f32, -2.0_f32),
            Vec2::new(216.0_f32, 18.0_f32),
        );
        let modifier = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };
        ui.painter().rect_filled(label_rect, egui::CornerRadius::same(6), palette.chrome_bg);
        ui.painter().rect_stroke(
            label_rect,
            egui::CornerRadius::same(6),
            egui::Stroke::new(1.0_f32, palette.chrome_border),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            label_rect.left_center() + Vec2::new(8.0_f32, 0.0_f32),
            egui::Align2::LEFT_CENTER,
            format!("Zoom: {modifier}+Shift+Enter to restore"),
            egui::FontId::proportional(10.0_f32),
            palette.text_muted,
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
    // The match borrows `node`. We collect a resize request (if any) so we can
    // apply it *after* the match releases the borrow.
    let resize_request: Option<(SplitId, usize, f32)> = match node {
        PaneNode::Leaf { id, terminal, .. } => {
            let is_active = *id == *active_pane;
            render_leaf(ui, *id, terminal.as_mut(), rect, is_active, active_pane);
            None
        }
        PaneNode::Browser { id, browser } => {
            let is_active = *id == *active_pane;
            render_browser(ui, *id, browser.as_mut(), rect, is_active, active_pane);
            None
        }
        PaneNode::Split { id, direction, children, sizes } => {
            let split_id = *id;
            render_split(ui, direction, children, sizes, rect, active_pane)
                .map(|(child_idx, delta)| (split_id, child_idx, delta))
        }
    };
    // Borrow of `node` ends here; safe to call resize_split mutably.
    if let Some((split_id, child_idx, delta)) = resize_request {
        let _ = node.resize_split(split_id, child_idx, delta);
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
        let palette = theme::palette();
        painter.rect_filled(rect, 0.0_f32, palette.panel_bg);
        painter.rect_stroke(
            rect.shrink(0.5_f32),
            egui::CornerRadius::ZERO,
            egui::Stroke::new(1.0_f32, palette.border),
            egui::StrokeKind::Inside,
        );
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Spawning terminal…",
            egui::FontId::monospace(12.0_f32),
            palette.text_muted,
        );
    }
}

/// Render a browser pane — delegates to [`crate::browser::webview::render_browser_pane`].
fn render_browser(
    ui: &mut egui::Ui,
    pane_id: PaneId,
    browser: &mut crate::browser::BrowserPane,
    rect: Rect,
    is_active: bool,
    active_pane: &mut PaneId,
) {
    crate::browser::webview::render_browser_pane(
        ui,
        pane_id,
        browser,
        rect,
        is_active,
        active_pane,
    );
}

/// Render a split node by dividing the rect among children.
fn render_split(
    ui: &mut egui::Ui,
    direction: &SplitDirection,
    children: &mut [PaneNode],
    sizes: &[f32],
    rect: Rect,
    active_pane: &mut PaneId,
) -> Option<(usize, f32)> {
    let is_horizontal = direction.is_horizontal();
    let available_dimension = if is_horizontal { rect.width() } else { rect.height() };
    let num_children = children.len();
    let total_borders = SPLIT_BORDER * (num_children.saturating_sub(1)) as f32;
    let usable_space = available_dimension - total_borders;

    let palette = theme::palette();
    let mut offset = 0.0_f32;
    let mut resize_request: Option<(usize, f32)> = None;

    // 2.5 px of invisible padding on each side of the 1-px hairline → 6-px total hit width.
    const HIT_HALF_EXTRA: f32 = 2.5_f32;

    for (i, child) in children.iter_mut().enumerate() {
        let ratio = sizes.get(i).copied().unwrap_or(1.0_f32 / num_children as f32);
        let child_size = usable_space * ratio;

        let child_rect = if is_horizontal {
            Rect::from_min_size(
                rect.left_top() + Vec2::new(offset, 0.0_f32),
                Vec2::new(child_size, rect.height()),
            )
        } else {
            Rect::from_min_size(
                rect.left_top() + Vec2::new(0.0_f32, offset),
                Vec2::new(rect.width(), child_size),
            )
        };

        render_node(ui, child, child_rect, active_pane);

        if i + 1 < num_children {
            let hairline_min = if is_horizontal {
                rect.left_top() + Vec2::new(offset + child_size, 0.0_f32)
            } else {
                rect.left_top() + Vec2::new(0.0_f32, offset + child_size)
            };

            let divider_rect = if is_horizontal {
                Rect::from_min_size(hairline_min, Vec2::new(SPLIT_BORDER, rect.height()))
            } else {
                Rect::from_min_size(hairline_min, Vec2::new(rect.width(), SPLIT_BORDER))
            };

            let hit_rect = if is_horizontal {
                Rect::from_min_size(
                    hairline_min - Vec2::new(HIT_HALF_EXTRA, 0.0_f32),
                    Vec2::new(SPLIT_BORDER + 2.0_f32 * HIT_HALF_EXTRA, rect.height()),
                )
            } else {
                Rect::from_min_size(
                    hairline_min - Vec2::new(0.0_f32, HIT_HALF_EXTRA),
                    Vec2::new(rect.width(), SPLIT_BORDER + 2.0_f32 * HIT_HALF_EXTRA),
                )
            };

            let response = ui.allocate_rect(hit_rect, egui::Sense::drag());

            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(if is_horizontal {
                    egui::CursorIcon::ResizeHorizontal
                } else {
                    egui::CursorIcon::ResizeVertical
                });
            }

            if response.dragged() && usable_space > 0.0_f32 {
                let pixel_delta =
                    if is_horizontal { response.drag_delta().x } else { response.drag_delta().y };
                resize_request = Some((i, pixel_delta / usable_space));
            }

            if response.dragged() {
                let accent_rect = if is_horizontal {
                    Rect::from_min_size(
                        hairline_min - Vec2::new(0.5_f32, 0.0_f32),
                        Vec2::new(2.0_f32, rect.height()),
                    )
                } else {
                    Rect::from_min_size(
                        hairline_min - Vec2::new(0.0_f32, 0.5_f32),
                        Vec2::new(rect.width(), 2.0_f32),
                    )
                };
                ui.painter().rect_filled(accent_rect, 0.0_f32, palette.accent);
            } else if response.hovered() {
                ui.painter().rect_filled(
                    divider_rect,
                    0.0_f32,
                    palette.border.gamma_multiply(1.5_f32),
                );
            } else {
                ui.painter().rect_filled(divider_rect, 0.0_f32, palette.border);
            }
        }

        offset += child_size + SPLIT_BORDER;
    }

    resize_request
}
