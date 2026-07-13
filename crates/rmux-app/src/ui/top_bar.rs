//! Top chrome bar: sidebar toggle, centered workspace title, notification bell.
//!
//! 34px `chrome_bg` strip with a 1px `chrome_border` hairline along its
//! bottom edge (see `docs/UI_REDESIGN.md` §D).

use egui::{CornerRadius, CursorIcon, FontId, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::notifications::NotificationManager;
use crate::ui::theme::{self, metrics};
use crate::workspace::WorkspaceManager;

/// Horizontal offset of the leftmost control (clears macOS traffic lights).
fn left_offset() -> f32 {
    if cfg!(target_os = "macos") { 76.0_f32 } else { 12.0_f32 }
}

/// Render the top bar. Call before any side panels so it spans the window.
pub fn show(
    ctx: &egui::Context,
    manager: &WorkspaceManager,
    notifications: &NotificationManager,
    sidebar_visible: &mut bool,
    notification_panel_visible: &mut bool,
    right_sidebar_visible: &mut bool,
    settings_open: &mut bool,
) {
    let p = theme::palette();
    egui::TopBottomPanel::top("rmux_top_bar")
        .exact_height(metrics::TOP_BAR_HEIGHT)
        .frame(egui::Frame::default().fill(p.chrome_bg))
        .show_separator_line(false)
        .show(ctx, |ui| {
            let rect = ui.max_rect();

            // Bottom hairline
            ui.painter().hline(
                rect.x_range(),
                rect.bottom() - 0.5_f32,
                Stroke::new(1.0_f32, p.chrome_border),
            );

            // Center: workspace name (14px strong) + optional pane-count
            // suffix (11px muted), measured and centered as one unit.
            let ws = manager.active();
            let name_galley = ui.painter().layout_no_wrap(
                ws.name.clone(),
                FontId::proportional(14.0_f32),
                p.text_primary,
            );
            let panes = ws.pane_count();
            let suffix_galley = (panes > 1).then(|| {
                ui.painter().layout_no_wrap(
                    format!(" · {panes} panes"),
                    FontId::proportional(11.0_f32),
                    p.text_muted,
                )
            });
            let total_width =
                name_galley.size().x + suffix_galley.as_ref().map_or(0.0_f32, |g| g.size().x);
            let mut cursor_x = rect.center().x - total_width / 2.0_f32;
            let name_pos = pos2(cursor_x, rect.center().y - name_galley.size().y / 2.0_f32);
            cursor_x += name_galley.size().x;
            ui.painter().galley(name_pos, name_galley, p.text_primary);
            if let Some(suffix) = suffix_galley {
                let suffix_pos = pos2(cursor_x, rect.center().y - suffix.size().y / 2.0_f32);
                ui.painter().galley(suffix_pos, suffix, p.text_muted);
            }

            // Sidebar toggle (left): 20×20, radius 2, no fill
            let toggle_rect = Rect::from_center_size(
                pos2(rect.left() + left_offset() + 10.0_f32, rect.center().y),
                vec2(20.0_f32, 20.0_f32),
            );
            let toggle = ui
                .interact(toggle_rect, ui.id().with("sidebar_toggle"), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand);
            let icon_color = if !*sidebar_visible {
                p.accent
            } else if toggle.hovered() {
                p.text_primary
            } else {
                p.text_muted
            };
            ui.painter().text(
                toggle_rect.center(),
                egui::Align2::CENTER_CENTER,
                "☰",
                FontId::proportional(12.0_f32),
                icon_color,
            );
            if toggle.clicked() {
                *sidebar_visible = !*sidebar_visible;
            }

            // Notification bell (right): h=22, px=6, sized to content.
            // The count is only shown when there are unread notifications.
            let unread = notifications.unread_count();
            let icon_galley = ui.painter().layout_no_wrap(
                "🔔".to_string(),
                FontId::proportional(11.0_f32),
                p.text_muted,
            );
            let count_galley = (unread > 0).then(|| {
                ui.painter().layout_no_wrap(
                    format!(" {unread}"),
                    FontId::proportional(11.0_f32),
                    p.accent,
                )
            });
            let content_width =
                icon_galley.size().x + count_galley.as_ref().map_or(0.0_f32, |g| g.size().x);
            let bell_width = content_width + 2.0_f32 * 6.0_f32;

            // Settings gear (20×20, mirrors the left ☰ style). Sits left of
            // the right-sidebar toggle; opens the settings panel (theme, etc).
            let settings_rect = Rect::from_center_size(
                pos2(rect.right() - 12.0_f32 - bell_width - 18.0_f32 - 26.0_f32, rect.center().y),
                vec2(20.0_f32, 20.0_f32),
            );
            let settings = ui
                .interact(settings_rect, ui.id().with("settings_gear"), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text("Settings");
            let settings_icon_color =
                if *settings_open || settings.hovered() { p.text_primary } else { p.text_muted };
            ui.painter().text(
                settings_rect.center(),
                egui::Align2::CENTER_CENTER,
                "\u{2699}",
                FontId::proportional(13.0_f32),
                settings_icon_color,
            );
            if settings.clicked() {
                *settings_open = !*settings_open;
            }

            // Right sidebar toggle (20×20, mirrors the left ☰ style). Drives
            // the cmux `Cmd+Opt+B` shortcut path; sits left of the bell.
            let right_toggle_rect = Rect::from_center_size(
                pos2(rect.right() - 12.0_f32 - bell_width - 18.0_f32, rect.center().y),
                vec2(20.0_f32, 20.0_f32),
            );
            let right_toggle = ui
                .interact(right_toggle_rect, ui.id().with("right_sidebar_toggle"), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text("Toggle right sidebar (\u{2318}\u{2325}B)");
            let right_icon_color = if !*right_sidebar_visible {
                p.accent
            } else if right_toggle.hovered() {
                p.text_primary
            } else {
                p.text_muted
            };
            ui.painter().text(
                right_toggle_rect.center(),
                egui::Align2::CENTER_CENTER,
                "\u{25a5}",
                FontId::proportional(12.0_f32),
                right_icon_color,
            );
            if right_toggle.clicked() {
                *right_sidebar_visible = !*right_sidebar_visible;
            }

            let bell_rect = Rect::from_min_size(
                pos2(rect.right() - 12.0_f32 - bell_width, rect.center().y - 11.0_f32),
                vec2(bell_width, 22.0_f32),
            );
            let bell = ui
                .interact(bell_rect, ui.id().with("notification_bell"), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text("Notifications (\u{2318}I)");
            let fill = if bell.hovered() { p.panel_bg } else { p.chrome_bg };
            ui.painter().rect_filled(bell_rect, CornerRadius::same(theme::radius_sm()), fill);
            ui.painter().rect_stroke(
                bell_rect,
                CornerRadius::same(theme::radius_sm()),
                Stroke::new(1.0_f32, p.border),
                StrokeKind::Inside,
            );
            let icon_pos = pos2(
                bell_rect.left() + 6.0_f32,
                bell_rect.center().y - icon_galley.size().y / 2.0_f32,
            );
            let count_x = icon_pos.x + icon_galley.size().x;
            ui.painter().galley(icon_pos, icon_galley, p.text_muted);
            if let Some(count) = count_galley {
                let count_pos = pos2(count_x, bell_rect.center().y - count.size().y / 2.0_f32);
                ui.painter().galley(count_pos, count, p.accent);
            }
            if bell.clicked() {
                *notification_panel_visible = !*notification_panel_visible;
            }
        });
}
