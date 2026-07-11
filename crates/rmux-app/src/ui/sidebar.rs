//! Sidebar view — Arbor-style workspace card list.
//!
//! The sidebar renders a vertical list of workspace cards on the left side
//! of the application window (see `docs/UI_REDESIGN.md`, section A). Each
//! card shows the workspace name, a mono metadata line (pane count and
//! optional status), an unread-notification badge, and an optional progress
//! capsule. The active workspace is emphasized with an `accent` border and
//! `panel_active_bg` fill; inactive rows are de-emphasized at 0.8 opacity.
//! Clicking a card switches to that workspace; double-clicking starts an
//! inline rename.

use crate::notifications::NotificationManager;
use crate::workspace::WorkspaceManager;
use crate::workspace::model::WorkspaceId;

/// Convenience accessor for the Arbor One Dark palette.
fn p() -> crate::ui::theme::Palette {
    crate::ui::theme::palette()
}

/// Card corner radius (`radius_sm` from the design spec).
const CARD_RADIUS: u8 = 2;
/// Card horizontal padding.
const CARD_PAD_X: f32 = 8.0;
/// Card vertical padding.
const CARD_PAD_Y: f32 = 6.0;
/// Opacity applied to inactive rows (via `gamma_multiply` on colors).
const INACTIVE_OPACITY: f32 = 0.8;

/// Per-workspace data captured before rendering a card.
///
/// Snapshotting avoids holding a borrow of the manager while cards also
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

/// The sidebar view renders workspace cards and handles workspace switching.
#[derive(Debug)]
pub struct SidebarView {
    /// Whether the sidebar is currently visible.
    pub visible: bool,
    /// Index of the card currently being renamed (None if not renaming).
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

    /// Start inline renaming a workspace card at the given index.
    ///
    /// Called by the `Cmd/Ctrl+Shift+R` keyboard shortcut. The sidebar
    /// must be visible for the rename to be rendered; if it is hidden,
    /// this call is a no-op.
    pub fn start_rename(&mut self, index: usize, name: String) {
        if !self.visible {
            return;
        }
        self.editing_index = Some(index);
        self.edit_buffer = name;
        tracing::debug!(index, "Started inline workspace rename via shortcut");
    }

