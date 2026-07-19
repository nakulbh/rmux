//! Sidebar view — cmux-style workspace card list.
//!
//! Slot order matches cmux `SidebarWorkspaceRowCellView.applyModel`:
//! 1. **Title** (semibold) + unread badge / close × / agent glyph
//! 2. **Notification subtitle** (`latestNotificationText`)
//! 3. **Progress** bar (`sidebar.set_progress`) + optional status label
//! 4. **Branch · directory** line (monospace, secondary)
//! 5. **Pull request** chip (best-effort `gh pr view`)
//! 6. **Ports** chips
//!
//! Active card uses the accent fill; hover reveals a close (×) button.
//! Double-click or Cmd/Ctrl+Shift+R starts an inline rename. Hold ⌘/Ctrl
//! to see `⌘1…⌘9` switch shortcuts on each card.

use crate::notifications::NotificationManager;
use crate::ui::help_menu::HelpMenu;
use crate::workspace::WorkspaceManager;
use crate::workspace::model::WorkspaceId;
use crate::workspace::sidebar_snapshot::WorkspaceSidebarSnapshot;

/// Convenience accessor for the Arbor One Dark palette.
fn p() -> crate::ui::theme::Palette {
    crate::ui::theme::palette()
}

/// Card corner radius.
fn card_radius() -> u8 {
    crate::ui::theme::radius_sm()
}

const CARD_PAD_X: f32 = 10.0;
const CARD_PAD_Y: f32 = 8.0;
const CLOSE_SIZE: f32 = 18.0;
/// Git branch glyph (cmux uses SF `arrow.triangle.branch`).
const BRANCH_GLYPH: &str = "\u{2387}"; // ⎇

/// Per-workspace data captured before rendering a card.
struct TabData {
    id: WorkspaceId,
    snap: WorkspaceSidebarSnapshot,
}

/// Action requested by the sidebar this frame (handled by the app).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarAction {
    /// User clicked × on a workspace card — close it.
    CloseWorkspace(WorkspaceId),
}

/// The sidebar view renders workspace cards and handles workspace switching.
#[derive(Debug)]
pub struct SidebarView {
    pub visible: bool,
    pub right_sidebar_visible: bool,
    editing_index: Option<usize>,
    edit_buffer: String,
}

impl Default for SidebarView {
    fn default() -> Self {
        Self {
            visible: true,
            right_sidebar_visible: false,
            editing_index: None,
            edit_buffer: String::new(),
        }
    }
}

