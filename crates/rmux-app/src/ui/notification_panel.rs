//! Notification panel — toggleable right-side list of notifications.
//!
//! Arbor/cmux-styled (see `docs/UI_REDESIGN.md` §C): `sidebar_bg` panel
//! with a 1px `border` left edge, dense notification cards on `panel_bg`
//! with a 2px `accent` stripe for unread rows, and read rows de-emphasized
//! at 0.85 opacity. Shows notifications newest-first with relative
//! timestamps. Clicking a row jumps to the workspace/pane that raised
//! the notification and marks it read. Toggled with Cmd/Ctrl+I or the
//! bell button in the top bar.

use std::sync::Arc;
use std::time::SystemTime;

use crate::notifications::{Notification, NotificationManager};
use crate::ui::theme;
use crate::workspace::WorkspaceManager;
use crate::workspace::splits::PaneId;

/// The notification panel state and renderer.
#[derive(Debug, Default)]
pub struct NotificationPanel {
    /// Whether the panel is currently visible.
    pub visible: bool,
}

impl NotificationPanel {
    /// Create a new panel (hidden by default).
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle panel visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        tracing::debug!(visible = self.visible, "Notification panel toggled");
    }

    /// Render the panel inside a right `egui::SidePanel`.
    ///
    /// Must be called before the central panel is added to the context.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        notifications: &mut NotificationManager,
        manager: &mut WorkspaceManager,
    ) {
        if !self.visible {
            return;
        }

        let palette = theme::palette();
        egui::SidePanel::right("rmux_notification_panel")
            .frame(
                egui::Frame::default()
                    .fill(palette.sidebar_bg)
                    .inner_margin(egui::Margin::same(8)),
            )
            .min_width(240.0_f32)
            .max_width(340.0_f32)
            .default_width(280.0_f32)
            .resizable(true)
            // 1px `border`-colored line on the panel's left edge (the
            // separator stroke comes from `widgets.noninteractive.bg_stroke`,
            // which `Theme::apply` sets to 1px `border`).
            .show_separator_line(true)
            .show(ctx, |ui| {
                render_panel(ui, notifications, manager);
            });
    }
}

/// Render the panel contents: header, actions, and the notification list.
fn render_panel(
    ui: &mut egui::Ui,
    notifications: &mut NotificationManager,
    manager: &mut WorkspaceManager,
) {
    let palette = theme::palette();

    // Header: "Notifications" 12px strong + right-aligned unread count pill.
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Notifications")
                .size(12.0_f32)
                .strong()
                .color(palette.text_primary),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            count_pill(ui, &palette, notifications.unread_count());
        });
    });
    ui.add_space(4.0_f32);

    // Action row.
    ui.horizontal(|ui| {
        if action_button(ui, &palette, "Mark all read").clicked() {
            notifications.mark_all_read();
        }
        if action_button(ui, &palette, "Clear").clicked() {
            notifications.clear();
        }
    });
    ui.add_space(6.0_f32);
    hline(ui, palette.border);
    ui.add_space(6.0_f32);

    if notifications.list().is_empty() {
        render_empty_state(ui, &palette);
        return;
    }

    // Row clicks are collected and applied after the loop so the list
    // is not mutated while it is being iterated.
    let mut clicked: Option<(u64, Option<u64>, Option<u64>)> = None;

    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 2.0_f32;
        for notification in notifications.list().iter().rev() {
            if render_row(ui, notification).clicked() {
                clicked = Some((notification.id, notification.workspace_id, notification.pane_id));
            }
        }
    });

    if let Some((id, workspace_id, pane_id)) = clicked {
        notifications.mark_read(id);
        jump_to(manager, workspace_id, pane_id);
    }
}

/// Render one notification card; returns its click response.
///
/// Unread cards get a 2px `accent` stripe inside the left border and a
/// `text_primary` title; read cards drop the stripe, use a `text_muted`
/// title, and are painted at 0.85 opacity.
fn render_row(ui: &mut egui::Ui, notification: &Notification) -> egui::Response {
    let palette = theme::palette();
    let title_font = egui::FontId::new(12.0_f32, egui::FontFamily::Proportional);
    let time_font = egui::FontId::new(10.0_f32, egui::FontFamily::Proportional);
    let body_font = egui::FontId::new(10.5_f32, egui::FontFamily::Proportional);

    // Padding 8px h / 6px v; line 1 (title + time), 2px gap, line 2 (body).
    let title_height = ui.fonts(|f| f.row_height(&title_font));
    let body_height = ui.fonts(|f| f.row_height(&body_font));
    let row_height = 6.0_f32
        + title_height
        + if notification.body.is_some() { 2.0_f32 + body_height } else { 0.0_f32 }
        + 6.0_f32;

    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), row_height), egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }

    // Read rows: whole card de-emphasized at 0.85 opacity.
    let alpha = if notification.read { 0.85_f32 } else { 1.0_f32 };
    let fill = if response.hovered() { palette.panel_active_bg } else { palette.panel_bg };
    let title_color = if notification.read { palette.text_muted } else { palette.text_primary };

    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::same(2), fill.gamma_multiply(alpha));
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(2),
        egui::Stroke::new(1.0_f32, palette.border.gamma_multiply(alpha)),
        egui::StrokeKind::Inside,
    );
    if !notification.read {
        // 2px accent stripe hugging the card's left edge, inside the border.
        let stripe = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 1.0_f32, rect.top() + 1.0_f32),
            egui::pos2(rect.left() + 3.0_f32, rect.bottom() - 1.0_f32),
        );
        painter.rect_filled(stripe, egui::CornerRadius::ZERO, palette.accent);
    }

    let content = rect.shrink2(egui::vec2(8.0_f32, 6.0_f32));

    // Line 1: title 12px (ellipsized) + right-aligned relative time 10px.
    let time_color = palette.text_disabled.gamma_multiply(alpha);
    let time_galley =
        painter.layout_no_wrap(relative_time(notification.timestamp), time_font, time_color);
    let title_max_width = (content.width() - time_galley.size().x - 8.0_f32).max(0.0);
    let title_galley = singleline_galley(
        ui,
        &notification.title,
        title_font,
        title_color.gamma_multiply(alpha),
        title_max_width,
    );
    let time_pos = egui::pos2(
        content.right() - time_galley.size().x,
        content.top() + (title_height - time_galley.size().y) / 2.0_f32,
    );
    painter.galley(content.left_top(), title_galley, title_color.gamma_multiply(alpha));
    painter.galley(time_pos, time_galley, time_color);

    // Line 2: body 10.5px, single line ellipsized.
    if let Some(body) = &notification.body {
        let body_color = palette.text_muted.gamma_multiply(alpha);
        let body_galley = singleline_galley(ui, body, body_font, body_color, content.width());
        painter.galley(
            egui::pos2(content.left(), content.top() + title_height + 2.0_f32),
            body_galley,
            body_color,
        );
    }

    response
}