    /// Toggle sidebar visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        tracing::debug!(visible = self.visible, "Sidebar toggled");
    }

    /// Render the sidebar inside an `egui::SidePanel`.
    ///
    /// This should be called from the main `update` loop. It draws the
    /// workspace card list (with per-workspace unread badges) and handles
    /// click events for workspace switching. The notification bell lives in
    /// the top bar now.
    ///
    /// Returns `true` when the footer "+ New Workspace" button was clicked;
    /// the caller (`app.rs`) routes that through
    /// `RmuxApp::create_workspace_with_terminal`, the same path as Cmd/Ctrl+N.
    #[must_use]
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        manager: &mut WorkspaceManager,
        notifications: &NotificationManager,
    ) -> bool {
        if !self.visible {
            return false;
        }

        egui::SidePanel::left("rmux_sidebar")
            .frame(egui::Frame::default().fill(p().sidebar_bg).inner_margin(egui::Margin::same(8)))
            .min_width(crate::ui::theme::metrics::SIDEBAR_MIN_WIDTH)
            .max_width(crate::ui::theme::metrics::SIDEBAR_MAX_WIDTH)
            .default_width(crate::ui::theme::metrics::SIDEBAR_DEFAULT_WIDTH)
            .resizable(true)
            .show(ctx, |ui| self.render_sidebar(ui, manager, notifications))
            .inner
    }

    /// Render the sidebar contents: header, card list, and footer.
    ///
    /// Returns `true` when the "+ New Workspace" button was clicked.
    fn render_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        manager: &mut WorkspaceManager,
        notifications: &NotificationManager,
    ) -> bool {
        // Snapshot workspace data so cards can take `&mut manager` for renames.
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

        // Footer is laid out bottom-up first; the nested top-down layout
        // then fills the remaining space with the header and card list.
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            // --- Footer (added bottom-up: hint, button, separator) ---
            ui.add_space(2.0_f32);
            let toggle_hint =
                if cfg!(target_os = "macos") { "\u{2318}B to toggle" } else { "Ctrl+B to toggle" };
            ui.label(egui::RichText::new(toggle_hint).size(10.0_f32).color(p().text_disabled));
            ui.add_space(4.0_f32);
            let create_requested = render_new_workspace_button(ui);
            ui.add_space(6.0_f32);
            let (line_rect, _) = ui.allocate_exact_size(
                egui::Vec2::new(ui.available_width(), 1.0_f32),
                egui::Sense::hover(),
            );
            ui.painter().hline(line_rect.x_range(), line_rect.center().y, (1.0_f32, p().border));
            ui.add_space(4.0_f32);

            // --- Header + card list (top-down in the remaining space) ---
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                render_header(ui, workspaces.len());

                let mut clicked_index: Option<usize> = None;
                egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                    // 2px gap between cards.
                    ui.spacing_mut().item_spacing = egui::Vec2::new(0.0_f32, 2.0_f32);

                    for (i, tab) in workspaces.iter().enumerate() {
                        let is_active = i == active_index;
                        let is_editing = self.editing_index == Some(i);

                        let card_response =
                            self.render_card(ui, tab, is_active, is_editing, i, manager);

                        // Detect single click for switching (only when not editing)
                        if card_response.clicked() && !is_editing {
                            clicked_index = Some(i);
                        }

                        // Detect double-click to start renaming
                        if card_response.double_clicked() && !is_editing {
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
            });

            create_requested
        })
        .inner
    }

    /// Render a single workspace card.
    ///
    /// If `is_editing` is true, renders a `TextEdit` widget for inline rename.
    /// Returns the response for click/double-click detection.
    fn render_card(
        &mut self,
        ui: &mut egui::Ui,
        tab: &TabData,
        is_active: bool,
        is_editing: bool,
        index: usize,
        manager: &mut WorkspaceManager,
    ) -> egui::Response {
        // Taller card when a status segment extends the metadata line.
        let height = if tab.status.is_some() { 52.0_f32 } else { 42.0_f32 };
        let desired_size = egui::Vec2::new(ui.available_width(), height);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

        if !ui.is_rect_visible(rect) {
            return response;
        }

        let radius = egui::CornerRadius::same(CARD_RADIUS);
        // Inactive rows are de-emphasized at 0.8 opacity (text/border colors).
        let dim = if is_active { 1.0_f32 } else { INACTIVE_OPACITY };
        // Owned painter clipped to the card so content never bleeds out.
        let painter = ui.painter().with_clip_rect(rect);

        if is_editing {
            // Inline rename: the card rect becomes an input — `panel_bg`
            // fill with an `accent` border while the field has focus.
            painter.rect_filled(rect, radius, p().panel_bg);

            let edit_rect = egui::Rect::from_center_size(
                rect.center(),
                egui::Vec2::new(rect.width() - 2.0_f32 * CARD_PAD_X, 18.0_f32),
            );
            let edit_response = ui.put(
                edit_rect,
                egui::TextEdit::singleline(&mut self.edit_buffer)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace)
                    .frame(false)
                    .text_color_opt(Some(p().text_primary)),
            );

            let border_color = if edit_response.has_focus() { p().accent } else { p().border };
            painter.rect_stroke(
                rect,
                radius,
                egui::Stroke::new(1.0_f32, border_color),
                egui::StrokeKind::Inside,
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
            // Card surfaces: `accent` border + `panel_active_bg` fill when
            // active; hover on inactive rows also lifts to `panel_active_bg`.
            let fill =
                if is_active || response.hovered() { p().panel_active_bg } else { p().panel_bg };
            let border_color = if is_active { p().accent } else { p().border.gamma_multiply(dim) };
            painter.rect_filled(rect, radius, fill);
            painter.rect_stroke(
                rect,
                radius,
                egui::Stroke::new(1.0_f32, border_color),
                egui::StrokeKind::Inside,
            );

            let content = rect.shrink2(egui::Vec2::new(CARD_PAD_X, CARD_PAD_Y));

            // Unread notification badge: accent-filled circle r=8 at the
            // right edge of line 1, count in 9px mono `accent_fg`.
            let badge_reserved = if tab.unread > 0 { 20.0_f32 } else { 0.0_f32 };
            if tab.unread > 0 {
                let badge_center =
                    egui::Pos2::new(content.right() - 8.0_f32, content.top() + 8.0_f32);
                painter.circle_filled(badge_center, 8.0_f32, p().accent);
                painter.text(
                    badge_center,
                    egui::Align2::CENTER_CENTER,
                    tab.unread.to_string(),
                    egui::FontId::monospace(9.0_f32),
                    p().accent_fg,
                );
            }

            // Line 1: workspace name, 12.5px `text_primary`, elided with `…`.
            let title_color = p().text_primary.gamma_multiply(dim);
            let mut job = egui::text::LayoutJob::simple_singleline(
                tab.name.clone(),
                egui::FontId::proportional(12.5_f32),
                title_color,
            );
            job.wrap = egui::text::TextWrapping::truncate_at_width(
                (content.width() - badge_reserved).max(0.0_f32),
            );
            let galley = ui.fonts(|f| f.layout_job(job));
            painter.galley(content.left_top(), galley, title_color);

            // Line 2: mono 10px metadata — "N panes" plus an optional
            // " · status" segment (status in `warning`, cmux "Working" style).
            let mono = egui::FontId::monospace(10.0_f32);
            let meta_color = p().text_muted.gamma_multiply(dim);
            let line2_pos = egui::Pos2::new(content.left(), content.top() + 17.0_f32);
            let pane_count = tab.pane_count;
            let panes_text =
                if pane_count == 1 { "1 pane".to_owned() } else { format!("{pane_count} panes") };
            let panes_rect = painter.text(
                line2_pos,
                egui::Align2::LEFT_TOP,
                panes_text,
                mono.clone(),
                meta_color,
            );
            if let Some(status) = &tab.status {
                let sep_rect = painter.text(
                    egui::Pos2::new(panes_rect.right(), line2_pos.y),
                    egui::Align2::LEFT_TOP,
                    " \u{b7} ",
                    mono.clone(),
                    meta_color,
                );
                painter.text(
                    egui::Pos2::new(sep_rect.right(), line2_pos.y),
                    egui::Align2::LEFT_TOP,
                    status,
                    mono,
                    p().warning.gamma_multiply(dim),
                );
            }
        }

        // Progress: 3px capsule along the card bottom — `accent` fill on a
        // `border`-color track, fill width = track width × progress.
        if let Some(progress) = tab.progress {
            // Clamp to [0.0, 1.0], treating NaN/infinite as 0.0 so they
            // don't produce degenerate geometry and UI glitches.
            let clamped =
                if progress.is_finite() { progress.clamp(0.0_f32, 1.0_f32) } else { 0.0_f32 };
            let capsule = egui::CornerRadius::same(2);
            let track = egui::Rect::from_min_max(
                egui::Pos2::new(rect.left() + 1.0_f32, rect.bottom() - 4.0_f32),
                egui::Pos2::new(rect.right() - 1.0_f32, rect.bottom() - 1.0_f32),
            );
            painter.rect_filled(track, capsule, p().border.gamma_multiply(dim));
            let fill_width = track.width() * clamped;
            if fill_width > 0.0_f32 {
                let fill_rect = egui::Rect::from_min_size(
                    track.min,
                    egui::Vec2::new(fill_width, track.height()),
                );
                painter.rect_filled(fill_rect, capsule, p().accent);
            }
        }

        response
    }
}

