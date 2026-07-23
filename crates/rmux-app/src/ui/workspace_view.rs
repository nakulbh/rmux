//! Workspace view — renders the pane tree as a split layout.
//!
//! This module renders the recursive `PaneNode` tree into the central panel
//! of the application window. Split nodes divide the available area among their
//! children. Leaf and browser panes always show cmux-style chrome (tabs +
//! globe / close). When a pane is zoomed (maximized), only that pane is
//! rendered at full workspace size.

use crate::ui::icons;
use crate::ui::theme;
use crate::workspace::WorkspaceManager;
use crate::workspace::splits::{PaneId, PaneNode, SplitDirection, SplitId};
use egui::{Rect, Vec2, pos2};

const SPLIT_BORDER: f32 = 1.0;
/// Height of the cmux-style pane chrome strip (tabs + globe / close).
const TAB_BAR_HEIGHT: f32 = 28.0;
/// Max width of a single tab label before ellipsis.
const TAB_MAX_WIDTH: f32 = 180.0;
/// Hit size for the per-tab close (×) control.
const TAB_CLOSE_SIZE: f32 = 16.0;
/// Size of the right-side chrome icon buttons (globe, close).
const CHROME_ICON_SIZE: f32 = 22.0_f32;

/// Actions emitted by pane chrome during rendering. Buffered and applied
/// after the tree-walk releases its `&mut Workspace` borrow.
#[derive(Debug, Clone, Copy)]
enum ChromeAction {
    SelectSurface(usize),
    CloseSurface(usize),
    NewTerminal,
    /// Globe button on a terminal pane — open a browser split from this pane.
    OpenBrowser {
        from_pane: PaneId,
    },
    /// Close a browser pane (chrome ×).
    CloseBrowser {
        pane: PaneId,
    },
}

/// Result of rendering the pane tree (deferred app-level actions).
#[derive(Debug, Default)]
pub struct PaneTreeResult {
    /// User clicked "+" for a new terminal surface (Cmd+T path).
    pub new_terminal_tab: bool,
    /// User clicked the globe on a terminal pane.
    pub open_browser_from: Option<PaneId>,
    /// User closed a browser pane via chrome ×.
    pub close_browser: Option<PaneId>,
}

