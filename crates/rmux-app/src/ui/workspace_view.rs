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
/// Height of the multi-surface tab strip (cmux-style flat bar).
const TAB_BAR_HEIGHT: f32 = 28.0;
/// Max width of a single tab label before ellipsis.
const TAB_MAX_WIDTH: f32 = 180.0;
/// Minimum width a tab may shrink to when many tabs share the strip.
const TAB_MIN_WIDTH: f32 = 56.0;
/// Hit size for the per-tab close (×) control.
const TAB_CLOSE_SIZE: f32 = 16.0;
/// Width reserved for the trailing "+" new-tab control.
const TAB_PLUS_WIDTH: f32 = 28.0;
/// Left pad before the first tab.
const TAB_BAR_PAD: f32 = 4.0;

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

/// Optional workspace wallpaper painted once behind all panes.
///
/// When `Some`, the image is drawn first (cover-fit) so every terminal
/// shares a continuous background — including agent TUIs.
pub struct WorkspaceBackground<'a> {
    /// Shared wallpaper texture manager.
    pub wallpaper: &'a crate::ui::Wallpaper,
    /// When true, skip the solid `app_bg` fill so the image is visible.
    pub active: bool,
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
///
/// Returns whether the user requested a new terminal tab (`+` / deferred
/// `TabAction::New`) so the app can spawn it with the current theme.
#[must_use]
pub fn render_pane_tree(
    ui: &mut egui::Ui,
    manager: &mut WorkspaceManager,
    zoomed_pane: Option<PaneId>,
    background: Option<WorkspaceBackground<'_>>,
) -> bool {
    let available = ui.available_rect_before_wrap();

    if !ui.is_rect_visible(available) {
        return false;
    }

    let palette = theme::palette();
    let wallpaper_active = background.as_ref().is_some_and(|b| b.active && b.wallpaper.is_ready());
    if wallpaper_active {
        if let Some(bg) = background.as_ref() {
            // Screen-aligned so the image matches the glass sidebar (no
            // free-floating layer that can cover top/status chrome).
            bg.wallpaper.paint_screen_aligned(ui.painter(), available, ui.ctx().screen_rect());
        }
    } else {
        ui.painter().rect_filled(available, 0.0_f32, palette.app_bg);
    }

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
    // Workspace` borrow has ended. `New` is returned to the app so it
    // can spawn with the current theme/font (not bare defaults).
    let mut new_requested = false;
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
                new_requested = true;
            }
        }
    }
    new_requested
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
        let tab_bar_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TAB_BAR_HEIGHT));
        render_tab_bar(ui, leaf, tab_bar_rect, is_active, actions);
    }

    let terminal_rect = Rect::from_min_size(
        rect.min + Vec2::new(0.0_f32, tab_bar_height),
        Vec2::new(rect.width(), (rect.height() - tab_bar_height).max(0.0_f32)),
    );

    // Clip child paint/interaction to the terminal area so tab-bar and
    // neighbouring splits never receive stray glyphs (GitHub #31).
    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(terminal_rect)
            .layout(egui::Layout::default())
            .id_salt(("leaf_term", pane_id.0)),
    );
    child_ui.set_clip_rect(terminal_rect);

    if let Some(pane) = leaf.active_terminal_mut() {
        // Sync keyboard-driven focus (FocusLeft/Right/Up/Down) into the pane
        // before rendering so keystrokes follow `active_pane` immediately.
        // Click detection inside `show()` runs after this and can still flip
        // focus to a different pane, which is promoted below.
        if pane.has_focus() != is_active {
            pane.set_focus(is_active);
        }
        pane.show(&mut child_ui);
        if pane.has_focus() && !is_active {
            *active_pane = pane_id;
        }
    } else {
        let painter = child_ui.painter();
        let palette = theme::palette();
        painter.rect_filled(terminal_rect, 0.0_f32, palette.panel_bg);
        painter.text(
            terminal_rect.center(),
            egui::Align2::CENTER_CENTER,
            "Spawning terminal\u{2026}",
            egui::FontId::monospace(12.0_f32),
            palette.text_muted,
        );
    }

    // cmux focus model: no accent border — unfocused splits are dimmed so
    // the active pane reads as the lit one.
    if !is_active {
        ui.painter().rect_filled(terminal_rect, 0.0_f32, egui::Color32::from_black_alpha(100));
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
    // Clip every tab paint/hit to the bar so tabs never bleed into the
    // terminal body or neighbouring chrome (GitHub #31).
    let painter = ui.painter().with_clip_rect(rect);

    // Near-black strip matching cmux terminal chrome (slightly darker than panel).
    let bar_bg = egui::Color32::from_rgb(
        palette.app_bg.r().saturating_sub(6),
        palette.app_bg.g().saturating_sub(6),
        palette.app_bg.b().saturating_sub(6),
    );
    painter.rect_filled(rect, 0.0_f32, bar_bg);
    // Bottom hairline separating tabs from the terminal body.
    painter.hline(
        rect.x_range(),
        rect.bottom() - 0.5_f32,
        egui::Stroke::new(1.0_f32, palette.chrome_border),
    );

    let active_idx = leaf.active_surface_index();
    let surface_count = leaf.leaf_surfaces().len();
    let titles: Vec<String> = leaf.leaf_surfaces().iter().map(|s| s.display_title()).collect();

    // Natural widths, then shrink proportionally so every tab + "+" fit.
    let natural: Vec<f32> = titles.iter().map(|t| measure_tab_width(ui, t)).collect();
    let tab_widths = fit_tab_widths(&natural, rect.width());

    let mut x = rect.left() + TAB_BAR_PAD;
    let cy = rect.center().y;

    for (idx, title) in titles.iter().enumerate() {
        let is_current = idx == active_idx;
        let tab_w = tab_widths.get(idx).copied().unwrap_or(TAB_MIN_WIDTH);
        // Stop once we would overflow the bar (defensive; fit_tab_widths
        // should already keep the sum under budget).
        if x + tab_w > rect.right() - TAB_PLUS_WIDTH + 0.5_f32 {
            break;
        }
        let tab_rect =
            Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(tab_w, rect.height()));

        let resp = ui
            .interact(tab_rect, ui.id().with(("surf_tab", idx)), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);

        // Active: slightly lifted fill + accent underline (cmux).
        // Inactive: transparent; hover lifts text.
        if is_current {
            painter.rect_filled(
                tab_rect.shrink2(Vec2::new(0.0_f32, 1.0_f32)),
                egui::CornerRadius::ZERO,
                palette.panel_active_bg,
            );
            painter.hline(
                tab_rect.x_range(),
                tab_rect.bottom() - 1.5_f32,
                egui::Stroke::new(2.0_f32, palette.accent),
            );
        } else if resp.hovered() {
            painter.rect_filled(
                tab_rect.shrink2(Vec2::new(0.0_f32, 1.0_f32)),
                egui::CornerRadius::ZERO,
                palette.panel_bg,
            );
        }

        let title_color =
            if is_current || resp.hovered() { palette.text_primary } else { palette.text_muted };

        // Title (left-padded); reserve room for close ×.
        let text_left = tab_rect.left() + 8.0_f32;
        let text_right = tab_rect.right() - TAB_CLOSE_SIZE - 4.0_f32;
        let mut job = egui::text::LayoutJob::simple_singleline(
            title.clone(),
            egui::FontId::proportional(11.5_f32),
            title_color,
        );
        job.wrap =
            egui::text::TextWrapping::truncate_at_width((text_right - text_left).max(0.0_f32));
        let galley = ui.fonts(|f| f.layout_job(job));
        painter.galley(egui::pos2(text_left, cy - galley.size().y / 2.0_f32), galley, title_color);

        // Close × — register hit target whenever multi-tab so the parent
        // tab click cannot steal the event. Paint when active or hovered.
        let mut closed_this_tab = false;
        if surface_count > 1 {
            let close_center =
                egui::pos2(tab_rect.right() - TAB_CLOSE_SIZE / 2.0_f32 - 2.0_f32, cy);
            let close_rect = Rect::from_center_size(close_center, Vec2::splat(TAB_CLOSE_SIZE));
            let close = ui
                .interact(close_rect, ui.id().with(("surf_close", idx)), egui::Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Close terminal");
            let click_in_close =
                resp.interact_pointer_pos().is_some_and(|pos| close_rect.contains(pos));
            if close.clicked() || (resp.clicked() && click_in_close) {
                actions.push(TabAction::Close(idx));
                closed_this_tab = true;
            }

            let show_close = is_current || resp.hovered() || close.hovered();
            if show_close {
                let x_color = if close.hovered() {
                    palette.danger
                } else if is_current {
                    palette.text_muted
                } else {
                    palette.text_disabled
                };
                if close.hovered() {
                    painter.circle_filled(
                        close_center,
                        TAB_CLOSE_SIZE / 2.0_f32 - 1.0_f32,
                        palette.danger.gamma_multiply(0.2_f32),
                    );
                }
                painter.text(
                    close_center,
                    egui::Align2::CENTER_CENTER,
                    "\u{00d7}",
                    egui::FontId::proportional(12.0_f32),
                    x_color,
                );
            }
        }

        if resp.clicked() && !is_current && !closed_this_tab {
            actions.push(TabAction::Select(idx));
        }

        x += tab_w;
    }

    // Subtle "+" to open another terminal tab (same as Cmd+T).
    // Pin to the right edge of the remaining bar so it never leaves the strip.
    let plus_center_x = (x + 12.0_f32).min(rect.right() - TAB_PLUS_WIDTH * 0.5_f32);
    let plus_rect = Rect::from_center_size(
        egui::pos2(plus_center_x, cy),
        Vec2::new(22.0_f32, TAB_BAR_HEIGHT - 6.0_f32),
    );
    let plus = ui
        .interact(plus_rect, ui.id().with("surf_new"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("New terminal (\u{2318}T)");
    let plus_color = if plus.hovered() { palette.text_primary } else { palette.text_muted };
    if plus.hovered() {
        painter.rect_filled(plus_rect, egui::CornerRadius::same(3), palette.panel_active_bg);
    }
    painter.text(
        plus_rect.center(),
        egui::Align2::CENTER_CENTER,
        "+",
        egui::FontId::proportional(14.0_f32),
        plus_color,
    );
    if plus.clicked() {
        actions.push(TabAction::New);
    }

    let _ = is_active; // pane-level active styling lives on the tab underline
}

/// Width of a tab chip for `title` (clamped to the natural max).
fn measure_tab_width(ui: &egui::Ui, title: &str) -> f32 {
    let galley = ui.painter().layout_no_wrap(
        title.to_owned(),
        egui::FontId::proportional(11.5_f32),
        egui::Color32::WHITE,
    );
    // left pad + text + close slot + right pad
    (8.0_f32 + galley.size().x + TAB_CLOSE_SIZE + 6.0_f32).clamp(72.0_f32, TAB_MAX_WIDTH)
}

/// Shrink tab widths so `sum(widths) + pad + plus` fits in `bar_width`.
///
/// Tabs keep their natural size when there is room; otherwise every tab is
/// scaled down proportionally, floored at [`TAB_MIN_WIDTH`]. This stops
/// Cmd+T tabs from drawing over each other (GitHub #31).
pub(crate) fn fit_tab_widths(natural: &[f32], bar_width: f32) -> Vec<f32> {
    let n = natural.len();
    if n == 0 {
        return Vec::new();
    }
    let budget = (bar_width - TAB_BAR_PAD - TAB_PLUS_WIDTH).max(TAB_MIN_WIDTH);
    let total: f32 = natural.iter().sum();
    if total <= budget {
        return natural.to_vec();
    }
    // Even floor if budget is tiny.
    let even = (budget / n as f32).max(TAB_MIN_WIDTH);
    if even * n as f32 > budget + 0.5_f32 {
        // Still overflow at min width — give each min and let the bar clip.
        return vec![TAB_MIN_WIDTH; n];
    }
    let scale = budget / total;
    let mut widths: Vec<f32> = natural.iter().map(|w| (*w * scale).max(TAB_MIN_WIDTH)).collect();
    // If flooring to min overshoots, rebalance by shaving the largest tabs.
    let mut sum: f32 = widths.iter().sum();
    if sum > budget {
        // Prefer shrinking the widest tabs first.
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|a, b| {
            widths[*b].partial_cmp(&widths[*a]).unwrap_or(std::cmp::Ordering::Equal)
        });
        for idx in order {
            if sum <= budget {
                break;
            }
            let excess = sum - budget;
            let reducible = (widths[idx] - TAB_MIN_WIDTH).max(0.0_f32);
            let cut = reducible.min(excess);
            widths[idx] -= cut;
            sum -= cut;
        }
    }
    widths
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
    use super::{TAB_MIN_WIDTH, fit_tab_widths, should_render_tab_bar};

    #[test]
    fn test_should_render_tab_bar_with_multiple_surfaces() {
        assert!(should_render_tab_bar(2));
        assert!(should_render_tab_bar(3));
        assert!(should_render_tab_bar(10));
        assert!(should_render_tab_bar(100));
    }

    #[test]
    fn test_fit_tab_widths_keeps_natural_when_room() {
        let natural = vec![100.0_f32, 120.0_f32, 80.0_f32];
        let fitted = fit_tab_widths(&natural, 800.0_f32);
        assert_eq!(fitted, natural);
    }

    #[test]
    fn test_fit_tab_widths_shrinks_to_budget() {
        let natural = vec![180.0_f32, 180.0_f32, 180.0_f32, 180.0_f32];
        // bar 400 → budget ≈ 400 - pad - plus ≈ 368
        let fitted = fit_tab_widths(&natural, 400.0_f32);
        let sum: f32 = fitted.iter().sum();
        assert!(sum <= 400.0_f32 - 4.0_f32 - 28.0_f32 + 1.0_f32, "sum={sum}");
        assert!(fitted.iter().all(|w| *w >= TAB_MIN_WIDTH - 0.01));
    }

    #[test]
    fn test_fit_tab_widths_empty() {
        assert!(fit_tab_widths(&[], 400.0_f32).is_empty());
    }

    #[test]
    fn test_should_render_tab_bar_hidden_for_single_or_zero_surfaces() {
        assert!(!should_render_tab_bar(0));
        assert!(!should_render_tab_bar(1));
    }
}