/// Render the sidebar header row: `"Workspaces"` label with a workspace
/// count pill on the right (fully rounded, `panel_bg` fill, 1px `border`).
fn render_header(ui: &mut egui::Ui, count: usize) {
    let (rect, _) = ui
        .allocate_exact_size(egui::Vec2::new(ui.available_width(), 32.0_f32), egui::Sense::hover());
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

    // Count pill: h=14, min-w 14, 9px mono text.
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

/// Render the footer `+ New Workspace` button.
///
/// Returns `true` when clicked. The click is routed through
/// `RmuxApp::create_workspace_with_terminal` by `app.rs` — the same path as
/// the Cmd/Ctrl+N shortcut — so the new workspace gets a live terminal.
/// Hover lifts the fill to `panel_active_bg` with an `accent` border
/// (arbor "Add Repository" pattern).
fn render_new_workspace_button(ui: &mut egui::Ui) -> bool {
    let size = egui::Vec2::new(ui.available_width(), crate::ui::theme::metrics::BUTTON_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return false;
    }

    let hovered = response.hovered();
    let fill = if hovered { p().panel_active_bg } else { p().panel_bg };
    let border_color = if hovered { p().accent } else { p().border };
    let radius = egui::CornerRadius::same(CARD_RADIUS);
    let painter = ui.painter();
    painter.rect_filled(rect, radius, fill);
    painter.rect_stroke(
        rect,
        radius,
        egui::Stroke::new(1.0_f32, border_color),
        egui::StrokeKind::Inside,
    );

    let label_font = egui::FontId::proportional(12.0_f32);
    let plus = painter.layout_no_wrap("+ ".to_owned(), label_font.clone(), p().accent);
    let label = painter.layout_no_wrap("New Workspace".to_owned(), label_font, p().text_primary);
    let total_width = plus.size().x + label.size().x;
    let plus_pos = egui::Pos2::new(
        rect.center().x - total_width / 2.0_f32,
        rect.center().y - plus.size().y / 2.0_f32,
    );
    let label_pos =
        egui::Pos2::new(plus_pos.x + plus.size().x, rect.center().y - label.size().y / 2.0_f32);
    painter.galley(plus_pos, plus, p().accent);
    painter.galley(label_pos, label, p().text_primary);

    let shortcut_hint = if cfg!(target_os = "macos") { "\u{2318}N" } else { "Ctrl+N" };
    let response = response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(format!("New workspace ({shortcut_hint})"));
    response.clicked()
}
