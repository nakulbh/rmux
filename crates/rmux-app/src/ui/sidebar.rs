//! Sidebar view — vertical tab list for workspace switching.
//!
//! The sidebar renders a vertical list of workspace tabs on the left side
//! of the application window. Each tab shows the workspace name and pane count.
//! The active workspace is highlighted. Clicking a tab switches to that workspace.

use crate::notifications::NotificationManager;
use crate::workspace::WorkspaceManager;
use crate::workspace::model::WorkspaceId;

/// Dark background color for the sidebar.
const SIDEBAR_BG: egui::Color32 = egui::Color32::from_rgb(25, 28, 35);

/// Subtle border color for the sidebar divider.
const SIDEBAR_BORDER: egui::Color32 = egui::Color32::from_rgb(40, 44, 52);

/// Background color for tabs (inactive).
const TAB_BG_INACTIVE: egui::Color32 = egui::Color32::from_rgb(35, 38, 45);

/// Background color for the active tab.
const TAB_BG_ACTIVE: egui::Color32 = egui::Color32::from_rgb(55, 60, 75);

/// Text color for tab labels.
const TAB_TEXT_COLOR: egui::Color32 = egui::Color32::from_rgb(200, 200, 210);

/// Text color for the active tab label.
const TAB_TEXT_COLOR_ACTIVE: egui::Color32 = egui::Color32::WHITE;

/// Accent color stripe for the active tab.
const ACCENT_COLOR: egui::Color32 = egui::Color32::from_rgb(70, 130, 250);

/// Per-workspace data captured before rendering a tab.
///
/// Snapshotting avoids holding a borrow of the manager while tabs also
/// need `&mut` access for renames.
struct TabData {
    /// Workspace id.
    id: WorkspaceId,
    /// Display name.
    name: String,
    /// Number of panes.
    pane_count: usize,
    /// Status text set via `sidebar.set_status`.
    status: Option<String>,
    /// Progress in `0.0..=1.0` set via `sidebar.set_progress`.
    progress: Option<f32>,
    /// Number of unread notifications for this workspace.
    unread: usize,
}

/// The sidebar view renders workspace tabs and handles tab switching.
#[derive(Debug)]
pub struct SidebarView {
    /// Whether the sidebar is currently visible.
    pub visible: bool,
    /// Index of the tab currently being renamed (None if not renaming).
    editing_index: Option<usize>,
    /// Temporary buffer for the rename text edit.
    edit_buffer: String,
}

impl Default for SidebarView {
    fn default() -> Self {
        Self { visible: true, editing_index: None, edit_buffer: String::new() }
    }
}

