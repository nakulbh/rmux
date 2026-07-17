//! cmux-style help menu — small circle-question control in the sidebar
//! bottom-left corner.
//!
//! Opens a compact popup with product links, keyboard shortcuts, and a
//! lightweight update check toast (Checking… / No Updates Available).

use std::time::{Duration, Instant};

use crate::ui::theme;

/// Official project URLs (open in the system browser).
const URL_GITHUB: &str = "https://github.com/nakulbh/rmux";
const URL_ISSUES: &str = "https://github.com/nakulbh/rmux/issues";
const URL_DOCS: &str = "https://github.com/nakulbh/rmux#readme";
const URL_CHANGELOG: &str = "https://github.com/nakulbh/rmux/releases";
const URL_FEEDBACK: &str = "https://github.com/nakulbh/rmux/issues/new";
/// Community Discord is not published yet — open GitHub Discussions-style issues.
const URL_DISCORD: &str = "https://github.com/nakulbh/rmux/issues";

/// Hit target for the circle-question button.
const BTN_SIZE: f32 = 22.0_f32;
/// Icon stroke radius (Lucide-style 24×24 viewBox scaled down).
const ICON_R: f32 = 8.0_f32;

/// Result of a local "check for updates" pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateCheckResult {
    /// No newer release known (default outcome without a remote checker).
    NoUpdates,
    /// A newer release may exist — user is pointed at the releases page.
    #[allow(dead_code)]
    Available,
}

/// In-progress or finished update-check toast state.
#[derive(Debug, Clone)]
enum UpdateToast {
    /// Spinner toast.
    Checking { started: Instant },
    /// Result pill; auto-dismisses after a few seconds.
    Done { result: UpdateCheckResult, since: Instant },
}

/// Help / about menu state owned by the app, rendered from the sidebar footer.
#[derive(Debug, Default)]
pub struct HelpMenu {
    /// Whether the popup menu is open.
    menu_open: bool,
    /// Welcome-to-rmux dialog.
    welcome_open: bool,
    /// Keyboard shortcuts reference window.
    shortcuts_open: bool,
    /// Update-check toast, if any.
    toast: Option<UpdateToast>,
}

impl HelpMenu {
    /// Create a closed help menu.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the welcome dialog is open (for tests).
    #[cfg(test)]
    pub fn is_welcome_open(&self) -> bool {
        self.welcome_open
    }

    /// Whether the shortcuts window is open (for tests).
    #[cfg(test)]
    pub fn is_shortcuts_open(&self) -> bool {
        self.shortcuts_open
    }

    /// Whether the popup is open (for tests).
    #[cfg(test)]
    pub fn is_menu_open(&self) -> bool {
        self.menu_open
    }

    /// Start an update check (for tests / callers).
    pub fn start_update_check(&mut self) {
        self.toast = Some(UpdateToast::Checking { started: Instant::now() });
        tracing::info!("Update check started");
    }

    /// Advance toast timers. Call once per frame from overlays.
    fn tick_toast(&mut self) {
        let Some(toast) = self.toast.as_ref() else {
            return;
        };
        match toast {
            UpdateToast::Checking { started }
                if started.elapsed() >= Duration::from_millis(900) =>
            {
                // No remote release API yet — report current build as up to date.
                self.toast = Some(UpdateToast::Done {
                    result: UpdateCheckResult::NoUpdates,
                    since: Instant::now(),
                });
            }
            UpdateToast::Done { since, .. } if since.elapsed() >= Duration::from_secs(4) => {
                self.toast = None;
            }
            _ => {}
        }
    }

