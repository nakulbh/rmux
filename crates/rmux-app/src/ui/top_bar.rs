//! Top chrome bar styled after cmux: left-aligned toolbar + workspace tabs.
//!
//! Layout (left → right, after macOS traffic lights):
//! ```text
//! [sidebar] [bell ¹] [+ ▾] [←] [→]   [📁 Workspace 1]
//!                                       ─────────────  (accent underline)
//! ```
//!
//! Icons are stroke-drawn (no emoji / Nerd Font dependency) so they stay
//! crisp at any DPI and match cmux's thin SF-Symbols aesthetic.

use egui::{
    Color32, CornerRadius, CursorIcon, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, pos2,
    vec2,
};

use crate::notifications::NotificationManager;
use crate::ui::shortcut_hints;
use crate::ui::theme::{self, metrics};
use crate::workspace::WorkspaceManager;

/// Actions the top bar can request of the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopBarAction {
    /// Toggle the left workspace sidebar.
    ToggleSidebar,
    /// Toggle the right notification panel.
    ToggleNotifications,
    /// Open / close the settings panel.
    ToggleSettings,
    /// Create a new workspace (same path as Cmd/Ctrl+N).
    NewWorkspace,
    /// Switch to the previous workspace.
    PrevWorkspace,
    /// Switch to the next workspace.
    NextWorkspace,
    /// Switch to the workspace at the given index.
    SelectWorkspace(usize),
}

/// Horizontal offset of the leftmost control (clears macOS traffic lights).
fn left_offset() -> f32 {
    if cfg!(target_os = "macos") { 78.0_f32 } else { 12.0_f32 }
}

/// Near-black fill matching cmux's title bar (darker than chrome_bg).
fn bar_bg(p: &theme::Palette) -> Color32 {
    // Slightly darker than app_bg for a flat, elevated chrome strip.
    Color32::from_rgb(
        p.app_bg.r().saturating_sub(8),
        p.app_bg.g().saturating_sub(8),
        p.app_bg.b().saturating_sub(8),
    )
}

/// Toolbar icon hit-target size (cmux uses compact ~28px squares).
const ICON_SIZE: f32 = 28.0_f32;
/// Gap between toolbar icons.
const ICON_GAP: f32 = 2.0_f32;
/// Gap between the icon cluster and the first workspace tab.
const CLUSTER_TAB_GAP: f32 = 10.0_f32;