impl SidebarView {
    /// Create a new sidebar view (visible by default).
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle sidebar visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        tracing::debug!(visible = self.visible, "Sidebar toggled");
    }

    /// Render the sidebar inside an `egui::SidePanel`.
    ///
    /// This should be called from the main `update` loop. It draws the vertical
    /// tab list (with per-workspace unread badges) and handles click events
    /// for workspace switching. `notification_panel_visible` is toggled by
    /// the bell button in the bottom area.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        manager: &mut WorkspaceManager,
        notifications: &NotificationManager,
        notification_panel_visible: &mut bool,
    ) {
        if !self.visible {
            return;
        }

        egui::SidePanel::left("rmux_sidebar")
            .min_width(180.0)
            .max_width(250.0)
            .default_width(200.0)
            .resizable(false)
            .show(ctx, |ui| {
                self.render_sidebar(ui, manager, notifications, notification_panel_visible);
            });
    }

    /// Render the sidebar contents.
    fn render_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        manager: &mut WorkspaceManager,
        notifications: &NotificationManager,
        notification_panel_visible: &mut bool,
    ) {
        // --- Background ---
        ui.visuals_mut().override_text_color = Some(TAB_TEXT_COLOR);
        let mut style = (*ui.ctx().style()).clone();
        style.visuals.panel_fill = SIDEBAR_BG;
        ui.ctx().set_style(style);

        // --- Header ---
        ui.add_space(12.0);
        ui.heading(egui::RichText::new("Workspaces").color(TAB_TEXT_COLOR_ACTIVE).size(13.0));
        ui.add_space(8.0);

        // --- Separator ---
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            (1.0, SIDEBAR_BORDER),
        );
        ui.add_space(8.0);

        // --- Tab list ---
        let workspaces: Vec<TabData> = manager
            .workspaces()
            .iter()
            .map(|w| TabData {
                id: w.id,
                name: w.name.clone(),
                pane_count: w.pane_count(),
                status: w.status.clone(),
                progress: w.progress,
                unread: notifications.unread_count_for_workspace(w.id.0),
            })
            .collect();
        let active_index = manager.active_index();
        let mut clicked_index: Option<usize> = None;

        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

            for (i, tab) in workspaces.iter().enumerate() {
                let is_active = i == active_index;
                let is_editing = self.editing_index == Some(i);

                let tab_response = self.render_tab(ui, tab, is_active, is_editing, i, manager);

                // Detect single click for switching (only when not editing)
                if tab_response.clicked() && !is_editing {
                    clicked_index = Some(i);
                }

                // Detect double-click to start renaming
                if tab_response.double_clicked() && !is_editing {
                    self.editing_index = Some(i);
                    self.edit_buffer = tab.name.clone();
                }
            }
        });

        // Handle workspace switching
        if let Some(index) = clicked_index
            && self.editing_index != Some(index)
        {
            manager.switch_to(index);
        }

        // --- Bottom area: hint + notification bell ---
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new("Ctrl+B to toggle")
                    .size(10.0)
                    .color(egui::Color32::from_rgb(100, 100, 110)),
            );
            ui.add_space(6.0);
            let unread = notifications.unread_count();
            let bell_label =
                if unread > 0 { format!("\u{1f514} {unread}") } else { "\u{1f514}".to_owned() };
            let bell = ui
                .small_button(egui::RichText::new(bell_label).size(11.0))
                .on_hover_text("Notifications (Cmd/Ctrl+Shift+N)");
            if bell.clicked() {
                *notification_panel_visible = !*notification_panel_visible;
            }
            ui.add_space(8.0);
        });
    }

    /// Render a single workspace tab.
    ///
    /// If `is_editing` is true, renders a `TextEdit` widget for inline rename.
    /// Returns the response for click/double-click detection.
    fn render_tab(
        &mut self,
        ui: &mut egui::Ui,
        tab: &TabData,
        is_active: bool,
        is_editing: bool,
        index: usize,
        manager: &mut WorkspaceManager,
    ) -> egui::Response {
        let bg_color = if is_active { TAB_BG_ACTIVE } else { TAB_BG_INACTIVE };

        // Taller tab when a status line is shown under the pane count hint.
        let height = if tab.status.is_some() { 54.0 } else { 42.0 };
        let desired_size = egui::Vec2::new(ui.available_width(), height);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // Background
            painter.rect_filled(rect, 4.0, bg_color);

            // Accent stripe on the left for active tab
            if is_active {
                let stripe_rect = egui::Rect::from_min_max(
                    rect.left_top(),
                    egui::Pos2::new(rect.left() + 3.0, rect.bottom()),
                );
                painter.rect_filled(stripe_rect, 0.0, ACCENT_COLOR);
            }

            if is_editing {
                // Render a TextEdit widget for inline rename
                let edit_rect = egui::Rect::from_min_max(
                    egui::Pos2::new(rect.left() + 16.0, rect.center().y - 8.0),
                    egui::Pos2::new(rect.right() - 16.0, rect.center().y + 8.0),
                );
                let edit_response = ui.put(
                    edit_rect,
                    egui::TextEdit::singleline(&mut self.edit_buffer)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Body)
                        .text_color_opt(Some(TAB_TEXT_COLOR_ACTIVE)),
                );

                // Request focus the first frame we enter edit mode. Skip this on the
                // frame the widget just lost focus (Enter/Escape/click-away) — otherwise
                // this re-queues focus for a widget that's about to disappear, leaving
                // egui's focus state stuck and blocking all keyboard shortcuts.
                if !edit_response.has_focus()
                    && !edit_response.lost_focus()
                    && self.editing_index == Some(index)
                {
                    ui.memory_mut(|mem| mem.request_focus(edit_response.id));
                }

                let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let escape_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));

                // Commit explicitly on Enter (don't rely solely on lost_focus, which
                // some egui versions don't trigger for singleline TextEdit on Enter).
                if escape_pressed {
                    self.editing_index = None;
                } else if enter_pressed || edit_response.lost_focus() {
                    if !self.edit_buffer.trim().is_empty() {
                        manager.rename_workspace(tab.id, self.edit_buffer.clone());
                    }
                    self.editing_index = None;
                }
            } else {
                // Tab label (static text)
                let text_color = if is_active { TAB_TEXT_COLOR_ACTIVE } else { TAB_TEXT_COLOR };
                let label_text = format!("{} ({})", tab.name, tab.pane_count);
                let label_pos = egui::Pos2::new(rect.left() + 16.0, rect.top() + 6.0);

                painter.text(
                    label_pos,
                    egui::Align2::LEFT_TOP,
                    label_text,
                    egui::FontId::proportional(12.5),
                    text_color,
                );

                // Pane count hint
                let pane_count = tab.pane_count;
                let hint = if pane_count == 1 { "1 pane" } else { &format!("{pane_count} panes") };
                let hint_pos = egui::Pos2::new(rect.left() + 16.0, rect.top() + 21.0);
                painter.text(
                    hint_pos,
                    egui::Align2::LEFT_TOP,
                    hint,
                    egui::FontId::proportional(10.0),
                    egui::Color32::from_rgb(120, 120, 130),
                );

                // Status text set via the sidebar API, under the pane count
                if let Some(status) = &tab.status {
                    let status_pos = egui::Pos2::new(rect.left() + 16.0, rect.top() + 34.0);
                    painter.text(
                        status_pos,
                        egui::Align2::LEFT_TOP,
                        status,
                        egui::FontId::proportional(10.0),
                        ACCENT_COLOR,
                    );
                }

                // Unread notification badge at the tab's right edge
                if tab.unread > 0 {
                    let badge_center = egui::Pos2::new(rect.right() - 16.0, rect.top() + 14.0);
                    painter.circle_filled(badge_center, 8.0, ACCENT_COLOR);
                    painter.text(
                        badge_center,
                        egui::Align2::CENTER_CENTER,
                        tab.unread.to_string(),
                        egui::FontId::proportional(9.0),
                        egui::Color32::WHITE,
                    );
                }
            }

            // Thin progress bar along the tab bottom (sidebar.set_progress).
            // Re-borrow the painter: the editing branch above needed `ui`
            // mutably for the TextEdit widget.
            if let Some(progress) = tab.progress {
                // Clamp to [0.0, 1.0], treating NaN/infinite as 0.0 so they
                // don't produce degenerate geometry and UI glitches.
                let clamped = if progress.is_finite() { progress.clamp(0.0, 1.0) } else { 0.0 };
                let width = rect.width() * clamped;
                let bar_rect = egui::Rect::from_min_max(
                    egui::Pos2::new(rect.left(), rect.bottom() - 3.0),
                    egui::Pos2::new(rect.left() + width, rect.bottom()),
                );
                ui.painter().rect_filled(bar_rect, 0.0, ACCENT_COLOR);
            }
        }

        response
    }
}
