//! Sidebar view — vertical tab list for workspace switching.
//!
//! The sidebar renders a vertical list of workspace tabs on the left side
//! of the application window. Each tab shows the workspace name and pane count.
//! The active workspace is highlighted. Clicking a tab switches to that workspace.

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
    /// tab list and handles click events for workspace switching.
    pub fn show(&mut self, ctx: &egui::Context, manager: &mut WorkspaceManager) {
        if !self.visible {
            return;
        }

        egui::SidePanel::left("rmux_sidebar")
            .min_width(180.0)
            .max_width(250.0)
            .default_width(200.0)
            .resizable(false)
            .show(ctx, |ui| {
                self.render_sidebar(ui, manager);
            });
    }

    /// Render the sidebar contents.
    fn render_sidebar(&mut self, ui: &mut egui::Ui, manager: &mut WorkspaceManager) {
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
        let workspaces: Vec<(WorkspaceId, String, usize)> = manager
            .list()
            .into_iter()
            .map(|(id, name, count)| (id, name.to_string(), count))
            .collect();
        let active_index = manager.active_index();
        let mut clicked_index: Option<usize> = None;

        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

            for (i, (id, name, pane_count)) in workspaces.iter().enumerate() {
                let is_active = i == active_index;
                let is_editing = self.editing_index == Some(i);

                let tab_response =
                    self.render_tab(ui, name, *pane_count, is_active, is_editing, i, *id, manager);

                // Detect single click for switching (only when not editing)
                if tab_response.clicked() && !is_editing {
                    clicked_index = Some(i);
                }

                // Detect double-click to start renaming
                if tab_response.double_clicked() && !is_editing {
                    self.editing_index = Some(i);
                    self.edit_buffer = name.to_string();
                }
            }
        });

        // Handle workspace switching
        if let Some(index) = clicked_index
            && self.editing_index != Some(index)
        {
            manager.switch_to(index);
        }

        // --- Bottom hint ---
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new("Ctrl+B to toggle")
                    .size(10.0)
                    .color(egui::Color32::from_rgb(100, 100, 110)),
            );
            ui.add_space(8.0);
        });
    }

    /// Render a single workspace tab.
    ///
    /// If `is_editing` is true, renders a `TextEdit` widget for inline rename.
    /// Returns the response for click/double-click detection.
    #[allow(clippy::too_many_arguments)]
    fn render_tab(
        &mut self,
        ui: &mut egui::Ui,
        name: &str,
        pane_count: usize,
        is_active: bool,
        is_editing: bool,
        index: usize,
        workspace_id: WorkspaceId,
        manager: &mut WorkspaceManager,
    ) -> egui::Response {
        let bg_color = if is_active { TAB_BG_ACTIVE } else { TAB_BG_INACTIVE };

        let desired_size = egui::Vec2::new(ui.available_width(), 42.0);
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

                // Request focus the first frame we enter edit mode
                if !edit_response.has_focus() && self.editing_index == Some(index) {
                    ui.memory_mut(|mem| mem.request_focus(edit_response.id));
                }

                // Commit on Enter or Escape, or when focus is permanently lost
                if edit_response.lost_focus() {
                    let escape_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));
                    if escape_pressed {
                        self.editing_index = None;
                    } else if !edit_response.has_focus() {
                        if !self.edit_buffer.is_empty() {
                            manager.rename_workspace(workspace_id, self.edit_buffer.clone());
                        }
                        self.editing_index = None;
                    }
                }
            } else {
                // Tab label (static text)
                let text_color = if is_active { TAB_TEXT_COLOR_ACTIVE } else { TAB_TEXT_COLOR };
                let label_text = format!("{} ({})", name, pane_count);
                let label_pos = egui::Pos2::new(rect.left() + 16.0, rect.center().y - 8.0);

                painter.text(
                    label_pos,
                    egui::Align2::LEFT_TOP,
                    label_text,
                    egui::FontId::proportional(12.5),
                    text_color,
                );

                // Pane count hint
                let hint = if pane_count == 1 { "1 pane" } else { &format!("{pane_count} panes") };
                let hint_pos = egui::Pos2::new(rect.left() + 16.0, rect.center().y + 4.0);
                painter.text(
                    hint_pos,
                    egui::Align2::LEFT_TOP,
                    hint,
                    egui::FontId::proportional(10.0),
                    egui::Color32::from_rgb(120, 120, 130),
                );
            }
        }

        response
    }
}
