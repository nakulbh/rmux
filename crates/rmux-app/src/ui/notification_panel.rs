//! Notification panel — toggleable right-side list of notifications.
//!
//! Shows notifications newest-first with unread markers and relative
//! timestamps. Clicking a row jumps to the workspace/pane that raised
//! the notification and marks it read. Toggled with Cmd/Ctrl+I or
//! the bell button in the sidebar.

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

        egui::SidePanel::right("rmux_notification_panel")
            .frame(
                egui::Frame::default()
                    .fill(theme::palette().background)
                    .stroke(egui::Stroke::new(1.0, theme::palette().border))
                    .inner_margin(egui::Margin::same(10)),
            )
            .min_width(240.0)
            .max_width(340.0)
            .default_width(280.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.visuals_mut().panel_fill = theme::palette().background;
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
    ui.add_space(10.0);
    ui.heading(
        egui::RichText::new(format!("Notifications ({} unread)", notifications.unread_count()))
            .size(14.0)
            .color(palette.foreground),
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(egui::RichText::new("Mark all read").size(12.0))
                    .fill(palette.secondary)
                    .stroke(egui::Stroke::new(1.0, palette.border))
                    .corner_radius(egui::CornerRadius::same(6))
                    .small(),
            )
            .clicked()
        {
            notifications.mark_all_read();
        }
        if ui
            .add(
                egui::Button::new(egui::RichText::new("Clear").size(12.0))
                    .fill(palette.secondary)
                    .stroke(egui::Stroke::new(1.0, palette.border))
                    .corner_radius(egui::CornerRadius::same(6))
                    .small(),
            )
            .clicked()
        {
            notifications.clear();
        }
    });
    ui.add_space(6.0);
    ui.separator();

    // Row clicks are collected and applied after the loop so the list
    // is not mutated while it is being iterated.
    let mut clicked: Option<(u64, Option<u64>, Option<u64>)> = None;

    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        if notifications.list().is_empty() {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("No notifications").color(palette.muted_foreground));
            return;
        }
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

/// Render one notification row; returns its click response.
fn render_row(ui: &mut egui::Ui, notification: &Notification) -> egui::Response {
    let palette = theme::palette();
    let row_height = if notification.body.is_some() { 64.0 } else { 46.0 };
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), row_height), egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let fill = if response.hovered() { palette.accent } else { palette.card };
        ui.painter().rect_filled(rect.shrink(1.0), egui::CornerRadius::same(8), fill);
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            egui::CornerRadius::same(8),
            egui::Stroke::new(1.0, palette.border),
            egui::StrokeKind::Inside,
        );
    }

    let mut row_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(10.0, 8.0)))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    row_ui.horizontal(|ui| {
        if !notification.read {
            ui.label(egui::RichText::new("\u{25cf}").color(palette.primary).size(10.0));
        }
        ui.label(egui::RichText::new(&notification.title).size(12.5).color(palette.foreground));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(relative_time(notification.timestamp))
                    .size(10.0)
                    .color(palette.muted_foreground),
            );
        });
    });
    if let Some(body) = &notification.body {
        row_ui.label(egui::RichText::new(body).size(11.0).color(palette.muted_foreground));
    }
    ui.add_space(4.0);
    response
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
