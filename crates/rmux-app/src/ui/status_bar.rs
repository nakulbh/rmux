//! Bottom status bar: workspace context on the left, counts on the right.
//!
//! 26px `chrome_bg` strip with a 1px `chrome_border` hairline along its
//! top edge (see `docs/UI_REDESIGN.md` §D).

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
            let font = FontId::proportional(11.0);

            // Top hairline
            painter.hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0, p.chrome_border));

            // Left: ● workspace • N panes
            let ws = manager.active();
            let dot_pos = pos2(rect.left() + 8.0, rect.center().y);
            painter.text(dot_pos, Align2::LEFT_CENTER, "●", FontId::proportional(9.0), p.success);
            let panes = ws.pane_count();
            let left_text =
                format!("{} • {} pane{}", ws.name, panes, if panes == 1 { "" } else { "s" });
            painter.text(
                pos2(dot_pos.x + 14.0, rect.center().y),
                Align2::LEFT_CENTER,
                left_text,
                font.clone(),
                p.text_muted,
            );

            // Right: M workspaces • K unread • ready
            let unread = notifications.unread_count();
            let right_anchor = pos2(rect.right() - 8.0, rect.center().y);
            let ready = painter.text(
                right_anchor,
                Align2::RIGHT_CENTER,
                "ready",
                font.clone(),
                p.text_muted,
            );
            let unread_color = if unread > 0 { p.accent } else { p.text_muted };
            let unread_galley = painter.text(
                pos2(ready.left() - 4.0, rect.center().y),
                Align2::RIGHT_CENTER,
                format!("{unread} unread •"),
                font.clone(),
                unread_color,
            );
            painter.text(
                pos2(unread_galley.left() - 4.0, rect.center().y),
                Align2::RIGHT_CENTER,
                format!("{} workspaces •", manager.workspace_count()),
                font,
                p.text_muted,
            );
        });
}
