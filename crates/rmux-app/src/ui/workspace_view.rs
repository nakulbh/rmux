//! Workspace view — renders the pane tree as a split layout.
//!
//! This module renders the recursive `PaneNode` tree into the central panel
//! of the application window. Split nodes divide the available area among their
//! children. Leaf nodes render terminal panes (with a tab bar above the
//! terminal area when the leaf holds more than one surface). When a pane is
//! zoomed (maximized), only that pane is rendered at full workspace size.

use crate::ui::theme;
use crate::workspace::WorkspaceManager;
use crate::workspace::splits::{PaneId, PaneNode, SplitDirection, SplitId};
use egui::{Rect, Vec2};

const SPLIT_BORDER: f32 = 1.0;
const TAB_BAR_HEIGHT: f32 = 24.0;

/// Actions emitted by the tab bar during rendering. They are collected
/// into a `Vec` and applied to the [`WorkspaceManager`] after the
/// tree-walk's mutable borrow is released, avoiding an unworkable
/// `&mut Workspace` + `&mut WorkspaceManager` overlap.
#[derive(Debug, Clone, Copy)]
enum TabAction {
    Select(usize),
    Close(usize),
    New,
}

/// Render the pane tree into the given `egui::Ui`, optionally zoomed to a
/// single pane.
///
/// When `zoomed_pane` is `Some(id)`, only that pane is rendered at full
/// workspace size. A small label in the top-right corner reminds the user
/// how to restore the layout.
///
/// `manager` is used both to read the active workspace (during render)
/// and to apply deferred tab-bar actions (after render). Actions emitted
/// by the tab bar are buffered in a `Vec` and replayed against the
/// manager once the tree-walk's mutable borrow has ended.
pub fn render_pane_tree(
    ui: &mut egui::Ui,
    manager: &mut WorkspaceManager,
    zoomed_pane: Option<PaneId>,
) {
    let available = ui.available_rect_before_wrap();

    if !ui.is_rect_visible(available) {
        return;
    }

    let palette = theme::palette();
    ui.painter().rect_filled(available, 0.0_f32, palette.app_bg);

    let mut actions: Vec<TabAction> = Vec::new();

    if let Some(zoom_id) = zoomed_pane {
        let ws = manager.active_mut();
        if let Some(leaf) = ws.root.find_pane_mut(zoom_id) {
            let is_active = zoom_id == ws.active_pane;
            render_leaf(ui, leaf, available, is_active, &mut ws.active_pane, &mut actions);
        } else if let Some(browser) = ws.root.find_browser_mut(zoom_id) {
            render_browser(ui, browser, available, zoom_id == ws.active_pane, &mut ws.active_pane);
        }

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
    } else {
        let ws = manager.active_mut();
        render_node(ui, &mut ws.root, available, &mut ws.active_pane, &mut actions);
    }

    // Replay buffered tab-bar actions now that the tree-walk's `&mut
    // Workspace` borrow has ended.
    for action in actions {
        match action {
            TabAction::Select(idx) => {
                if let Err(e) = manager.select_surface_in_active(idx) {
                    tracing::warn!(error = %e, "select_surface_in_active failed");
                }
            }
            TabAction::Close(idx) => {
                if let Err(e) = manager.close_surface_in_active(Some(idx)) {
                    tracing::warn!(error = %e, "close_surface_in_active failed");
                }
            }
            TabAction::New => {
                if let Err(e) = manager.new_surface_in_active(None) {
                    tracing::warn!(error = %e, "new_surface_in_active failed");
                }
            }
        }
    }
}

fn render_node(
    ui: &mut egui::Ui,
    node: &mut PaneNode,
    rect: Rect,
    active_pane: &mut PaneId,
    actions: &mut Vec<TabAction>,
) {
    let resize_request: Option<(SplitId, usize, f32)> = match node {
        PaneNode::Leaf { id, .. } => {
            let is_active = *id == *active_pane;
            render_leaf(ui, node, rect, is_active, active_pane, actions);
            None
        }
        PaneNode::Browser { id, browser } => {
            let is_active = *id == *active_pane;
            render_browser(ui, browser.as_mut(), rect, is_active, active_pane);
            None
        }
        PaneNode::Split { id, direction, children, sizes } => {
            let split_id = *id;
            render_split(ui, direction, children, sizes, rect, active_pane, actions)
                .map(|(child_idx, delta)| (split_id, child_idx, delta))
        }
    };
    if let Some((split_id, child_idx, delta)) = resize_request {
        let _ = node.resize_split(split_id, child_idx, delta);
    }
}

fn render_leaf(
    ui: &mut egui::Ui,
    leaf: &mut PaneNode,
    rect: Rect,
    is_active: bool,
    active_pane: &mut PaneId,
    actions: &mut Vec<TabAction>,
) {
    let PaneNode::Leaf { id, .. } = leaf else {
        return;
    };
    let pane_id = *id;
    let surface_count = leaf.leaf_surfaces().len();
    let show_tab_bar = should_render_tab_bar(surface_count);
    let tab_bar_height = if show_tab_bar { TAB_BAR_HEIGHT } else { 0.0_f32 };

    if show_tab_bar {
        let tab_bar_rect =
            Rect::from_min_size(rect.min, Vec2::new(rect.width(), TAB_BAR_HEIGHT));
        render_tab_bar(ui, leaf, tab_bar_rect, is_active, actions);
    }

    let terminal_rect = Rect::from_min_size(
        rect.min + Vec2::new(0.0_f32, tab_bar_height),
        Vec2::new(rect.width(), (rect.height() - tab_bar_height).max(0.0_f32)),
    );

    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(terminal_rect)
            .layout(egui::Layout::default()),
    );

    if let Some(pane) = leaf.active_terminal_mut() {
        pane.show(&mut child_ui);
        if pane.has_focus() && !is_active {
            *active_pane = pane_id;
        }
    } else {
        let painter = child_ui.painter();
        let palette = theme::palette();
        painter.rect_filled(terminal_rect, 0.0_f32, palette.panel_bg);
        painter.rect_stroke(
            terminal_rect.shrink(0.5_f32),
            egui::CornerRadius::ZERO,
            egui::Stroke::new(1.0_f32, palette.border),
            egui::StrokeKind::Inside,
        );
        painter.text(
            terminal_rect.center(),
            egui::Align2::CENTER_CENTER,
            "Spawning terminal\u{2026}",
            egui::FontId::monospace(12.0_f32),
            palette.text_muted,
        );
    }
}

