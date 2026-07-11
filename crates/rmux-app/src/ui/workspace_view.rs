//! Workspace view — renders the pane tree as a split layout.
//!
//! This module renders the recursive `PaneNode` tree into the central panel
//! of the application window. Split nodes divide the available area among their
//! children. Leaf nodes render terminal panes. When a pane is zoomed (maximized),
//! only that pane is rendered at full workspace size.

use crate::ui::theme;
use crate::workspace::splits::{PaneId, PaneNode, SplitDirection};
use egui::{Rect, RichText, Vec2};

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
    ui.painter().rect_filled(available, 0.0, palette.app_bg);

    // If a pane is zoomed, render only that pane
    if let Some(zoom_id) = zoomed_pane {
        if let Some(terminal) = root.find_terminal_mut(zoom_id) {
            render_leaf(ui, zoom_id, terminal, available, zoom_id == *active_pane, active_pane);
        } else if let Some(browser) = root.find_browser_mut(zoom_id) {
            render_browser(ui, zoom_id, browser, available, zoom_id == *active_pane, active_pane);
        }

        // Zoom indicator: chrome pill in the top-right corner
        let label_rect = Rect::from_min_size(
            available.right_top() - Vec2::new(220.0, -2.0),
            Vec2::new(216.0, 18.0),
        );
        let modifier = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };
        ui.painter().rect_filled(label_rect, egui::CornerRadius::same(6), palette.chrome_bg);
        ui.painter().rect_stroke(
            label_rect,
            egui::CornerRadius::same(6),
            egui::Stroke::new(1.0, palette.chrome_border),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            label_rect.left_center() + Vec2::new(8.0, 0.0),
            egui::Align2::LEFT_CENTER,
            format!("Zoom: {modifier}+Shift+Enter to restore"),
            egui::FontId::proportional(10.0),
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
    match node {
        PaneNode::Leaf { id, terminal } => {
            let is_active = *id == *active_pane;
            render_leaf(ui, *id, terminal.as_mut(), rect, is_active, active_pane);
        }
        PaneNode::Browser { id, browser } => {
            let is_active = *id == *active_pane;
            render_browser(ui, *id, browser.as_mut(), rect, is_active, active_pane);
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
        let palette = theme::palette();
        painter.rect_filled(rect, 0.0, palette.panel_bg);
        painter.rect_stroke(
            rect.shrink(0.5),
            egui::CornerRadius::ZERO,
            egui::Stroke::new(1.0, palette.border),
            egui::StrokeKind::Inside,
        );
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Spawning terminal…",
            egui::FontId::monospace(12.0),
            palette.text_muted,
        );
    }
}

/// Render a browser pane with navigation controls and webview area.
fn render_browser(
    ui: &mut egui::Ui,
    _pane_id: PaneId,
    browser: &mut crate::browser::BrowserPane,
    rect: Rect,
    is_active: bool,
    active_pane: &mut PaneId,
) {
    let palette = theme::palette();
    let mut child_ui =
        ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::default()));

    // Fill background
    child_ui.painter().rect_filled(rect, 0.0, palette.panel_bg);

    // Focus border
    if is_active {
        child_ui.painter().rect_stroke(
            rect.shrink(0.5),
            egui::CornerRadius::ZERO,
            egui::Stroke::new(1.5, palette.accent.gamma_multiply(0.75)),
            egui::StrokeKind::Inside,
        );
    }

    let toolbar_h = 32.0;
    let toolbar_rect = Rect::from_min_size(rect.left_top(), Vec2::new(rect.width(), toolbar_h));
    let webview_rect = Rect::from_min_size(
        rect.left_top() + Vec2::new(0.0, toolbar_h + SPLIT_BORDER),
        Vec2::new(rect.width(), rect.height() - toolbar_h - SPLIT_BORDER),
    );

    // Toolbar background
    child_ui.painter().rect_filled(toolbar_rect, 0.0, palette.chrome_bg);

    // Layout toolbar with egui widgets
    child_ui.allocate_new_ui(egui::UiBuilder::new().max_rect(toolbar_rect.shrink(4.0)), |ui| {
        ui.horizontal(|ui| {
            // Back button
            let back_enabled = browser.can_go_back();
            let back_btn = egui::Button::new(RichText::new("\u{2190}").size(14.0))
                .min_size(Vec2::new(24.0, 22.0));
            if ui.add_enabled(back_enabled, back_btn).clicked() {
                let _ = browser.go_back();
            }

            // Forward button
            let fwd_enabled = browser.can_go_forward();
            let fwd_btn = egui::Button::new(RichText::new("\u{2192}").size(14.0))
                .min_size(Vec2::new(24.0, 22.0));
            if ui.add_enabled(fwd_enabled, fwd_btn).clicked() {
                let _ = browser.go_forward();
            }

            // Reload button
            if ui
                .add(
                    egui::Button::new(RichText::new("\u{21BB}").size(14.0))
                        .min_size(Vec2::new(24.0, 22.0)),
                )
                .clicked()
            {
                let _ = browser.reload();
            }

            // URL bar
            let mut url = browser.url().to_string();
            let url_id = ui.next_auto_id();
            let url_response = ui.add_sized(
                Vec2::new(ui.available_width() - 4.0, 22.0),
                egui::TextEdit::singleline(&mut url)
                    .id(url_id)
                    .font(egui::FontId::proportional(12.0))
                    .desired_width(f32::INFINITY),
            );

            // Cmd/Ctrl+L: request focus on URL bar
            if browser.focus_url_bar {
                ui.memory_mut(|mem| mem.request_focus(url_id));
                browser.focus_url_bar = false;
            }

            if url_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let _ = browser.navigate(&url);
            }
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) && url_response.has_focus() {
                let _ = browser.navigate(&url);
            }
        });
    });

    // Webview area placeholder / real webview
    if !browser.is_open() {
        child_ui.painter().rect_filled(webview_rect, 0.0, palette.app_bg);
        child_ui.painter().rect_stroke(
            webview_rect.shrink(0.5),
            egui::CornerRadius::ZERO,
            egui::Stroke::new(1.0, palette.border),
            egui::StrokeKind::Inside,
        );
        child_ui.painter().text(
            webview_rect.center(),
            egui::Align2::CENTER_CENTER,
            "Waiting for webview...",
            egui::FontId::proportional(12.0),
            palette.text_muted,
        );
    }

    // Update browser bounds for native webview positioning
    browser.set_bounds(webview_rect);
    browser.reposition_webview();

    // Set active pane on click
    if child_ui.response().clicked() && !is_active {
        *active_pane = _pane_id;
    }
}

/// Render a split node by dividing the rect among children.
///
/// A 1px hairline in the `border` color is drawn inside each gap between
/// adjacent children (Arbor's visible split divider).
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

    let divider_color = theme::palette().border;
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

        // Draw the 1px divider hairline in the gap after this child
        if i + 1 < num_children {
            let divider_rect = if is_horizontal {
                Rect::from_min_size(
                    rect.left_top() + Vec2::new(offset + child_size, 0.0),
                    Vec2::new(SPLIT_BORDER, rect.height()),
                )
            } else {
                Rect::from_min_size(
                    rect.left_top() + Vec2::new(0.0, offset + child_size),
                    Vec2::new(rect.width(), SPLIT_BORDER),
                )
            };
            ui.painter().rect_filled(divider_rect, 0.0, divider_color);
        }

        offset += child_size + SPLIT_BORDER;
    }
}
