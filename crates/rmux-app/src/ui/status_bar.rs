//! Bottom status bar: workspace context on the left, counts on the right.
//!
//! 26px `chrome_bg` strip with a 1px `chrome_border` hairline along its
//! top edge (see `docs/UI_REDESIGN.md` §D). Left side mixes an 11px
//! proportional workspace name with 10px mono status indicators
//! (`{active}/{total}` index and `N panes` count). Right side stays
//! 11px proportional — only the unread count may take `accent`; the
//! `ZOOM` badge, when a pane is zoomed, uses `warning`.

use egui::{Align2, FontFamily, FontId, Stroke, pos2};

use crate::notifications::NotificationManager;
use crate::ui::theme::{self, metrics};
use crate::workspace::WorkspaceManager;

/// Horizontal padding from the panel edges to the first/last segment.
const EDGE_PAD: f32 = 8.0_f32;
/// Gap between the leading dot and the workspace name (no separator).
const DOT_GAP: f32 = 4.0_f32;
/// Gap between left-side segments joined with `" • "`.
const SEG_GAP: f32 = 8.0_f32;

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
            let mono_font = FontId::new(10.0_f32, FontFamily::Monospace);

            // Top hairline
            painter.hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0_f32, p.chrome_border));

            // Left: ● {name} • {active}/{total} • N panes
            // Dot is `success`; name is 11px proportional `text_muted`;
            // index and pane count are 10px mono `text_muted`.
            let ws = manager.active();
            let dot = painter.text(
                pos2(rect.left() + EDGE_PAD, rect.center().y),
                Align2::LEFT_CENTER,
                "●",
                FontId::proportional(9.0_f32),
                p.success,
            );
            let name_seg = painter.text(
                pos2(dot.right() + DOT_GAP, rect.center().y),
                Align2::LEFT_CENTER,
                ws.name.clone(),
                font.clone(),
                p.text_muted,
            );
            let index_seg = painter.text(
                pos2(name_seg.right() + SEG_GAP, rect.center().y),
                Align2::LEFT_CENTER,
                format!("{}/{}", manager.active_index() + 1, manager.workspace_count()),
                mono_font.clone(),
                p.text_muted,
            );
            let panes = ws.pane_count();
            painter.text(
                pos2(index_seg.right() + SEG_GAP, rect.center().y),
                Align2::LEFT_CENTER,
                format!("{} pane{}", panes, if panes == 1 { "" } else { "s" }),
                mono_font,
                p.text_muted,
            );

            // Right: [ZOOM • ] M workspaces • K unread • ready
            // Painted right-to-left; separators stay muted so only the
            // unread count takes `accent`. `ZOOM` (leftmost in the right
            // group) uses `warning` and only appears when a pane is zoomed.
            let unread = notifications.unread_count();
            let unread_color = if unread > 0 { p.accent } else { p.text_muted };
            let ready = painter.text(
                pos2(rect.right() - EDGE_PAD, rect.center().y),
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
            let ws_count_seg = painter.text(
                pos2(sep1.left(), rect.center().y),
                Align2::RIGHT_CENTER,
                format!("{} workspaces", manager.workspace_count()),
                font.clone(),
                p.text_muted,
            );
            if ws.zoomed_pane.is_some() {
                let sep0 = painter.text(
                    pos2(ws_count_seg.left(), rect.center().y),
                    Align2::RIGHT_CENTER,
                    " • ",
                    font.clone(),
                    p.text_muted,
                );
                painter.text(
                    pos2(sep0.left(), rect.center().y),
                    Align2::RIGHT_CENTER,
                    "ZOOM",
                    font,
                    p.warning,
                );
            }
        });
}