/// Render the pane tree into the given `egui::Ui`, optionally zoomed.
#[must_use]
pub fn render_pane_tree(
    ui: &mut egui::Ui,
    manager: &mut WorkspaceManager,
    zoomed_pane: Option<PaneId>,
) -> PaneTreeResult {
    let available = ui.available_rect_before_wrap();

    if !ui.is_rect_visible(available) {
        return PaneTreeResult::default();
    }

    let palette = theme::palette();
    ui.painter().rect_filled(available, 0.0_f32, palette.app_bg);

    let mut actions: Vec<ChromeAction> = Vec::new();

    if let Some(zoom_id) = zoomed_pane {
        let ws = manager.active_mut();
        if ws.root.is_browser_pane(zoom_id) {
            if let Some(browser) = ws.root.find_browser_mut(zoom_id) {
                let is_active = zoom_id == ws.active_pane;
                render_browser(
                    ui,
                    zoom_id,
                    browser,
                    available,
                    is_active,
                    &mut ws.active_pane,
                    &mut actions,
                );
            }
        } else if let Some(leaf) = ws.root.find_pane_mut(zoom_id) {
            let is_active = zoom_id == ws.active_pane;
            render_leaf(ui, leaf, available, is_active, &mut ws.active_pane, &mut actions);
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

    let mut result = PaneTreeResult::default();
    for action in actions {
        match action {
            ChromeAction::SelectSurface(idx) => {
                if let Err(e) = manager.select_surface_in_active(idx) {
                    tracing::warn!(error = %e, "select_surface_in_active failed");
                }
            }
            ChromeAction::CloseSurface(idx) => {
                if let Err(e) = manager.close_surface_in_active(Some(idx)) {
                    tracing::warn!(error = %e, "close_surface_in_active failed");
                }
            }
            ChromeAction::NewTerminal => {
                result.new_terminal_tab = true;
            }
            ChromeAction::OpenBrowser { from_pane } => {
                result.open_browser_from = Some(from_pane);
            }
            ChromeAction::CloseBrowser { pane } => {
                result.close_browser = Some(pane);
            }
        }
    }
    result
}

fn render_node(
    ui: &mut egui::Ui,
    node: &mut PaneNode,
    rect: Rect,
    active_pane: &mut PaneId,
    actions: &mut Vec<ChromeAction>,
) {
    let resize_request: Option<(SplitId, usize, f32)> = match node {
        PaneNode::Leaf { id, .. } => {
            let is_active = *id == *active_pane;
            render_leaf(ui, node, rect, is_active, active_pane, actions);
            None
        }
        PaneNode::Browser { id, browser } => {
            let pane_id = *id;
            let is_active = pane_id == *active_pane;
            render_browser(ui, pane_id, browser.as_mut(), rect, is_active, active_pane, actions);
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
    actions: &mut Vec<ChromeAction>,
) {
    let PaneNode::Leaf { id, .. } = leaf else {
        return;
    };
    let pane_id = *id;

    // Always show cmux-style chrome (tabs + globe) on every terminal pane.
    let chrome_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TAB_BAR_HEIGHT));
    render_terminal_chrome(ui, leaf, pane_id, chrome_rect, is_active, actions);

    let terminal_rect = Rect::from_min_size(
        rect.min + Vec2::new(0.0_f32, TAB_BAR_HEIGHT),
        Vec2::new(rect.width(), (rect.height() - TAB_BAR_HEIGHT).max(0.0_f32)),
    );

    let mut child_ui = ui
        .new_child(egui::UiBuilder::new().max_rect(terminal_rect).layout(egui::Layout::default()));

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

/// cmux-style terminal chrome: surface tabs on the left, globe + close on the right.
fn render_terminal_chrome(
    ui: &mut egui::Ui,
    leaf: &mut PaneNode,
    pane_id: PaneId,
    rect: Rect,
    is_active: bool,
    actions: &mut Vec<ChromeAction>,
) {
    let palette = theme::palette();
    let bar_bg = egui::Color32::from_rgb(
        palette.app_bg.r().saturating_sub(6),
        palette.app_bg.g().saturating_sub(6),
        palette.app_bg.b().saturating_sub(6),
    );
    ui.painter().rect_filled(rect, 0.0_f32, bar_bg);
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5_f32,
        egui::Stroke::new(1.0_f32, palette.chrome_border),
    );

    // Right-side icon cluster reserves space so tabs don't collide.
    let icons_w = CHROME_ICON_SIZE * 2.0_f32 + 10.0_f32;
    let tabs_right = rect.right() - icons_w;
    let cy = rect.center().y;

    let active_idx = leaf.active_surface_index();
    let surface_count = leaf.leaf_surfaces().len();
    let titles: Vec<String> = leaf.leaf_surfaces().iter().map(|s| s.display_title()).collect();

    let mut x = rect.left() + 4.0_f32;

    for (idx, title) in titles.iter().enumerate() {
        if x >= tabs_right - 40.0_f32 {
            break;
        }
        let is_current = idx == active_idx;
        let tab_w = measure_tab_width(ui, title).min(tabs_right - x);
        let tab_rect = Rect::from_min_size(pos2(x, rect.top()), Vec2::new(tab_w, rect.height()));

        let resp = ui
            .interact(tab_rect, ui.id().with(("surf_tab", pane_id.0, idx)), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);

        if is_current {
            ui.painter().rect_filled(
                tab_rect.shrink2(Vec2::new(0.0_f32, 1.0_f32)),
                egui::CornerRadius::ZERO,
                palette.panel_active_bg,
            );
            ui.painter().hline(
                tab_rect.x_range(),
                tab_rect.bottom() - 1.5_f32,
                egui::Stroke::new(2.0_f32, palette.accent),
            );
        } else if resp.hovered() {
            ui.painter().rect_filled(
                tab_rect.shrink2(Vec2::new(0.0_f32, 1.0_f32)),
                egui::CornerRadius::ZERO,
                palette.panel_bg,
            );
        }

        let title_color =
            if is_current || resp.hovered() { palette.text_primary } else { palette.text_muted };

        let text_left = tab_rect.left() + 8.0_f32;
        let text_right =
            tab_rect.right() - if surface_count > 1 { TAB_CLOSE_SIZE + 4.0_f32 } else { 4.0_f32 };
        let mut job = egui::text::LayoutJob::simple_singleline(
            title.clone(),
            egui::FontId::proportional(11.5_f32),
            title_color,
        );
        job.wrap =
            egui::text::TextWrapping::truncate_at_width((text_right - text_left).max(0.0_f32));
        let galley = ui.fonts(|f| f.layout_job(job));
        ui.painter().galley(pos2(text_left, cy - galley.size().y / 2.0_f32), galley, title_color);

        let mut closed_this_tab = false;
        if surface_count > 1 {
            let close_center = pos2(tab_rect.right() - TAB_CLOSE_SIZE / 2.0_f32 - 2.0_f32, cy);
            let close_rect = Rect::from_center_size(close_center, Vec2::splat(TAB_CLOSE_SIZE));
            let close = ui
                .interact(
                    close_rect,
                    ui.id().with(("surf_close", pane_id.0, idx)),
                    egui::Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Close terminal");
            let click_in_close =
                resp.interact_pointer_pos().is_some_and(|pos| close_rect.contains(pos));
            if close.clicked() || (resp.clicked() && click_in_close) {
                actions.push(ChromeAction::CloseSurface(idx));
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
                icons::draw_close_x(ui.painter(), close_center, x_color);
            }
        }

        if resp.clicked() && !is_current && !closed_this_tab {
            actions.push(ChromeAction::SelectSurface(idx));
        }

        x += tab_w;
    }

    // "+" new terminal tab
    if x + 24.0_f32 < tabs_right {
        let plus_rect = Rect::from_center_size(
            pos2(x + 12.0_f32, cy),
            Vec2::new(22.0_f32, TAB_BAR_HEIGHT - 6.0_f32),
        );
        let plus = ui
            .interact(plus_rect, ui.id().with(("surf_new", pane_id.0)), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("New terminal (\u{2318}T)");
        let plus_color = if plus.hovered() { palette.text_primary } else { palette.text_muted };
        if plus.hovered() {
            ui.painter().rect_filled(
                plus_rect,
                egui::CornerRadius::same(3),
                palette.panel_active_bg,
            );
        }
        ui.painter().text(
            plus_rect.center(),
            egui::Align2::CENTER_CENTER,
            "+",
            egui::FontId::proportional(14.0_f32),
            plus_color,
        );
        if plus.clicked() {
            actions.push(ChromeAction::NewTerminal);
        }
    }

    // Right cluster: globe (open browser) then close-pane is intentionally
    // omitted for terminals (surface × handles multi-tab; last pane stays).
    let globe_center = pos2(rect.right() - CHROME_ICON_SIZE / 2.0_f32 - 6.0_f32, cy);
    if chrome_icon_button(
        ui,
        globe_center,
        ui.id().with(("globe", pane_id.0)),
        "Open browser",
        &palette,
        icons::draw_globe,
    ) {
        actions.push(ChromeAction::OpenBrowser { from_pane: pane_id });
    }

    let _ = is_active;
}

fn chrome_icon_button(
    ui: &mut egui::Ui,
    center: egui::Pos2,
    id: egui::Id,
    tip: &str,
    palette: &theme::Palette,
    draw: impl FnOnce(&egui::Painter, egui::Pos2, egui::Color32),
) -> bool {
    let rect = Rect::from_center_size(center, Vec2::splat(CHROME_ICON_SIZE));
    let resp = ui
        .interact(rect, id, egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tip);
    let color = if resp.hovered() { palette.text_primary } else { palette.text_muted };
    if resp.hovered() {
        ui.painter().rect_filled(
            rect.shrink(1.0_f32),
            egui::CornerRadius::same(4),
            palette.panel_active_bg,
        );
    }
    draw(ui.painter(), center, color);
    resp.clicked()
}

/// Width of a tab chip for `title` (clamped).
fn measure_tab_width(ui: &egui::Ui, title: &str) -> f32 {
    let galley = ui.painter().layout_no_wrap(
        title.to_owned(),
        egui::FontId::proportional(11.5_f32),
        egui::Color32::WHITE,
    );
    // left pad + text + close slot + right pad
    (8.0_f32 + galley.size().x + TAB_CLOSE_SIZE + 6.0_f32).clamp(72.0_f32, TAB_MAX_WIDTH)
}

fn render_browser(
    ui: &mut egui::Ui,
    pane_id: PaneId,
    browser: &mut crate::browser::BrowserPane,
    rect: Rect,
    is_active: bool,
    active_pane: &mut PaneId,
    actions: &mut Vec<ChromeAction>,
) {
    // cmux-style tab chrome: globe + "New tab" / page title on the left, × on right.
    let chrome_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TAB_BAR_HEIGHT));
    render_browser_chrome(ui, pane_id, browser, chrome_rect, is_active, actions);

    let body = Rect::from_min_size(
        rect.min + Vec2::new(0.0_f32, TAB_BAR_HEIGHT),
        Vec2::new(rect.width(), (rect.height() - TAB_BAR_HEIGHT).max(0.0_f32)),
    );
    crate::browser::webview::render_browser_pane(
        ui,
        pane_id,
        browser,
        body,
        is_active,
        active_pane,
    );
}

/// Browser pane tab strip — shows a single tab with globe icon like cmux "New tab".
fn render_browser_chrome(
    ui: &mut egui::Ui,
    pane_id: PaneId,
    browser: &crate::browser::BrowserPane,
    rect: Rect,
    is_active: bool,
    actions: &mut Vec<ChromeAction>,
) {
    let palette = theme::palette();
    let bar_bg = egui::Color32::from_rgb(
        palette.app_bg.r().saturating_sub(6),
        palette.app_bg.g().saturating_sub(6),
        palette.app_bg.b().saturating_sub(6),
    );
    ui.painter().rect_filled(rect, 0.0_f32, bar_bg);
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5_f32,
        egui::Stroke::new(1.0_f32, palette.chrome_border),
    );

    let cy = rect.center().y;
    let tab_title = browser.tab_title();
    // Extra width for the leading globe glyph inside the tab chip.
    let tab_w =
        (measure_tab_width(ui, &tab_title) + 18.0_f32).min(rect.width() - 48.0_f32).max(72.0_f32);
    let tab_rect = Rect::from_min_size(
        pos2(rect.left() + 4.0_f32, rect.top()),
        Vec2::new(tab_w, rect.height()),
    );

    // Active tab fill + accent underline
    ui.painter().rect_filled(
        tab_rect.shrink2(Vec2::new(0.0_f32, 1.0_f32)),
        egui::CornerRadius::ZERO,
        if is_active { palette.panel_active_bg } else { palette.panel_bg },
    );
    if is_active {
        ui.painter().hline(
            tab_rect.x_range(),
            tab_rect.bottom() - 1.5_f32,
            egui::Stroke::new(2.0_f32, palette.accent),
        );
    }

    // Globe + title
    let globe_c = pos2(tab_rect.left() + 12.0_f32, cy);
    icons::draw_globe(ui.painter(), globe_c, palette.text_muted);
    let title_color = if is_active { palette.text_primary } else { palette.text_muted };
    let text_left = tab_rect.left() + 24.0_f32;
    let text_right = tab_rect.right() - TAB_CLOSE_SIZE - 4.0_f32;
    let mut job = egui::text::LayoutJob::simple_singleline(
        tab_title,
        egui::FontId::proportional(11.5_f32),
        title_color,
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width((text_right - text_left).max(0.0_f32));
    let galley = ui.fonts(|f| f.layout_job(job));
    ui.painter().galley(pos2(text_left, cy - galley.size().y / 2.0_f32), galley, title_color);

    // Engine badge (os-webview vs chromium) — tiny, right of tab, before close.
    let engine_label = browser.engine_kind().as_str();
    let engine_galley = ui.painter().layout_no_wrap(
        engine_label.to_owned(),
        egui::FontId::proportional(9.5_f32),
        palette.text_disabled,
    );
    let close_center = pos2(rect.right() - CHROME_ICON_SIZE / 2.0_f32 - 6.0_f32, cy);
    let engine_pos = pos2(
        close_center.x - CHROME_ICON_SIZE / 2.0_f32 - 6.0_f32 - engine_galley.size().x,
        cy - engine_galley.size().y / 2.0_f32,
    );
    ui.painter().galley(engine_pos, engine_galley, palette.text_disabled);

    // Close browser pane
    if chrome_icon_button(
        ui,
        close_center,
        ui.id().with(("browser_close", pane_id.0)),
        "Close browser",
        &palette,
        icons::draw_close_x,
    ) {
        actions.push(ChromeAction::CloseBrowser { pane: pane_id });
    }
}

fn render_split(
    ui: &mut egui::Ui,
    direction: &SplitDirection,
    children: &mut [PaneNode],
    sizes: &[f32],
    rect: Rect,
    active_pane: &mut PaneId,
    actions: &mut Vec<ChromeAction>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_tree_result_defaults_empty() {
        let r = PaneTreeResult::default();
        assert!(!r.new_terminal_tab);
        assert!(r.open_browser_from.is_none());
        assert!(r.close_browser.is_none());
    }
}