    /// Draw the small circle-question control (left-aligned). Returns click response.
    pub fn show_button(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let p = theme::palette();
        let (rect, response) =
            ui.allocate_exact_size(egui::Vec2::splat(BTN_SIZE), egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let center = rect.center();
            let hovered = response.hovered() || self.menu_open;
            let stroke_color = if hovered { p.text_primary } else { p.text_muted };
            let fill = if self.menu_open {
                p.panel_active_bg
            } else if hovered {
                p.panel_bg
            } else {
                egui::Color32::TRANSPARENT
            };

            if fill != egui::Color32::TRANSPARENT {
                ui.painter().circle_filled(center, BTN_SIZE / 2.0_f32, fill);
            }

            // Lucide-style circle-question: outer ring + curved "?" + bottom dot.
            ui.painter().circle_stroke(center, ICON_R, egui::Stroke::new(1.5_f32, stroke_color));
            // Question mark body (simple text glyph, sized to the icon).
            ui.painter().text(
                egui::Pos2::new(center.x, center.y - 1.0_f32),
                egui::Align2::CENTER_CENTER,
                "?",
                egui::FontId::proportional(12.0_f32),
                stroke_color,
            );
        }

        let response =
            response.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Help & about");

        if response.clicked() {
            self.menu_open = !self.menu_open;
        }

        response
    }

    /// Popup anchored above the help button + dialogs + update toast.
    pub fn show_overlays(&mut self, ctx: &egui::Context, button_rect: Option<egui::Rect>) {
        self.tick_toast();

        if self.menu_open {
            self.show_popup(ctx, button_rect);
        }

        if self.welcome_open {
            self.show_welcome(ctx);
        }

        if self.shortcuts_open {
            self.show_shortcuts(ctx);
        }

        self.show_toast(ctx);
    }

    /// Dark rounded popup matching cmux's help menu.
    fn show_popup(&mut self, ctx: &egui::Context, button_rect: Option<egui::Rect>) {
        let p = theme::palette();
        let mut open = self.menu_open;

        let mut area = egui::Area::new(egui::Id::new("rmux_help_menu"))
            .order(egui::Order::Foreground)
            .constrain(true)
            .interactable(true);

        if let Some(rect) = button_rect {
            // Anchor just above the button, left-aligned with the sidebar footer.
            area = area.fixed_pos(egui::Pos2::new(rect.left(), rect.top() - 4.0_f32));
            area = area.pivot(egui::Align2::LEFT_BOTTOM);
        } else {
            area = area.anchor(egui::Align2::LEFT_BOTTOM, egui::Vec2::new(12.0_f32, -40.0_f32));
        }

        let response = area.show(ctx, |ui| {
            egui::Frame::popup(&ctx.style())
                .fill(p.panel_bg)
                .stroke(egui::Stroke::new(1.0_f32, p.border))
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(6, 6))
                .shadow(egui::Shadow {
                    offset: [0, 4],
                    blur: 16,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(120),
                })
                .show(ui, |ui| {
                    ui.set_min_width(220.0_f32);
                    ui.spacing_mut().item_spacing = egui::Vec2::new(0.0_f32, 1.0_f32);

                    let mut action: Option<HelpAction> = None;

                    if menu_item(ui, "Welcome to rmux!", None).clicked() {
                        action = Some(HelpAction::Welcome);
                    }
                    if menu_item(ui, "Send Feedback", Some(MenuTrailing::Label("None"))).clicked() {
                        action = Some(HelpAction::Feedback);
                    }
                    if menu_item(ui, "Keyboard Shortcuts", None).clicked() {
                        action = Some(HelpAction::Shortcuts);
                    }
                    // Browser import is cmux-specific; keep the row but mark unavailable.
                    let import = menu_item(ui, "Import Browser Data…", None);
                    if import.clicked() {
                        action = Some(HelpAction::ImportBrowserStub);
                    }
                    ui.add_space(4.0_f32);
                    ui.separator();
                    ui.add_space(4.0_f32);

                    if menu_item(ui, "Docs", Some(MenuTrailing::External)).clicked() {
                        action = Some(HelpAction::OpenUrl(URL_DOCS));
                    }
                    if menu_item(ui, "Changelog", Some(MenuTrailing::External)).clicked() {
                        action = Some(HelpAction::OpenUrl(URL_CHANGELOG));
                    }
                    if menu_item(ui, "GitHub", Some(MenuTrailing::External)).clicked() {
                        action = Some(HelpAction::OpenUrl(URL_GITHUB));
                    }
                    if menu_item(ui, "GitHub Issues", Some(MenuTrailing::External)).clicked() {
                        action = Some(HelpAction::OpenUrl(URL_ISSUES));
                    }
                    if menu_item(ui, "Discord", Some(MenuTrailing::External)).clicked() {
                        action = Some(HelpAction::OpenUrl(URL_DISCORD));
                    }

                    ui.add_space(4.0_f32);
                    ui.separator();
                    ui.add_space(4.0_f32);

                    if menu_item(ui, "Check for Updates", None).clicked() {
                        action = Some(HelpAction::CheckUpdates);
                    }

                    if let Some(a) = action {
                        self.handle_action(a);
                        open = false;
                    }
                });
        });

