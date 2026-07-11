//! Bottom status bar: workspace context on the left, counts on the right.
//!
//! 26px `chrome_bg` strip with a 1px `chrome_border` hairline along its
//! top edge (see `docs/UI_REDESIGN.md` §D). All text 11px `text_muted`,
//! segments joined with `" • "` — only the unread count may take `accent`.

use egui::{Align2, FontId, Stroke, pos2};

use crate::notifications::NotificationManager;
use crate::ui::theme::{self, metrics};
use crate::workspace::WorkspaceManager;

/// Render the status bar. Call before any side panels so it spans the window.
pub fn show(ctx: &egui::Context, manager: &WorkspaceManager, notifications: &NotificationManager) {
    let p = theme::palette();
    egui::TopBottomPanel::bottom("rmux_status_bar")
        .exact_height(metrics::STATUS_BAR_HEIGHT)
        .frame(egui::Frame::default().fill(p.chrome_bg))
        .show_separator_line(false)
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            let painter = ui.painter();
            let font = FontId::proportional(11.0_f32);

            // Top hairline
            painter.hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0_f32, p.chrome_border));

            // Left: ● workspace • N panes
            let ws = manager.active();
            let dot = painter.text(
                pos2(rect.left() + 8.0_f32, rect.center().y),
                Align2::LEFT_CENTER,
                "●",
                FontId::proportional(9.0_f32),
                p.success,
            );
            let panes = ws.pane_count();
            painter.text(
                pos2(dot.right() + 4.0_f32, rect.center().y),
                Align2::LEFT_CENTER,
                format!("{} • {} pane{}", ws.name, panes, if panes == 1 { "" } else { "s" }),
                font.clone(),
                p.text_muted,
            );

            // Right: M workspaces • K unread • ready
            // Painted right-to-left; separators stay muted so only the
            // unread count itself takes the accent color.
            let unread = notifications.unread_count();
            let unread_color = if unread > 0 { p.accent } else { p.text_muted };
            let ready = painter.text(
                pos2(rect.right() - 8.0_f32, rect.center().y),
                Align2::RIGHT_CENTER,
                "ready",
                font.clone(),
                p.text_muted,
            );
            let sep2 = painter.text(
                pos2(ready.left(), rect.center().y),
                Align2::RIGHT_CENTER,
                " • ",
                font.clone(),
                p.text_muted,
            );
            let unread_seg = painter.text(
                pos2(sep2.left(), rect.center().y),
                Align2::RIGHT_CENTER,
                format!("{unread} unread"),
                font.clone(),
                unread_color,
            );
            let sep1 = painter.text(
                pos2(unread_seg.left(), rect.center().y),
                Align2::RIGHT_CENTER,
                " • ",
                font.clone(),
                p.text_muted,
            );
            painter.text(
                pos2(sep1.left(), rect.center().y),
                Align2::RIGHT_CENTER,
                format!("{} workspaces", manager.workspace_count()),
                font,
                p.text_muted,
            );
        });
}