impl SidebarView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_rename(&mut self, index: usize, name: String) {
        if !self.visible {
            return;
        }
        self.editing_index = Some(index);
        self.edit_buffer = name;
        tracing::debug!(index, "Started inline workspace rename via shortcut");
    }

    #[cfg(test)]
    fn is_renaming(&self) -> bool {
        self.editing_index.is_some()
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        tracing::debug!(visible = self.visible, "Sidebar toggled");
    }

    #[allow(dead_code)]
    pub fn toggle_right(&mut self) {
        self.right_sidebar_visible = !self.right_sidebar_visible;
        tracing::debug!(right_visible = self.right_sidebar_visible, "Right sidebar toggled");
    }

    pub fn is_right_visible(&self) -> bool {
        self.right_sidebar_visible
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        manager: &mut WorkspaceManager,
        notifications: &NotificationManager,
        help: &mut HelpMenu,
        help_button_rect: &mut Option<egui::Rect>,
    ) -> Option<SidebarAction> {
        if !self.visible {
            return None;
        }

        let show_hints = crate::ui::shortcut_hints::primary_mod_held(ctx);
        let mut action = None;

        egui::SidePanel::left("rmux_sidebar")
            .frame(egui::Frame::default().fill(p().sidebar_bg).inner_margin(egui::Margin::same(8)))
            .min_width(crate::ui::theme::metrics::SIDEBAR_MIN_WIDTH)
            .max_width(crate::ui::theme::metrics::SIDEBAR_MAX_WIDTH)
            .default_width(crate::ui::theme::metrics::SIDEBAR_DEFAULT_WIDTH)
            .resizable(true)
            .show(ctx, |ui| {
                action = self.render_sidebar(
                    ui,
                    manager,
                    notifications,
                    show_hints,
                    help,
                    help_button_rect,
                );
            });

        action
    }

    fn render_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        manager: &mut WorkspaceManager,
        notifications: &NotificationManager,
        show_hints: bool,
        help: &mut HelpMenu,
        help_button_rect: &mut Option<egui::Rect>,
    ) -> Option<SidebarAction> {
        let workspaces: Vec<TabData> = manager
            .workspaces()
            .iter()
            .map(|w| {
                let unread = notifications.unread_count_for_workspace(w.id.0);
                let latest = notifications.latest_unread_text_for_workspace(w.id.0);
                let term = w.root.find_pane(w.active_pane).and_then(|n| n.active_terminal());
                let snap = WorkspaceSidebarSnapshot::build(
                    w.name.clone(),
                    w.status.as_deref(),
                    w.progress,
                    w.ports(),
                    unread,
                    latest.as_deref(),
                    w.git_branch.as_deref().or_else(|| term.and_then(|t| t.cached_git_branch())),
                    term.and_then(|t| t.cached_cwd()),
                    term.and_then(|t| t.cached_fg_title()),
                    w.pull_request.clone(),
                );
                // Prefer live agent flag from workspace refresh when set.
                let mut snap = snap;
                if w.shows_agent_activity {
                    snap.shows_agent_activity = true;
                }
                TabData { id: w.id, snap }
            })
            .collect();
        let active_index = manager.active_index();
        let can_close = workspaces.len() > 1;
        let mut action = None;

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(4.0_f32);
            let help_resp = help.show_button(ui);
            *help_button_rect = Some(help_resp.rect);
            ui.add_space(4.0_f32);

            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                render_header(ui, workspaces.len());

                let mut clicked_index: Option<usize> = None;
                let mut close_id: Option<WorkspaceId> = None;

                egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::Vec2::new(0.0_f32, 4.0_f32);

                    for (i, tab) in workspaces.iter().enumerate() {
                        let is_active = i == active_index;
                        let is_editing = self.editing_index == Some(i);

                        let card = self.render_card(
                            ui, tab, is_active, is_editing, i, manager, show_hints, can_close,
                        );

                        if card.close_clicked {
                            close_id = Some(tab.id);
                        } else if card.response.clicked() && !is_editing {
                            clicked_index = Some(i);
                        }

                        if card.response.double_clicked() && !is_editing {
                            self.editing_index = Some(i);
                            self.edit_buffer = tab.snap.title.clone();
                        }
                    }
                });

                if let Some(id) = close_id {
                    action = Some(SidebarAction::CloseWorkspace(id));
                } else if let Some(index) = clicked_index
                    && self.editing_index != Some(index)
                {
                    manager.switch_to(index);
                }
            });
        });

        action
    }

    /// cmux multi-slot workspace row.
    #[allow(clippy::too_many_arguments)]
    fn render_card(
        &mut self,
        ui: &mut egui::Ui,
        tab: &TabData,
        is_active: bool,
        is_editing: bool,
        index: usize,
        manager: &mut WorkspaceManager,
        show_hints: bool,
        can_close: bool,
    ) -> CardResult {
        let snap = &tab.snap;
        let card_h = estimate_card_height(snap);
        let desired_size = egui::Vec2::new(ui.available_width(), card_h);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

        if !ui.is_rect_visible(rect) {
            return CardResult { response, close_clicked: false };
        }

        let radius = egui::CornerRadius::same(card_radius());
        let hovered = response.hovered();
        let painter = ui.painter().with_clip_rect(rect);

        let (fill, border_color, title_color, secondary) = if is_active {
            (p().accent, p().accent, p().accent_fg, p().accent_fg.gamma_multiply(0.78_f32))
        } else if hovered {
            (p().panel_active_bg, p().border, p().text_primary, p().text_muted)
        } else {
            (p().panel_bg, p().border, p().text_primary.gamma_multiply(0.92_f32), p().text_disabled)
        };

        painter.rect_filled(rect, radius, fill);
        painter.rect_stroke(
            rect,
            radius,
            egui::Stroke::new(1.0_f32, border_color),
            egui::StrokeKind::Inside,
        );

        let mut close_clicked = false;

        if is_editing {
            let edit_rect = rect.shrink2(egui::Vec2::new(CARD_PAD_X - 2.0_f32, 6.0_f32));
            painter.rect_filled(
                edit_rect,
                egui::CornerRadius::same(3),
                p().app_bg.gamma_multiply(0.55_f32),
            );

            let edit_response = ui.put(
                edit_rect.shrink2(egui::Vec2::new(4.0_f32, 2.0_f32)),
                egui::TextEdit::singleline(&mut self.edit_buffer)
                    .desired_width(f32::INFINITY)
                    .font(egui::FontId::proportional(12.5_f32))
                    .frame(false)
                    .text_color_opt(Some(p().text_primary))
                    .margin(egui::Margin::symmetric(4, 2)),
            );

            if !edit_response.has_focus()
                && !edit_response.lost_focus()
                && self.editing_index == Some(index)
            {
                ui.memory_mut(|mem| mem.request_focus(edit_response.id));
            }

            let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let escape_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));

            if escape_pressed {
                self.editing_index = None;
            } else if enter_pressed || edit_response.lost_focus() {
                if !self.edit_buffer.trim().is_empty() {
                    manager.rename_workspace(tab.id, self.edit_buffer.clone());
                }
                self.editing_index = None;
            }
        } else {
            let content = rect.shrink2(egui::Vec2::new(CARD_PAD_X, CARD_PAD_Y));
            let chip_y = content.top() + 7.0_f32;
            let mut right_reserved = 0.0_f32;

            if can_close && !show_hints {
                let close_rect = egui::Rect::from_center_size(
                    egui::Pos2::new(content.right() - CLOSE_SIZE / 2.0_f32, chip_y),
                    egui::Vec2::splat(CLOSE_SIZE),
                );
                let close = ui
                    .interact(
                        close_rect,
                        ui.id().with(("ws_close", tab.id.0)),
                        egui::Sense::click(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text("Close workspace");

                let click_in_close =
                    response.interact_pointer_pos().is_some_and(|pos| close_rect.contains(pos));
                if close.clicked() || (response.clicked() && click_in_close) {
                    close_clicked = true;
                }

                if hovered || close.hovered() {
                    let x_color = if is_active {
                        if close.hovered() {
                            p().accent_fg
                        } else {
                            p().accent_fg.gamma_multiply(0.75_f32)
                        }
                    } else if close.hovered() {
                        p().danger
                    } else {
                        p().text_muted
                    };

                    if close.hovered() {
                        let chip_fill = if is_active {
                            p().accent_fg.gamma_multiply(0.18_f32)
                        } else {
                            p().danger.gamma_multiply(0.2_f32)
                        };
                        painter.rect_filled(close_rect, egui::CornerRadius::same(3), chip_fill);
                    }

                    painter.text(
                        close_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "\u{00d7}",
                        egui::FontId::proportional(14.0_f32),
                        x_color,
                    );
                    right_reserved = CLOSE_SIZE + 4.0_f32;
                }
            } else if show_hints && index < 9 {
                let badge_center = egui::Pos2::new(content.right() - 12.0_f32, chip_y);
                crate::ui::shortcut_hints::draw_workspace_badge(ui, badge_center, index, is_active);
                right_reserved = 28.0_f32;
            } else if snap.unread_count > 0 {
                let badge_center = egui::Pos2::new(content.right() - 8.0_f32, chip_y);
                painter.circle_filled(
                    badge_center,
                    7.0_f32,
                    if is_active { p().accent_fg } else { p().accent },
                );
                painter.text(
                    badge_center,
                    egui::Align2::CENTER_CENTER,
                    snap.unread_count.to_string(),
                    egui::FontId::monospace(9.0_f32),
                    if is_active { p().accent } else { p().accent_fg },
                );
                right_reserved = 20.0_f32;
            }

            let mut left = content.left();
            if snap.shows_agent_activity {
                painter.text(
                    egui::Pos2::new(left, content.top() + 2.0_f32),
                    egui::Align2::LEFT_TOP,
                    "\u{25cc}",
                    egui::FontId::proportional(11.0_f32),
                    secondary,
                );
                left += 14.0_f32;
            }

            let text_width = (content.right() - left - right_reserved).max(0.0_f32);
            let mut y = content.top();

            // 1) Title — cmux ~12.5pt semibold
            y = paint_truncated_line(
                ui,
                &painter,
                egui::Pos2::new(left, y),
                text_width,
                &snap.title,
                egui::FontId::new(12.5_f32, egui::FontFamily::Proportional),
                title_color,
            );

            // 2) Notification subtitle
            if let Some(ref notif) = snap.latest_notification {
                y += 2.0_f32;
                y = paint_truncated_line(
                    ui,
                    &painter,
                    egui::Pos2::new(left, y),
                    text_width,
                    notif,
                    egui::FontId::proportional(10.0_f32),
                    secondary,
                );
            }

            // 3) Progress + status label
            if let Some(prog) = snap.progress {
                y += 4.0_f32;
                let bar_h = 3.0_f32;
                let bar_rect = egui::Rect::from_min_size(
                    egui::Pos2::new(left, y),
                    egui::Vec2::new(text_width, bar_h),
                );
                painter.rect_filled(
                    bar_rect,
                    egui::CornerRadius::same(1),
                    if is_active { p().accent_fg.gamma_multiply(0.2_f32) } else { p().border },
                );
                let fill_w = bar_rect.width() * prog.clamp(0.0_f32, 1.0_f32);
                if fill_w > 0.5_f32 {
                    painter.rect_filled(
                        egui::Rect::from_min_size(bar_rect.min, egui::Vec2::new(fill_w, bar_h)),
                        egui::CornerRadius::same(1),
                        if is_active { p().accent_fg } else { p().accent },
                    );
                }
                y += bar_h + 2.0_f32;
                if let Some(ref status) = snap.status {
                    y = paint_truncated_line(
                        ui,
                        &painter,
                        egui::Pos2::new(left, y),
                        text_width,
                        status,
                        egui::FontId::proportional(9.0_f32),
                        secondary.gamma_multiply(0.85_f32),
                    );
                }
            } else if let Some(ref status) = snap.status {
                y += 2.0_f32;
                y = paint_truncated_line(
                    ui,
                    &painter,
                    egui::Pos2::new(left, y),
                    text_width,
                    status,
                    egui::FontId::proportional(10.0_f32),
                    secondary,
                );
            }

            // 4) Branch · directory
            if snap.shows_branch_line()
                && let Some(ref branch_line) = snap.branch_directory_text
            {
                y += 2.0_f32;
                let line = format!("{BRANCH_GLYPH} {branch_line}");
                y = paint_truncated_line(
                    ui,
                    &painter,
                    egui::Pos2::new(left, y),
                    text_width,
                    &line,
                    egui::FontId::monospace(10.0_f32),
                    secondary,
                );
            }

            // 5) Pull request
            if let Some(ref pr) = snap.pull_request {
                y += 2.0_f32;
                let pr_color = if pr.is_open {
                    if is_active { p().accent_fg } else { p().accent }
                } else {
                    secondary
                };
                y = paint_truncated_line(
                    ui,
                    &painter,
                    egui::Pos2::new(left, y),
                    text_width,
                    &pr.label,
                    egui::FontId::proportional(10.0_f32),
                    pr_color,
                );
            }

            // 6) Ports
            if !snap.ports.is_empty() {
                y += 2.0_f32;
                let ports: String = snap
                    .ports
                    .iter()
                    .take(4)
                    .map(|port| format!(":{port}"))
                    .collect::<Vec<_>>()
                    .join("  ");
                let _ = y;
                let _ = paint_truncated_line(
                    ui,
                    &painter,
                    egui::Pos2::new(left, y),
                    text_width,
                    &ports,
                    egui::FontId::monospace(10.0_f32),
                    secondary,
                );
            }
        }

        CardResult { response, close_clicked }
    }
}

