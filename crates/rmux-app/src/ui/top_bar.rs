//! Top chrome bar: sidebar toggle, centered workspace title, notification bell.
//!
//! 34px `chrome_bg` strip with a 1px `chrome_border` hairline along its
//! bottom edge (see `docs/UI_REDESIGN.md` §D).

use egui::{Align2, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::notifications::NotificationManager;
use crate::ui::theme::{self, metrics};
use crate::workspace::WorkspaceManager;

/// Horizontal offset of the leftmost control (clears macOS traffic lights).
fn left_offset() -> f32 {
    if cfg!(target_os = "macos") { 76.0 } else { 12.0 }
}

/// Render the top bar. Call before any side panels so it spans the window.
pub fn show(
    ctx: &egui::Context,
    manager: &WorkspaceManager,
    notifications: &NotificationManager,
    sidebar_visible: &mut bool,
    notification_panel_visible: &mut bool,
) {
    let p = theme::palette();
    egui::TopBottomPanel::top("rmux_top_bar")
        .exact_height(metrics::TOP_BAR_HEIGHT)
        .frame(egui::Frame::default().fill(p.chrome_bg))
        .show_separator_line(false)
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            let painter = ui.painter();

            // Bottom hairline
            painter.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, p.chrome_border));

            // Centered workspace title
            let ws = manager.active();
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                &ws.name,
                FontId::proportional(14.0),
                p.text_primary,
            );

            // Sidebar toggle (left)
            let toggle_rect = Rect::from_center_size(
                pos2(rect.left() + left_offset() + 10.0, rect.center().y),
                vec2(20.0, 20.0),
            );
            let toggle = ui.interact(toggle_rect, ui.id().with("sidebar_toggle"), Sense::click());
            let icon_color = if !*sidebar_visible {
                p.accent
            } else if toggle.hovered() {
                p.text_primary
            } else {
                p.text_muted
            };
            ui.painter().text(
                toggle_rect.center(),
                Align2::CENTER_CENTER,
                "☰",
                FontId::proportional(12.0),
                icon_color,
            );
            if toggle.clicked() {
                *sidebar_visible = !*sidebar_visible;
            }

            // Notification bell (right)
            let unread = notifications.unread_count();
            let label = if unread > 0 { format!("🔔 {unread}") } else { "🔔".to_string() };
            let bell_rect = Rect::from_min_size(
                pos2(rect.right() - 12.0 - 44.0, rect.center().y - 11.0),
                vec2(44.0, 22.0),
            );
            let bell = ui.interact(bell_rect, ui.id().with("notification_bell"), Sense::click());
            let fill = if bell.hovered() { p.panel_bg } else { p.chrome_bg };
            ui.painter().rect_filled(bell_rect, CornerRadius::same(2), fill);
            ui.painter().rect_stroke(
                bell_rect,
                CornerRadius::same(2),
                Stroke::new(1.0, p.border),
                StrokeKind::Inside,
            );
            let bell_color = if unread > 0 { p.accent } else { p.text_muted };
            ui.painter().text(
                bell_rect.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(11.0),
                bell_color,
            );
            if bell.clicked() {
                *notification_panel_visible = !*notification_panel_visible;
            }
        });
}
