//! Sidebar view — cmux-style workspace card list.
//!
//! Vertical list of workspace cards on the left. Each card shows the
//! workspace name (no pane/git/port metadata for now). Active card uses
//! the accent fill; hover reveals a close (×) button. Double-click or
//! Cmd/Ctrl+Shift+R starts an inline rename. Hold ⌘/Ctrl to see `⌘1…⌘9`
//! switch shortcuts on each card.

use crate::notifications::NotificationManager;
use crate::ui::help_menu::HelpMenu;
use crate::workspace::WorkspaceManager;
use crate::workspace::model::WorkspaceId;

/// Convenience accessor for the Arbor One Dark palette.
fn p() -> crate::ui::theme::Palette {
    crate::ui::theme::palette()
}

/// Card corner radius.
fn card_radius() -> u8 {
    crate::ui::theme::radius_sm()
}

/// Card horizontal padding.
const CARD_PAD_X: f32 = 10.0;
/// Card vertical padding.
const CARD_PAD_Y: f32 = 8.0;
/// Fixed card height (name-only layout, cmux-like density).
const CARD_HEIGHT: f32 = 36.0;
/// Close (×) hit target size.
const CLOSE_SIZE: f32 = 18.0;

/// Per-workspace data captured before rendering a card.
struct TabData {
    /// Workspace id.
    id: WorkspaceId,
    /// Display name.
    name: String,
    /// Number of unread notifications for this workspace.
    unread: usize,
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
    /// Whether the sidebar is currently visible.
    pub visible: bool,
    /// Whether the right-side notification panel is currently visible.
    pub right_sidebar_visible: bool,
    /// Index of the card currently being renamed (`None` if not renaming).
    editing_index: Option<usize>,
    /// Temporary buffer for the rename text edit.
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
    /// Create a new sidebar view (visible by default).
    pub fn new() -> Self {
        Self::default()
    }

    /// Start inline renaming a workspace card at the given index.
    pub fn start_rename(&mut self, index: usize, name: String) {
        if !self.visible {
            return;
        }
        self.editing_index = Some(index);
        self.edit_buffer = name;
        tracing::debug!(index, "Started inline workspace rename via shortcut");
    }

    /// Whether an inline rename is in progress (for tests / callers).
    #[cfg(test)]
    fn is_renaming(&self) -> bool {
        self.editing_index.is_some()
    }