        // Close when clicking outside the popup (and not on the button).
        let popup_rect = response.response.rect;
        let clicked_elsewhere = ctx.input(|i| {
            i.pointer.any_click()
                && i.pointer.interact_pos().is_some_and(|pos| {
                    !popup_rect.contains(pos) && button_rect.is_none_or(|b| !b.contains(pos))
                })
        });
        if clicked_elsewhere {
            open = false;
        }
        // Escape closes the menu.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            open = false;
        }

        self.menu_open = open;
    }

    fn handle_action(&mut self, action: HelpAction) {
        match action {
            HelpAction::Welcome => {
                self.welcome_open = true;
            }
            HelpAction::Feedback => {
                open_url(URL_FEEDBACK);
            }
            HelpAction::Shortcuts => {
                self.shortcuts_open = true;
            }
            HelpAction::ImportBrowserStub => {
                tracing::info!("Import Browser Data is not available in rmux yet");
            }
            HelpAction::OpenUrl(url) => {
                open_url(url);
            }
            HelpAction::CheckUpdates => {
                self.start_update_check();
            }
        }
    }

    fn show_welcome(&mut self, ctx: &egui::Context) {
        let p = theme::palette();
        let mut open = self.welcome_open;
        egui::Window::new("Welcome to rmux!")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(360.0_f32)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(p.panel_bg)
                    .stroke(egui::Stroke::new(1.0_f32, p.border)),
            )
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!("rmux v{}", env!("CARGO_PKG_VERSION")))
                        .size(14.0_f32)
                        .color(p.text_primary),
                );
                ui.add_space(8.0_f32);
                ui.label(
                    egui::RichText::new(
                        "Cross-platform terminal multiplexer GUI. Workspaces, splits, \
                         browser panes, and a socket API — inspired by cmux.",
                    )
                    .size(12.0_f32)
                    .color(p.text_muted),
                );
                ui.add_space(12.0_f32);
                ui.horizontal(|ui| {
                    if ui.button("Keyboard Shortcuts").clicked() {
                        self.shortcuts_open = true;
                    }
                    if ui.button("Docs").clicked() {
                        open_url(URL_DOCS);
                    }
                    if ui.button("GitHub").clicked() {
                        open_url(URL_GITHUB);
                    }
                });
            });
        self.welcome_open = open;
    }

    fn show_shortcuts(&mut self, ctx: &egui::Context) {
        let p = theme::palette();
        let mut open = self.shortcuts_open;
        let mod_label = if cfg!(target_os = "macos") { "⌘" } else { "Ctrl+" };

        egui::Window::new("Keyboard Shortcuts")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(380.0_f32)
            .default_height(420.0_f32)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(p.panel_bg)
                    .stroke(egui::Stroke::new(1.0_f32, p.border)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    shortcut_section(
                        ui,
                        "General",
                        &[
                            (&format!("{mod_label}N"), "New workspace"),
                            (&format!("{mod_label}B"), "Toggle sidebar"),
                            (&format!("{mod_label}I"), "Toggle notifications"),
                            (&format!("{mod_label},"), "Settings"),
                            (&format!("{mod_label}W"), "Close pane / tab"),
                        ],
                    );
                    shortcut_section(
                        ui,
                        "Panes",
                        &[
                            (&format!("{mod_label}D"), "Split right"),
                            (&format!("{mod_label}⇧D"), "Split down"),
                            (&format!("{mod_label}T"), "New terminal tab"),
                            ("⌥←/→/↑/↓", "Focus adjacent pane"),
                        ],
                    );
                    shortcut_section(
                        ui,
                        "Workspaces",
                        &[
                            (&format!("{mod_label}1…9"), "Switch workspace"),
                            (&format!("{mod_label}["), "Previous workspace"),
                            (&format!("{mod_label}]"), "Next workspace"),
                            (&format!("{mod_label}⇧R"), "Rename workspace"),
                        ],
                    );
                    shortcut_section(
                        ui,
                        "Terminal",
                        &[
                            (&format!("{mod_label}+ / −"), "Font size"),
                            (&format!("{mod_label}0"), "Reset font size"),
                            (&format!("{mod_label}F"), "Find"),
                            (&format!("{mod_label}K"), "Clear scrollback"),
                        ],
                    );
                    ui.add_space(8.0_f32);
                    ui.label(
                        egui::RichText::new("Hold ⌘/Ctrl to see chrome shortcut badges.")
                            .size(11.0_f32)
                            .color(p.text_disabled),
                    );
                    ui.hyperlink_to(
                        egui::RichText::new("Full reference on GitHub").size(11.0_f32),
                        URL_DOCS,
                    );
                });
            });
        self.shortcuts_open = open;
    }

    fn show_toast(&self, ctx: &egui::Context) {
        let Some(toast) = self.toast.as_ref() else {
            return;
        };
        let p = theme::palette();

        egui::Area::new(egui::Id::new("rmux_update_toast"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_BOTTOM, egui::Vec2::new(0.0_f32, -48.0_f32))
            .show(ctx, |ui| {
                match toast {
                    UpdateToast::Checking { .. } => {
                        egui::Frame::NONE
                            .fill(p.panel_bg)
                            .stroke(egui::Stroke::new(1.0_f32, p.border))
                            .corner_radius(egui::CornerRadius::same(20))
                            .inner_margin(egui::Margin::symmetric(14, 8))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(
                                        egui::RichText::new("Checking for Updates…")
                                            .size(13.0_f32)
                                            .color(p.text_primary),
                                    );
                                });
                            });
                    }
                    UpdateToast::Done { result, .. } => {
                        let (fill, label) = match result {
                            UpdateCheckResult::NoUpdates => (p.accent, "No Updates Available"),
                            UpdateCheckResult::Available => (p.success, "Update Available"),
                        };
                        egui::Frame::NONE
                            .fill(fill)
                            .corner_radius(egui::CornerRadius::same(20))
                            .inner_margin(egui::Margin::symmetric(14, 8))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Info circle glyph
                                    ui.label(
                                        egui::RichText::new("ⓘ").size(14.0_f32).color(p.accent_fg),
                                    );
                                    ui.label(
                                        egui::RichText::new(label)
                                            .size(13.0_f32)
                                            .color(p.accent_fg)
                                            .strong(),
                                    );
                                });
                            });
                    }
                }
            });
    }
}