fn render_tab_bar(
    ui: &mut egui::Ui,
    leaf: &mut PaneNode,
    rect: Rect,
    is_active: bool,
    actions: &mut Vec<TabAction>,
) {
    let palette = theme::palette();
    ui.painter().rect_filled(rect, 0.0_f32, palette.panel_bg);
    ui.painter().rect_stroke(
        Rect::from_min_size(
            rect.min + Vec2::new(0.0_f32, rect.height() - 1.0_f32),
            Vec2::new(rect.width(), 1.0_f32),
        ),
        egui::CornerRadius::ZERO,
        egui::Stroke::new(1.0_f32, palette.chrome_border),
        egui::StrokeKind::Inside,
    );

    let active_idx = leaf.active_surface_index();
    let surface_count = leaf.leaf_surfaces().len();
    let titles: Vec<String> = leaf
        .leaf_surfaces()
        .iter()
        .map(|s| s.display_title().to_string())
        .collect();

    let mut tab_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink(2.0_f32).shrink2(Vec2::new(2.0_f32, 0.0_f32)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );

    tab_ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0_f32;
        for (idx, title) in titles.iter().enumerate() {
            let is_current = idx == active_idx;
            let (fill, stroke) = if is_current {
                (palette.tab_active_bg, egui::Stroke::new(1.0_f32, palette.accent))
            } else {
                (palette.panel_active_bg, egui::Stroke::new(1.0_f32, palette.chrome_border))
            };
            let button = egui::Button::new(egui::RichText::new(title).size(11.0_f32))
                .min_size(egui::Vec2::new(0.0_f32, TAB_BAR_HEIGHT - 4.0_f32))
                .fill(fill)
                .stroke(stroke);
            if ui.add(button).clicked() {
                actions.push(TabAction::Select(idx));
            }
            if is_current && is_active && surface_count > 1 {
                let close_btn = egui::Button::new(egui::RichText::new("\u{2715}").size(10.0_f32))
                    .min_size(egui::Vec2::new(18.0_f32, TAB_BAR_HEIGHT - 4.0_f32))
                    .fill(palette.danger.gamma_multiply(0.4_f32));
                if ui.add(close_btn).clicked() {
                    actions.push(TabAction::Close(idx));
                }
            }
        }
        let plus_btn = egui::Button::new(egui::RichText::new("+").size(13.0_f32))
            .min_size(egui::Vec2::new(22.0_f32, TAB_BAR_HEIGHT - 4.0_f32))
            .fill(palette.panel_active_bg);
        if ui.add(plus_btn).clicked() {
            actions.push(TabAction::New);
        }
    });
}

fn render_browser(
    ui: &mut egui::Ui,
    browser: &mut crate::browser::BrowserPane,
    rect: Rect,
    is_active: bool,
    active_pane: &mut PaneId,
) {
    let pane_id = *active_pane;
    crate::browser::webview::render_browser_pane(
        ui,
        pane_id,
        browser,
        rect,
        is_active,
        active_pane,
    );
}

fn render_split(
    ui: &mut egui::Ui,
    direction: &SplitDirection,
    children: &mut [PaneNode],
    sizes: &[f32],
    rect: Rect,
    active_pane: &mut PaneId,
    actions: &mut Vec<TabAction>,
) -> Option<(usize, f32)> {
    let is_horizontal = direction.is_horizontal();
    let available_dimension = if is_horizontal { rect.width() } else { rect.height() };
    let num_children = children.len();
    let total_borders = SPLIT_BORDER * (num_children.saturating_sub(1)) as f32;
    let usable_space = available_dimension - total_borders;

    let palette = theme::palette();
    let mut offset = 0.0_f32;
    let mut resize_request: Option<(usize, f32)> = None;

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

        render_node(ui, child, child_rect, active_pane, actions);

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

/// Decide whether the tab bar UI should be rendered above a leaf pane.
///
/// `pub(crate)` (not `pub`) so the test submodule can reach it without
/// leaking it into the crate's public surface.
pub(crate) fn should_render_tab_bar(surface_count: usize) -> bool {
    surface_count > 1
}

#[cfg(test)]
mod tests {
    use super::should_render_tab_bar;

    #[test]
    fn test_should_render_tab_bar_with_multiple_surfaces() {
        assert!(should_render_tab_bar(2));
        assert!(should_render_tab_bar(3));
        assert!(should_render_tab_bar(10));
        assert!(should_render_tab_bar(100));
    }

    #[test]
    fn test_should_render_tab_bar_hidden_for_single_or_zero_surfaces() {
        assert!(!should_render_tab_bar(0));
        assert!(!should_render_tab_bar(1));
    }
}