struct CardResult {
    response: egui::Response,
    close_clicked: bool,
}

fn paint_truncated_line(
    ui: &egui::Ui,
    painter: &egui::Painter,
    pos: egui::Pos2,
    max_width: f32,
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
) -> f32 {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_width);
    let galley = ui.fonts(|f| f.layout_job(job));
    let h = galley.size().y;
    painter.galley(pos, galley, color);
    pos.y + h
}

fn estimate_card_height(snap: &WorkspaceSidebarSnapshot) -> f32 {
    let mut h = CARD_PAD_Y * 2.0_f32 + 16.0_f32;
    if snap.latest_notification.is_some() {
        h += 14.0_f32;
    }
    if snap.progress.is_some() {
        h += 10.0_f32;
        if snap.status.is_some() {
            h += 12.0_f32;
        }
    } else if snap.status.is_some() {
        h += 14.0_f32;
    }
    if snap.shows_branch_line() {
        h += 14.0_f32;
    }
    if snap.pull_request.is_some() {
        h += 14.0_f32;
    }
    if !snap.ports.is_empty() {
        h += 14.0_f32;
    }
    h.max(36.0_f32)
}

fn render_header(ui: &mut egui::Ui, count: usize) {
    let (rect, _) = ui
        .allocate_exact_size(egui::Vec2::new(ui.available_width(), 28.0_f32), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();

    painter.text(
        egui::Pos2::new(rect.left(), rect.center().y),
        egui::Align2::LEFT_CENTER,
        "Workspaces",
        egui::FontId::proportional(11.0_f32),
        p().text_muted,
    );

    let galley =
        painter.layout_no_wrap(count.to_string(), egui::FontId::monospace(9.0_f32), p().text_muted);
    let pill_height = 14.0_f32;
    let pill_width = (galley.size().x + 10.0_f32).max(pill_height);
    let pill_rect = egui::Rect::from_min_size(
        egui::Pos2::new(rect.right() - pill_width, rect.center().y - pill_height / 2.0_f32),
        egui::Vec2::new(pill_width, pill_height),
    );
    let pill_radius = egui::CornerRadius::same((pill_height / 2.0_f32) as u8);
    painter.rect_filled(pill_rect, pill_radius, p().panel_bg);
    painter.rect_stroke(
        pill_rect,
        pill_radius,
        egui::Stroke::new(1.0_f32, p().border),
        egui::StrokeKind::Inside,
    );
    let text_pos = pill_rect.center() - galley.size() * 0.5_f32;
    painter.galley(text_pos, galley, p().text_muted);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_right_sidebar_default_false() {
        let sidebar = SidebarView::new();
        assert!(!sidebar.is_right_visible());
    }

    #[test]
    fn test_right_sidebar_toggle_flips_state() {
        let mut sidebar = SidebarView::new();
        assert!(!sidebar.is_right_visible());
        sidebar.toggle_right();
        assert!(sidebar.is_right_visible());
    }

    #[test]
    fn test_right_sidebar_toggle_twice_returns_to_false() {
        let mut sidebar = SidebarView::new();
        sidebar.toggle_right();
        sidebar.toggle_right();
        assert!(!sidebar.is_right_visible());
    }

    #[test]
    fn test_is_renaming_tracks_edit_state() {
        let mut sidebar = SidebarView::new();
        assert!(!sidebar.is_renaming());
        sidebar.start_rename(0, "ws".into());
        assert!(sidebar.is_renaming());
    }

    #[test]
    fn test_start_rename_noop_when_hidden() {
        let mut sidebar = SidebarView::new();
        sidebar.visible = false;
        sidebar.start_rename(0, "ws".into());
        assert!(!sidebar.is_renaming());
    }

    #[test]
    fn test_estimate_card_height_grows_with_slots() {
        let base = WorkspaceSidebarSnapshot::build(
            "title",
            None,
            None,
            &[],
            0,
            None,
            None,
            None,
            None,
            None,
        );
        let with_notif = WorkspaceSidebarSnapshot::build(
            "title",
            None,
            None,
            &[],
            1,
            Some("Claude is waiting for your input"),
            Some("main"),
            Some(std::path::Path::new("/tmp/rmux")),
            Some("cargo run"),
            None,
        );
        assert!(estimate_card_height(&with_notif) > estimate_card_height(&base));
    }
}