/// Internal menu click targets.
enum HelpAction {
    Welcome,
    Feedback,
    Shortcuts,
    ImportBrowserStub,
    OpenUrl(&'static str),
    CheckUpdates,
}

/// Right-side affordance on a menu row.
enum MenuTrailing {
    Label(&'static str),
    External,
}

fn menu_item(ui: &mut egui::Ui, label: &str, trailing: Option<MenuTrailing>) -> egui::Response {
    let p = theme::palette();
    let desired = egui::Vec2::new(ui.available_width().max(200.0_f32), 28.0_f32);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        if hovered {
            ui.painter().rect_filled(rect, egui::CornerRadius::same(6), p.panel_active_bg);
        }

        let text_pos = egui::Pos2::new(rect.left() + 10.0_f32, rect.center().y);
        ui.painter().text(
            text_pos,
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(13.0_f32),
            p.text_primary,
        );

        match trailing {
            Some(MenuTrailing::Label(t)) => {
                ui.painter().text(
                    egui::Pos2::new(rect.right() - 10.0_f32, rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    t,
                    egui::FontId::proportional(12.0_f32),
                    p.text_disabled,
                );
            }
            Some(MenuTrailing::External) => {
                // Small "↗" external-link mark (cmux style).
                ui.painter().text(
                    egui::Pos2::new(rect.right() - 10.0_f32, rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    "\u{2197}",
                    egui::FontId::proportional(12.0_f32),
                    p.text_disabled,
                );
            }
            None => {}
        }
    }

    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn shortcut_section(ui: &mut egui::Ui, title: &str, rows: &[(&str, &str)]) {
    let p = theme::palette();
    ui.add_space(6.0_f32);
    ui.label(egui::RichText::new(title).size(11.0_f32).color(p.text_muted).strong());
    ui.add_space(4.0_f32);
    for (chord, desc) in rows {
        ui.horizontal(|ui| {
            let chord_galley = ui.painter().layout_no_wrap(
                (*chord).to_owned(),
                egui::FontId::monospace(11.0_f32),
                p.text_primary,
            );
            let chip_w = (chord_galley.size().x + 12.0_f32).max(48.0_f32);
            let (chip_rect, _) =
                ui.allocate_exact_size(egui::Vec2::new(chip_w, 20.0_f32), egui::Sense::hover());
            ui.painter().rect_filled(chip_rect, egui::CornerRadius::same(4), p.app_bg);
            ui.painter().rect_stroke(
                chip_rect,
                egui::CornerRadius::same(4),
                egui::Stroke::new(1.0_f32, p.border),
                egui::StrokeKind::Inside,
            );
            ui.painter().galley(
                egui::Pos2::new(
                    chip_rect.center().x - chord_galley.size().x / 2.0_f32,
                    chip_rect.center().y - chord_galley.size().y / 2.0_f32,
                ),
                chord_galley,
                p.text_primary,
            );
            ui.label(egui::RichText::new(*desc).size(12.0_f32).color(p.text_primary));
        });
    }
}

/// Open a URL with the platform default handler (no extra crate).
fn open_url(url: &str) {
    tracing::info!(%url, "Opening URL from help menu");
    let result = open_url_impl(url);
    if let Err(err) = result {
        tracing::warn!(%url, error = %err, "Failed to open URL");
    }
}

fn open_url_impl(url: &str) -> std::io::Result<()> {
    // Genuinely platform-specific: each OS has a different default opener.
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn().map(|_| ())
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn().map(|_| ())
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn().map(|_| ())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = url;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "open_url not supported on this platform",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_menu_default_closed() {
        let help = HelpMenu::new();
        assert!(!help.is_menu_open());
        assert!(!help.is_welcome_open());
        assert!(!help.is_shortcuts_open());
    }

    #[test]
    fn test_start_update_check_sets_checking_toast() {
        let mut help = HelpMenu::new();
        help.start_update_check();
        assert!(matches!(help.toast, Some(UpdateToast::Checking { .. })));
    }

    #[test]
    fn test_tick_toast_promotes_checking_to_no_updates() {
        let mut help = HelpMenu::new();
        help.toast =
            Some(UpdateToast::Checking { started: Instant::now() - Duration::from_secs(2) });
        help.tick_toast();
        assert!(matches!(
            help.toast,
            Some(UpdateToast::Done { result: UpdateCheckResult::NoUpdates, .. })
        ));
    }

    #[test]
    fn test_tick_toast_dismisses_done_after_timeout() {
        let mut help = HelpMenu::new();
        help.toast = Some(UpdateToast::Done {
            result: UpdateCheckResult::NoUpdates,
            since: Instant::now() - Duration::from_secs(10),
        });
        help.tick_toast();
        assert!(help.toast.is_none());
    }

    #[test]
    fn test_handle_action_welcome_opens_dialog() {
        let mut help = HelpMenu::new();
        help.handle_action(HelpAction::Welcome);
        assert!(help.is_welcome_open());
    }

    #[test]
    fn test_handle_action_shortcuts_opens_window() {
        let mut help = HelpMenu::new();
        help.handle_action(HelpAction::Shortcuts);
        assert!(help.is_shortcuts_open());
    }

    #[test]
    fn test_handle_action_check_updates() {
        let mut help = HelpMenu::new();
        help.handle_action(HelpAction::CheckUpdates);
        assert!(matches!(help.toast, Some(UpdateToast::Checking { .. })));
    }
}