    /// Toggle sidebar visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        tracing::debug!(visible = self.visible, "Sidebar toggled");
    }

    /// Toggle the right-side notification panel (cmux `Cmd+Opt+B`).
    #[allow(dead_code)]
    pub fn toggle_right(&mut self) {
        self.right_sidebar_visible = !self.right_sidebar_visible;
        tracing::debug!(right_visible = self.right_sidebar_visible, "Right sidebar toggled");
    }

    /// Whether the right-side notification panel should be shown.
    pub fn is_right_visible(&self) -> bool {
        self.right_sidebar_visible
    }

    /// Render the sidebar. Returns a close action if the user clicked ×.
    ///
    /// `help` owns the cmux-style circle-question control in the footer
    /// (bottom-left). Its screen rect is returned via `help_button_rect`
    /// so the popup can anchor above the button.
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

    /// Render header, cards, footer (help button). Returns close action if any.
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
            .map(|w| TabData {
                id: w.id,
                name: w.name.clone(),
                unread: notifications.unread_count_for_workspace(w.id.0),
            })
            .collect();
        let active_index = manager.active_index();
        let can_close = workspaces.len() > 1;
        let mut action = None;

        // bottom_up so the help control stays pinned to the lower-left corner.
        // No divider above the icon — matches cmux (quiet glyph in empty footer).
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(4.0_f32);
            // Small circle-question — left corner of the sidebar footer.
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
                            self.edit_buffer = tab.name.clone();
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

    /// Render a single workspace card. Name-only, cmux accent fill when active.
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
        let desired_size = egui::Vec2::new(ui.available_width(), CARD_HEIGHT);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

        if !ui.is_rect_visible(rect) {
            return CardResult { response, close_clicked: false };
        }

        let radius = egui::CornerRadius::same(card_radius());
        let hovered = response.hovered();
        let painter = ui.painter().with_clip_rect(rect);

        // cmux: active = solid accent blue + dark text; inactive = flat panel.
        let (fill, border_color, title_color) = if is_active {
            (p().accent, p().accent, p().accent_fg)
        } else if hovered {
            (p().panel_active_bg, p().border, p().text_primary)
        } else {
            (p().panel_bg, p().border, p().text_muted)
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
            // Inline rename: full-width field with padding, accent card look.
            let edit_rect = rect.shrink2(egui::Vec2::new(CARD_PAD_X - 2.0_f32, 6.0_f32));
            // Slightly darker inset so the caret/text read clearly on accent.
            painter.rect_filled(
                edit_rect,
                egui::CornerRadius::same(3),
                p().app_bg.gamma_multiply(0.55_f32),
            );

            let edit_response = ui.put(
                edit_rect.shrink2(egui::Vec2::new(4.0_f32, 2.0_f32)),
                egui::TextEdit::singleline(&mut self.edit_buffer)
                    .desired_width(f32::INFINITY)
                    .font(egui::FontId::proportional(13.0_f32))
                    .frame(false)
                    .text_color_opt(Some(p().text_primary))
                    .margin(egui::Margin::symmetric(4, 2)),
            );

            // Request focus the first frame we enter edit mode. Skip the
            // frame the widget just lost focus so we don't re-queue focus
            // onto a disappearing widget.
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

            // Right-side chips: close × on hover, else shortcut hint / unread.
            let mut right_reserved = 0.0_f32;

            if hovered && can_close && !show_hints {
                let close_rect = egui::Rect::from_center_size(
                    egui::Pos2::new(content.right() - CLOSE_SIZE / 2.0_f32, content.center().y),
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
                    "\u{00d7}", // ×
                    egui::FontId::proportional(14.0_f32),
                    x_color,
                );

                if close.clicked() {
                    close_clicked = true;
                }
                right_reserved = CLOSE_SIZE + 4.0_f32;
            } else if show_hints && index < 9 {
                let badge_center = egui::Pos2::new(content.right() - 12.0_f32, content.center().y);
                crate::ui::shortcut_hints::draw_workspace_badge(ui, badge_center, index, is_active);
                right_reserved = 28.0_f32;
            } else if tab.unread > 0 {
                let badge_center = egui::Pos2::new(content.right() - 8.0_f32, content.center().y);
                painter.circle_filled(
                    badge_center,
                    7.0_f32,
                    if is_active { p().accent_fg } else { p().accent },
                );
                painter.text(
                    badge_center,
                    egui::Align2::CENTER_CENTER,
                    tab.unread.to_string(),
                    egui::FontId::monospace(9.0_f32),
                    if is_active { p().accent } else { p().accent_fg },
                );
                right_reserved = 20.0_f32;
            }

            // Workspace name — single line, truncated.
            let mut job = egui::text::LayoutJob::simple_singleline(
                tab.name.clone(),
                egui::FontId::proportional(13.0_f32),
                title_color,
            );
            job.wrap = egui::text::TextWrapping::truncate_at_width(
                (content.width() - right_reserved).max(0.0_f32),
            );
            let galley = ui.fonts(|f| f.layout_job(job));
            let name_pos =
                egui::Pos2::new(content.left(), content.center().y - galley.size().y / 2.0_f32);
            painter.galley(name_pos, galley, title_color);
        }

        CardResult { response, close_clicked }
    }
}

/// Result of drawing one workspace card.
struct CardResult {
    response: egui::Response,
    close_clicked: bool,
}

/// Header: `"Workspaces"` + count pill.
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
}