/// Centered empty state: "No notifications" + toggle hint below.
fn render_empty_state(ui: &mut egui::Ui, palette: &theme::Palette) {
    let offset = ((ui.available_height() - 40.0_f32) / 2.0_f32).max(8.0_f32);
    ui.add_space(offset);
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new("No notifications").size(12.0_f32).color(palette.text_muted));
        ui.add_space(2.0_f32);
        ui.label(egui::RichText::new("⌘I to toggle").size(10.0_f32).color(palette.text_disabled));
    });
}

/// Header action button: h=22, radius 2, `panel_bg` + 1px `border`,
/// `panel_active_bg` on hover, 11px label.
fn action_button(ui: &mut egui::Ui, palette: &theme::Palette, label: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::new(11.0_f32, egui::FontFamily::Proportional),
        palette.text_primary,
    );
    let size = egui::vec2(galley.size().x + 16.0_f32, 22.0_f32);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let fill = if response.hovered() { palette.panel_active_bg } else { palette.panel_bg };
        let painter = ui.painter();
        painter.rect_filled(rect, egui::CornerRadius::same(2), fill);
        painter.rect_stroke(
            rect,
            egui::CornerRadius::same(2),
            egui::Stroke::new(1.0_f32, palette.border),
            egui::StrokeKind::Inside,
        );
        painter.galley(rect.center() - galley.size() / 2.0_f32, galley, palette.text_primary);
    }
    response
}

/// Count pill (sidebar spec): `panel_bg` fill, 1px `border`, fully
/// rounded, h=14, min-w 14, 9px mono text.
fn count_pill(ui: &mut egui::Ui, palette: &theme::Palette, count: usize) {
    let galley = ui.painter().layout_no_wrap(
        count.to_string(),
        egui::FontId::new(9.0_f32, egui::FontFamily::Monospace),
        palette.text_muted,
    );
    let size = egui::vec2((galley.size().x + 8.0_f32).max(14.0_f32), 14.0_f32);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, egui::CornerRadius::same(7), palette.panel_bg);
        painter.rect_stroke(
            rect,
            egui::CornerRadius::same(7),
            egui::Stroke::new(1.0_f32, palette.border),
            egui::StrokeKind::Inside,
        );
        painter.galley(rect.center() - galley.size() / 2.0_f32, galley, palette.text_muted);
    }
}

/// Full-width 1px horizontal separator line.
fn hline(ui: &mut egui::Ui, color: egui::Color32) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0_f32), egui::Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, egui::Stroke::new(1.0_f32, color));
}

/// Lay out `text` on a single line, ellipsized with `…` at `max_width`.
fn singleline_galley(
    ui: &egui::Ui,
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
    max_width: f32,
) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_width);
    ui.fonts(|f| f.layout_job(job))
}

/// Jump to the pane (preferred) or workspace a notification points at.
///
/// Falls back to the workspace when the pane no longer exists; does
/// nothing for external notifications with no location.
fn jump_to(manager: &mut WorkspaceManager, workspace_id: Option<u64>, pane_id: Option<u64>) {
    if let Some(pane) = pane_id
        && manager.focus_pane_global(PaneId(pane))
    {
        return;
    }
    if let Some(ws) = workspace_id
        && let Some(index) = manager.workspaces().iter().position(|w| w.id.0 == ws)
    {
        manager.switch_to(index);
    }
}

/// Format a timestamp as a short relative string, e.g. `"2m ago"`.
fn relative_time(timestamp: SystemTime) -> String {
    let secs = timestamp.elapsed().map(|d| d.as_secs()).unwrap_or(0);
    if secs < 60 {
        "just now".to_owned()
    } else if secs < 3_600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3_600)
    } else {
        format!("{}d ago", secs / 86_400)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn test_relative_time_buckets() {
        let now = SystemTime::now();
        assert_eq!(relative_time(now), "just now");
        assert_eq!(relative_time(now - Duration::from_secs(120)), "2m ago");
        assert_eq!(relative_time(now - Duration::from_secs(7_200)), "2h ago");
        assert_eq!(relative_time(now - Duration::from_secs(172_800)), "2d ago");
    }

    #[test]
    fn test_relative_time_future_timestamp_is_just_now() {
        // A timestamp in the future must not panic or underflow.
        let future = SystemTime::now() + Duration::from_secs(60);
        assert_eq!(relative_time(future), "just now");
    }
}