/// Render the top bar. Call before any side panels so it spans the window.
///
/// Returns the action (if any) the user requested this frame.
pub fn show(
    ctx: &egui::Context,
    manager: &WorkspaceManager,
    notifications: &NotificationManager,
    sidebar_visible: bool,
    notification_panel_visible: bool,
    settings_open: bool,
) -> Option<TopBarAction> {
    let p = theme::palette();
    let mut action = None;
    // Hold ⌘ (macOS) / Ctrl (Linux/Windows) to reveal chord badges on each
    // control — same discoverability pattern as cmux.
    let show_hints = shortcut_hints::primary_mod_held(ctx);

    egui::TopBottomPanel::top("rmux_top_bar")
        .exact_height(metrics::TOP_BAR_HEIGHT)
        .frame(egui::Frame::default().fill(bar_bg(&p)))
        .show_separator_line(false)
        .show(ctx, |ui| {
            let rect = ui.max_rect();

            // Bottom hairline
            ui.painter().hline(
                rect.x_range(),
                rect.bottom() - 0.5_f32,
                Stroke::new(1.0_f32, p.chrome_border),
            );

            let cy = rect.center().y;
            let mut x = rect.left() + left_offset();

            // --- Toolbar icon cluster (all left-aligned, cmux order) ---

            // 1. Sidebar toggle  (⌘B / Ctrl+B)
            let sidebar_center = pos2(x, cy);
            if icon_button(
                ui,
                sidebar_center,
                "sidebar_toggle",
                sidebar_visible,
                true,
                "Toggle sidebar (\u{2318}B)",
                &p,
                draw_sidebar_icon,
            ) {
                action = Some(TopBarAction::ToggleSidebar);
            }
            if show_hints {
                shortcut_hints::draw_overlay_badge(ui, sidebar_center, &shortcut_hints::chord("B"));
            }
            x += ICON_SIZE + ICON_GAP;

            // 2. Notification bell + unread badge  (⌘I / Ctrl+I)
            let bell_center = pos2(x, cy);
            let unread = notifications.unread_count();
            if icon_button(
                ui,
                bell_center,
                "notification_bell",
                notification_panel_visible,
                true,
                "Notifications (\u{2318}I)",
                &p,
                draw_bell_icon,
            ) {
                action = Some(TopBarAction::ToggleNotifications);
            }
            if unread > 0 && !show_hints {
                draw_badge(ui, pos2(x + 8.0_f32, cy - 8.0_f32), unread, &p);
            }
            if show_hints {
                shortcut_hints::draw_overlay_badge(ui, bell_center, &shortcut_hints::chord("I"));
            }
            x += ICON_SIZE + ICON_GAP;

            // 3. Plus (+ ▾) — new workspace  (⌘N / Ctrl+N)
            let plus_center = pos2(x, cy);
            if plus_button(ui, plus_center, &p) {
                action = Some(TopBarAction::NewWorkspace);
            }
            if show_hints {
                shortcut_hints::draw_overlay_badge(ui, plus_center, &shortcut_hints::chord("N"));
            }
            x += ICON_SIZE + 6.0_f32 + ICON_GAP;

            // 4. Back (previous workspace)  (⌘[ / Ctrl+[)
            let prev_center = pos2(x, cy);
            let can_nav = manager.workspace_count() > 1;
            if icon_button(
                ui,
                prev_center,
                "ws_prev",
                false,
                can_nav,
                "Previous workspace",
                &p,
                draw_chevron_left,
            ) && can_nav
            {
                action = Some(TopBarAction::PrevWorkspace);
            }
            if show_hints {
                shortcut_hints::draw_overlay_badge(ui, prev_center, &shortcut_hints::chord("["));
            }
            x += ICON_SIZE + ICON_GAP;

            // 5. Forward (next workspace)  (⌘] / Ctrl+])
            let next_center = pos2(x, cy);
            if icon_button(
                ui,
                next_center,
                "ws_next",
                false,
                can_nav,
                "Next workspace",
                &p,
                draw_chevron_right,
            ) && can_nav
            {
                action = Some(TopBarAction::NextWorkspace);
            }
            if show_hints {
                shortcut_hints::draw_overlay_badge(ui, next_center, &shortcut_hints::chord("]"));
            }
            x += ICON_SIZE + CLUSTER_TAB_GAP;

            // --- Workspace tabs (folder + name, accent underline on active) ---
            let active_idx =
                manager.workspaces().iter().position(|w| w.id == manager.active().id).unwrap_or(0);

            for (idx, ws) in manager.workspaces().iter().enumerate() {
                let is_active = idx == active_idx;
                let tab_w = workspace_tab(ui, pos2(x, cy), rect.height(), &ws.name, is_active, &p);
                let tab_rect = Rect::from_center_size(
                    pos2(x + tab_w / 2.0_f32, cy),
                    vec2(tab_w, rect.height() - 2.0_f32),
                );
                let resp = ui
                    .interact(tab_rect, ui.id().with(("ws_tab", idx)), Sense::click())
                    .on_hover_cursor(CursorIcon::PointingHand);
                if resp.clicked() && !is_active {
                    action = Some(TopBarAction::SelectWorkspace(idx));
                }
                // Workspace number chords (⌘1…) live on the sidebar cards
                // only — painting them on the tab title would cover the name
                // (cmux keeps top-bar tabs free of number badges).
                x += tab_w + 4.0_f32;
            }

            // --- Settings (far right, Lucide-style gear next to the panel) ---
            let settings_center = pos2(rect.right() - 18.0_f32, cy);
            if icon_button(
                ui,
                settings_center,
                "settings_gear",
                settings_open,
                true,
                "Settings",
                &p,
                draw_settings_icon,
            ) {
                action = Some(TopBarAction::ToggleSettings);
            }
        });

    action
}

// ─── Buttons ────────────────────────────────────────────────────────────────

/// Compact square toolbar button. `draw` paints a 14×14 icon at `center`.
#[allow(clippy::too_many_arguments)]
fn icon_button(
    ui: &mut egui::Ui,
    center: Pos2,
    id: &str,
    active: bool,
    enabled: bool,
    tip: &str,
    p: &theme::Palette,
    draw: impl FnOnce(&egui::Painter, Pos2, Color32),
) -> bool {
    let rect = Rect::from_center_size(center, vec2(ICON_SIZE, ICON_SIZE));
    let mut resp = ui.interact(rect, ui.id().with(id), Sense::click());
    if enabled {
        resp = resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(tip);
    }

    let color = if !enabled {
        p.text_disabled
    } else if active {
        p.accent
    } else if resp.hovered() {
        p.text_primary
    } else {
        p.text_muted
    };

    if enabled && resp.hovered() {
        ui.painter().rect_filled(rect.shrink(2.0_f32), CornerRadius::same(4), p.panel_active_bg);
    }

    draw(ui.painter(), center, color);
    enabled && resp.clicked()
}

/// Plus button with a small dropdown chevron (cmux "+ ▾").
fn plus_button(ui: &mut egui::Ui, center: Pos2, p: &theme::Palette) -> bool {
    // Slightly wider to fit + and chevron.
    let size = vec2(ICON_SIZE + 6.0_f32, ICON_SIZE);
    let rect = Rect::from_center_size(center + vec2(3.0_f32, 0.0_f32), size);
    let resp = ui
        .interact(rect, ui.id().with("new_plus"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text("New workspace (\u{2318}N)");

    let color = if resp.hovered() { p.text_primary } else { p.text_muted };

    if resp.hovered() {
        ui.painter().rect_filled(rect.shrink(2.0_f32), CornerRadius::same(4), p.panel_active_bg);
    }

    // Plus cross
    let painter = ui.painter();
    let s = 5.0_f32;
    let stroke = Stroke::new(1.5_f32, color);
    let px = center.x - 2.0_f32;
    painter.line_segment([pos2(px - s, center.y), pos2(px + s, center.y)], stroke);
    painter.line_segment([pos2(px, center.y - s), pos2(px, center.y + s)], stroke);

    // Small down-chevron to the right of +
    let cx = center.x + 9.0_f32;
    let cy = center.y + 0.5_f32;
    let ch = 2.5_f32;
    painter.line_segment(
        [pos2(cx - ch, cy - 1.0_f32), pos2(cx, cy + ch - 1.0_f32)],
        Stroke::new(1.2_f32, color),
    );
    painter.line_segment(
        [pos2(cx, cy + ch - 1.0_f32), pos2(cx + ch, cy - 1.0_f32)],
        Stroke::new(1.2_f32, color),
    );

    resp.clicked()
}

/// Draw a workspace tab: accent folder + name; accent underline when active.
/// Returns the tab's total width so the caller can advance the cursor.
fn workspace_tab(
    ui: &mut egui::Ui,
    left_center: Pos2,
    bar_h: f32,
    name: &str,
    active: bool,
    p: &theme::Palette,
) -> f32 {
    let text_color = if active { p.text_primary } else { p.text_muted };
    let folder_color = if active { p.accent } else { p.text_muted };

    let name_galley =
        ui.painter().layout_no_wrap(name.to_owned(), FontId::proportional(12.5_f32), text_color);

    let folder_w = 14.0_f32;
    let gap = 5.0_f32;
    let pad_x = 8.0_f32;
    let total_w = pad_x + folder_w + gap + name_galley.size().x + pad_x;

    let origin = pos2(left_center.x, left_center.y);

    // Folder icon
    let folder_center = pos2(origin.x + pad_x + folder_w / 2.0_f32, origin.y);
    draw_folder_icon(ui.painter(), folder_center, folder_color, active);

    // Name
    let name_pos =
        pos2(origin.x + pad_x + folder_w + gap, origin.y - name_galley.size().y / 2.0_f32);
    ui.painter().galley(name_pos, name_galley, text_color);

    // Accent underline under the whole tab when active (cmux style)
    if active {
        let y = left_center.y + bar_h / 2.0_f32 - 1.5_f32;
        ui.painter().hline(origin.x..=origin.x + total_w, y, Stroke::new(2.0_f32, p.accent));
    }

    total_w
}

/// Unread count badge (small filled circle with number), cmux-style.
fn draw_badge(ui: &mut egui::Ui, center: Pos2, count: usize, p: &theme::Palette) {
    let label = if count > 99 { "99+".to_owned() } else { count.to_string() };
    let galley = ui.painter().layout_no_wrap(label, FontId::proportional(9.0_f32), Color32::WHITE);
    let r = (galley.size().x / 2.0_f32 + 3.5_f32).max(7.0_f32);
    ui.painter().circle_filled(center, r, p.accent);
    ui.painter().galley(
        pos2(center.x - galley.size().x / 2.0_f32, center.y - galley.size().y / 2.0_f32),
        galley,
        Color32::WHITE,
    );
}

// ─── Icon painters (14×14 stroke icons, SF-Symbols weight) ───────────────────

fn draw_sidebar_icon(painter: &egui::Painter, c: Pos2, color: Color32) {
    // Rounded rect with a left sidebar pane.
    let w = 13.0_f32;
    let h = 11.0_f32;
    let rect = Rect::from_center_size(c, vec2(w, h));
    let stroke = Stroke::new(1.3_f32, color);
    painter.rect_stroke(rect, CornerRadius::same(2), stroke, StrokeKind::Outside);
    // Vertical divider ~1/3 from left
    let div_x = rect.left() + w * 0.35_f32;
    painter.line_segment([pos2(div_x, rect.top()), pos2(div_x, rect.bottom())], stroke);
    // Fill left pane lightly
    let left = Rect::from_min_max(rect.min, pos2(div_x, rect.bottom()));
    painter.rect_filled(
        left.shrink(1.0_f32),
        CornerRadius::same(1),
        color.gamma_multiply(0.35_f32),
    );
}

fn draw_bell_icon(painter: &egui::Painter, c: Pos2, color: Color32) {
    let stroke = Stroke::new(1.3_f32, color);
    // Bell body: inverted U / dome
    let top = c.y - 5.0_f32;
    let bottom = c.y + 3.0_f32;
    let half_w = 5.0_f32;
    // Dome arc approximated with a few line segments
    let points = [
        pos2(c.x - half_w, bottom - 1.0_f32),
        pos2(c.x - half_w, c.y - 1.0_f32),
        pos2(c.x - half_w * 0.7_f32, top + 1.5_f32),
        pos2(c.x, top),
        pos2(c.x + half_w * 0.7_f32, top + 1.5_f32),
        pos2(c.x + half_w, c.y - 1.0_f32),
        pos2(c.x + half_w, bottom - 1.0_f32),
    ];
    painter.add(Shape::line(points.to_vec(), stroke));
    // Bottom rim
    painter.line_segment(
        [
            pos2(c.x - half_w - 1.5_f32, bottom - 1.0_f32),
            pos2(c.x + half_w + 1.5_f32, bottom - 1.0_f32),
        ],
        stroke,
    );
    // Clapper
    painter.circle_filled(pos2(c.x, bottom + 1.5_f32), 1.2_f32, color);
}

fn draw_chevron_left(painter: &egui::Painter, c: Pos2, color: Color32) {
    let stroke = Stroke::new(1.5_f32, color);
    let s = 4.0_f32;
    painter.line_segment([pos2(c.x + s * 0.4_f32, c.y - s), pos2(c.x - s * 0.5_f32, c.y)], stroke);
    painter.line_segment([pos2(c.x - s * 0.5_f32, c.y), pos2(c.x + s * 0.4_f32, c.y + s)], stroke);
}

fn draw_chevron_right(painter: &egui::Painter, c: Pos2, color: Color32) {
    let stroke = Stroke::new(1.5_f32, color);
    let s = 4.0_f32;
    painter.line_segment([pos2(c.x - s * 0.4_f32, c.y - s), pos2(c.x + s * 0.5_f32, c.y)], stroke);
    painter.line_segment([pos2(c.x + s * 0.5_f32, c.y), pos2(c.x - s * 0.4_f32, c.y + s)], stroke);
}

fn draw_folder_icon(painter: &egui::Painter, c: Pos2, color: Color32, filled: bool) {
    let w = 13.0_f32;
    let h = 10.0_f32;
    let rect = Rect::from_center_size(c + vec2(0.0_f32, 0.5_f32), vec2(w, h));
    // Tab on top-left
    let tab = Rect::from_min_size(pos2(rect.left(), rect.top() - 2.5_f32), vec2(5.0_f32, 3.0_f32));
    if filled {
        painter.rect_filled(rect, CornerRadius::same(1), color);
        painter.rect_filled(tab, CornerRadius::same(1), color);
    } else {
        let stroke = Stroke::new(1.2_f32, color);
        painter.rect_stroke(rect, CornerRadius::same(1), stroke, StrokeKind::Outside);
        painter.rect_stroke(tab, CornerRadius::same(1), stroke, StrokeKind::Outside);
    }
}

/// Lucide-style settings gear (circle + toothed rim).
///
/// Mirrors the Lucide `settings` SVG (24×24 viewBox: outer gear path +
/// `circle cx="12" cy="12" r="3"`), scaled to the toolbar icon size.
fn draw_settings_icon(painter: &egui::Painter, c: Pos2, color: Color32) {
    // Icon lives in a ~14px optical box inside the 28px hit target.
    let scale = 14.0_f32 / 24.0_f32;
    let stroke = Stroke::new(1.5_f32, color);

    // Inner hole — Lucide r=3 → ~1.75 CSS-px, slightly larger for clarity.
    let hole_r = 3.0_f32 * scale + 0.4_f32;
    painter.circle_stroke(c, hole_r, stroke);

    // Gear body radius (between hole and tooth tips).
    let body_r = 7.2_f32 * scale;
    // Tooth tip radius.
    let tip_r = 10.0_f32 * scale;

    // 8 teeth, aligned like Lucide (teeth at 0°/45°/…).
    const TEETH: usize = 8;
    let mut outline: Vec<Pos2> = Vec::with_capacity(TEETH * 4);
    for i in 0..TEETH {
        let base =
            (i as f32) * (std::f32::consts::TAU / TEETH as f32) - std::f32::consts::FRAC_PI_8;
        // Each tooth: outer arc points, then valley on the body.
        let tooth_half = std::f32::consts::TAU / (TEETH as f32 * 2.0_f32) * 0.55_f32;
        let valley_half = std::f32::consts::TAU / (TEETH as f32 * 2.0_f32) * 0.45_f32;

        let a0 = base - tooth_half;
        let a1 = base + tooth_half;
        let a2 = base + tooth_half + valley_half;
        let a3 = base + std::f32::consts::TAU / TEETH as f32 - tooth_half;

        for (ang, r) in [(a0, tip_r), (a1, tip_r), (a2, body_r), (a3, body_r)] {
            outline.push(pos2(c.x + ang.cos() * r, c.y + ang.sin() * r));
        }
    }

    painter.add(Shape::closed_line(outline, stroke));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_top_bar_action_variants() {
        // Smoke: enum is usable and Copy.
        let a = TopBarAction::NewWorkspace;
        let b = a;
        assert_eq!(a, b);
        assert_eq!(TopBarAction::SelectWorkspace(2), TopBarAction::SelectWorkspace(2));
    }

    #[test]
    fn test_left_offset_is_positive() {
        assert!(left_offset() > 0.0_f32);
    }
}
