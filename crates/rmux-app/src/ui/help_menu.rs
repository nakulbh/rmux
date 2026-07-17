//! cmux-style help menu — small circle-question control in the sidebar
//! bottom-left corner.
//!
//! Opens a compact popup with product links, keyboard shortcuts, and a
//! GitHub-backed update check toast (Checking… / No Updates / Update Available).

use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::ui::theme;
use crate::update::{self, ApplyUpdateOutcome, UpdateCheckOutcome, UpdateSource, UpdateStatus};

/// Official project URLs (open in the system browser).
const URL_GITHUB: &str = "https://github.com/nakulbh/rmux";
const URL_ISSUES: &str = "https://github.com/nakulbh/rmux/issues";
const URL_DOCS: &str = "https://github.com/nakulbh/rmux#readme";
const URL_CHANGELOG: &str = "https://github.com/nakulbh/rmux/releases";
const URL_FEEDBACK: &str = "https://github.com/nakulbh/rmux/issues/new";
/// Community Discord is not published yet — open GitHub Discussions-style issues.
const URL_DISCORD: &str = "https://github.com/nakulbh/rmux/issues";

/// Hit target for the circle-question button (cmux-scale: compact footer glyph).
const BTN_SIZE: f32 = 16.0_f32;
/// Outer ring radius — slightly smaller than the hit target so it reads as a
/// quiet chrome control, not a full toolbar button.
const ICON_R: f32 = 6.0_f32;

/// Minimum time to show the spinner so it doesn't flash.
const CHECK_MIN_DISPLAY: Duration = Duration::from_millis(600);
/// Max wait for the background check before reporting a timeout error.
const CHECK_TIMEOUT: Duration = Duration::from_secs(20);
/// Install can compile from source — allow a long window.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// How long passive result toasts stay on screen.
const TOAST_HOLD: Duration = Duration::from_secs(6);
/// Keep "Update Available" visible long enough to click and install.
const AVAILABLE_HOLD: Duration = Duration::from_secs(60);
/// Keep "Updated · click to restart" until the user acts (or times out).
const RESTART_HOLD: Duration = Duration::from_secs(120);

/// Finished toast kind after a check or install.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ToastResult {
    NoUpdates,
    /// Click starts a system install of the remote build.
    Available {
        label: String,
        url: String,
        source: UpdateSource,
    },
    Failed {
        message: String,
    },
    /// Install succeeded — click relaunches the new binary and quits.
    Installed {
        binary_path: String,
        installed_ref: String,
    },
}

/// In-progress or finished update toast state.
#[derive(Debug)]
enum UpdateToast {
    /// Spinner while querying GitHub.
    Checking {
        started: Instant,
        rx: mpsc::Receiver<UpdateCheckOutcome>,
        ready: Option<UpdateCheckOutcome>,
    },
    /// Spinner while `install.sh` builds/installs (can take minutes).
    Installing {
        started: Instant,
        label: String,
        rx: mpsc::Receiver<ApplyUpdateOutcome>,
        ready: Option<ApplyUpdateOutcome>,
    },
    /// Result pill.
    Done { result: ToastResult, since: Instant },
}

/// Modal shown after a successful install: ask the user to quit and reopen.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RestartPrompt {
    binary_path: String,
    installed_ref: String,
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
    /// Post-install "quit and reopen?" dialog.
    restart_prompt: Option<RestartPrompt>,
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

    /// Start a background GitHub update check and show the spinner toast.
    pub fn start_update_check(&mut self) {
        // Don't stack concurrent checks / installs.
        if matches!(self.toast, Some(UpdateToast::Checking { .. } | UpdateToast::Installing { .. }))
        {
            return;
        }
        let rx = update::spawn_check();
        self.toast = Some(UpdateToast::Checking { started: Instant::now(), rx, ready: None });
        tracing::info!("Update check started (GitHub Releases + main commit)");
    }

    /// Start installing the available update into the system (background).
    fn start_apply_update(&mut self, source: UpdateSource, label: String) {
        if matches!(self.toast, Some(UpdateToast::Installing { .. })) {
            return;
        }
        let rx = update::spawn_apply_update(source, label.clone());
        self.toast =
            Some(UpdateToast::Installing { started: Instant::now(), label, rx, ready: None });
        tracing::info!(?source, "system update install started");
    }

    /// Advance toast timers and poll background work. Call once per frame.
    fn tick_toast(&mut self) {
        let Some(toast) = self.toast.take() else {
            return;
        };

        let next = match toast {
            UpdateToast::Checking { started, rx, mut ready } => {
                if ready.is_none() {
                    match rx.try_recv() {
                        Ok(outcome) => ready = Some(outcome),
                        Err(mpsc::TryRecvError::Empty) => {}
                        Err(mpsc::TryRecvError::Disconnected) => {
                            ready = Some(UpdateCheckOutcome {
                                status: UpdateStatus::Error(
                                    "update check thread ended unexpectedly".into(),
                                ),
                            });
                        }
                    }
                }

                let elapsed = started.elapsed();
                if let Some(outcome) = ready {
                    if elapsed >= CHECK_MIN_DISPLAY {
                        Some(UpdateToast::Done {
                            result: toast_result_from_outcome(outcome),
                            since: Instant::now(),
                        })
                    } else {
                        Some(UpdateToast::Checking { started, rx, ready: Some(outcome) })
                    }
                } else if elapsed >= CHECK_TIMEOUT {
                    Some(UpdateToast::Done {
                        result: ToastResult::Failed {
                            message: "timed out contacting GitHub".into(),
                        },
                        since: Instant::now(),
                    })
                } else {
                    Some(UpdateToast::Checking { started, rx, ready: None })
                }
            }
            UpdateToast::Installing { started, label, rx, mut ready } => {
                if ready.is_none() {
                    match rx.try_recv() {
                        Ok(outcome) => ready = Some(outcome),
                        Err(mpsc::TryRecvError::Empty) => {}
                        Err(mpsc::TryRecvError::Disconnected) => {
                            ready = Some(ApplyUpdateOutcome::Failed {
                                message: "install thread ended unexpectedly".into(),
                            });
                        }
                    }
                }

                let elapsed = started.elapsed();
                if let Some(outcome) = ready {
                    let result = toast_result_from_apply(outcome);
                    // On success, open the quit-and-reopen dialog immediately.
                    if let ToastResult::Installed { binary_path, installed_ref } = &result {
                        self.restart_prompt = Some(RestartPrompt {
                            binary_path: binary_path.clone(),
                            installed_ref: installed_ref.clone(),
                        });
                    }
                    Some(UpdateToast::Done { result, since: Instant::now() })
                } else if elapsed >= INSTALL_TIMEOUT {
                    Some(UpdateToast::Done {
                        result: ToastResult::Failed {
                            message: "install timed out (still building?)".into(),
                        },
                        since: Instant::now(),
                    })
                } else {
                    Some(UpdateToast::Installing { started, label, rx, ready: None })
                }
            }
            UpdateToast::Done { result, since } => {
                let hold = match &result {
                    ToastResult::Available { .. } => AVAILABLE_HOLD,
                    ToastResult::Installed { .. } => RESTART_HOLD,
                    _ => TOAST_HOLD,
                };
                if since.elapsed() >= hold {
                    None
                } else {
                    Some(UpdateToast::Done { result, since })
                }
            }
        };

        self.toast = next;
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

            // Lucide-style circle-question: thin outer ring + compact "?".
            ui.painter().circle_stroke(center, ICON_R, egui::Stroke::new(1.25_f32, stroke_color));
            ui.painter().text(
                egui::Pos2::new(center.x, center.y - 0.5_f32),
                egui::Align2::CENTER_CENTER,
                "?",
                egui::FontId::proportional(9.0_f32),
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

        if self.restart_prompt.is_some() {
            self.show_restart_prompt(ctx);
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
                let sha = update::local_git_sha();
                let version_line = if sha.is_empty() {
                    format!("rmux v{}", update::local_version())
                } else {
                    format!("rmux v{} ({sha})", update::local_version())
                };
                ui.label(egui::RichText::new(version_line).size(14.0_f32).color(p.text_primary));
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

    fn show_toast(&mut self, ctx: &egui::Context) {
        let Some(toast) = self.toast.as_ref() else {
            return;
        };
        let p = theme::palette();

        // Interaction payloads (clone so we can mutate self after the paint).
        enum ClickAction {
            Install { source: UpdateSource, label: String },
            Restart { binary_path: String, installed_ref: String },
        }
        let click = match toast {
            UpdateToast::Done { result: ToastResult::Available { label, source, .. }, .. } => {
                Some(ClickAction::Install { source: *source, label: label.clone() })
            }
            UpdateToast::Done {
                result: ToastResult::Installed { binary_path, installed_ref },
                ..
            } => Some(ClickAction::Restart {
                binary_path: binary_path.clone(),
                installed_ref: installed_ref.clone(),
            }),
            _ => None,
        };

        let response = egui::Area::new(egui::Id::new("rmux_update_toast"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_BOTTOM, egui::Vec2::new(0.0_f32, -48.0_f32))
            .show(ctx, |ui| match toast {
                UpdateToast::Checking { .. } => {
                    spinner_toast(ui, &p, "Checking for Updates…");
                }
                UpdateToast::Installing { label, started, .. } => {
                    let mins = started.elapsed().as_secs() / 60;
                    let text = if mins == 0 {
                        format!("Updating rmux · {label}…")
                    } else {
                        format!("Updating rmux · {label}… ({mins}m)")
                    };
                    spinner_toast(ui, &p, &text);
                }
                UpdateToast::Done { result, .. } => {
                    let (fill, label, stroke, text_color, hover) = match result {
                        ToastResult::NoUpdates => (
                            p.accent,
                            "No Updates Available".to_string(),
                            egui::Stroke::NONE,
                            p.accent_fg,
                            None,
                        ),
                        ToastResult::Available { label, .. } => (
                            p.success,
                            format!("Update Available · {label}  ·  Click to install"),
                            egui::Stroke::NONE,
                            p.accent_fg,
                            Some("Download, build, and install into ~/.local/bin"),
                        ),
                        ToastResult::Installed { installed_ref, .. } => (
                            p.success,
                            format!("Updated to {installed_ref}  ·  Restart required"),
                            egui::Stroke::NONE,
                            p.accent_fg,
                            Some("Open the restart dialog to quit and reopen rmux"),
                        ),
                        ToastResult::Failed { message } => (
                            p.panel_bg,
                            format!("Update failed · {message}"),
                            egui::Stroke::new(1.0_f32, p.border),
                            p.text_primary,
                            None,
                        ),
                    };

                    let frame_resp = egui::Frame::NONE
                        .fill(fill)
                        .stroke(stroke)
                        .corner_radius(egui::CornerRadius::same(20))
                        .inner_margin(egui::Margin::symmetric(14, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                paint_info_icon(ui, text_color, 14.0_f32);
                                ui.add_space(4.0_f32);
                                ui.label(
                                    egui::RichText::new(label)
                                        .size(13.0_f32)
                                        .color(text_color)
                                        .strong(),
                                );
                            });
                        });
                    if let Some(tip) = hover {
                        frame_resp
                            .response
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(tip);
                    }
                }
            });

        if !response.response.clicked() {
            return;
        }
        match click {
            Some(ClickAction::Install { source, label }) => {
                self.start_apply_update(source, label);
            }
            Some(ClickAction::Restart { binary_path, installed_ref }) => {
                // Re-open the quit-and-reopen dialog if the user dismissed it.
                self.restart_prompt = Some(RestartPrompt { binary_path, installed_ref });
            }
            None => {}
        }
    }

    /// Modal: update finished — ask the user to quit and reopen.
    fn show_restart_prompt(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.restart_prompt.clone() else {
            return;
        };
        let p = theme::palette();
        let mut open = true;
        let mut do_restart = false;
        let mut do_later = false;

        egui::Window::new("Restart to finish update")
            .id(egui::Id::new("rmux_restart_prompt"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .default_width(380.0_f32)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(p.panel_bg)
                    .stroke(egui::Stroke::new(1.0_f32, p.border)),
            )
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!("rmux was updated to {}.", prompt.installed_ref))
                        .size(14.0_f32)
                        .color(p.text_primary)
                        .strong(),
                );
                ui.add_space(8.0_f32);
                ui.label(
                    egui::RichText::new(
                        "Quit and reopen rmux to use the new version. Unsaved terminal \
                         sessions will be closed.",
                    )
                    .size(12.0_f32)
                    .color(p.text_muted),
                );
                ui.add_space(16.0_f32);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let restart = ui.add(
                            egui::Button::new(
                                egui::RichText::new("Quit & Reopen").color(p.accent_fg).strong(),
                            )
                            .fill(p.accent)
                            .corner_radius(egui::CornerRadius::same(6))
                            .min_size(egui::Vec2::new(120.0_f32, 28.0_f32)),
                        );
                        if restart.clicked() {
                            do_restart = true;
                        }
                        ui.add_space(8.0_f32);
                        if ui.button("Later").clicked() {
                            do_later = true;
                        }
                    });
                });
            });

        if do_restart {
            self.perform_restart(ctx, &prompt.binary_path);
            return;
        }
        if do_later || !open {
            // Keep the toast so they can reopen this dialog later by clicking it.
            self.restart_prompt = None;
        }
    }

    /// Spawn the newly installed binary, then close this window.
    fn perform_restart(&mut self, ctx: &egui::Context, binary_path: &str) {
        match update::relaunch(binary_path) {
            Ok(()) => {
                self.restart_prompt = None;
                self.toast = None;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Err(err) => {
                tracing::error!(%err, "relaunch failed");
                self.restart_prompt = None;
                self.toast = Some(UpdateToast::Done {
                    result: ToastResult::Failed { message: err },
                    since: Instant::now(),
                });
            }
        }
    }
}

fn spinner_toast(ui: &mut egui::Ui, p: &theme::Palette, text: &str) {
    egui::Frame::NONE
        .fill(p.panel_bg)
        .stroke(egui::Stroke::new(1.0_f32, p.border))
        .corner_radius(egui::CornerRadius::same(20))
        .inner_margin(egui::Margin::symmetric(14, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(egui::RichText::new(text).size(13.0_f32).color(p.text_primary));
            });
        });
}

fn toast_result_from_outcome(outcome: UpdateCheckOutcome) -> ToastResult {
    match outcome.status {
        UpdateStatus::UpToDate => ToastResult::NoUpdates,
        UpdateStatus::Available { remote_label, url, source } => {
            ToastResult::Available { label: remote_label, url, source }
        }
        UpdateStatus::Error(message) => ToastResult::Failed { message },
    }
}

fn toast_result_from_apply(outcome: ApplyUpdateOutcome) -> ToastResult {
    match outcome {
        ApplyUpdateOutcome::Success { binary_path, installed_ref } => {
            ToastResult::Installed { binary_path, installed_ref }
        }
        ApplyUpdateOutcome::Failed { message } => ToastResult::Failed { message },
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

/// Paint a Lucide-style info icon (circle + stem + top dot) without relying
/// on a font glyph. Matches:
/// `<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>`
fn paint_info_icon(ui: &mut egui::Ui, color: egui::Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let c = rect.center();
    // Scale from Lucide's 24×24 viewBox (r=10 → ~41.7% of half-size).
    let r = size * 0.42_f32;
    let stroke = egui::Stroke::new((size * 0.085_f32).clamp(1.25_f32, 2.0_f32), color);
    let painter = ui.painter();

    painter.circle_stroke(c, r, stroke);

    // Stem: M12 16v-4 → from ~2/3 down to center.
    let stem_top = egui::Pos2::new(c.x, c.y - r * 0.05_f32);
    let stem_bot = egui::Pos2::new(c.x, c.y + r * 0.45_f32);
    painter.line_segment([stem_top, stem_bot], stroke);

    // Top dot: M12 8h.01 — small filled circle near the top of the ring.
    let dot_y = c.y - r * 0.42_f32;
    let dot_r = (size * 0.07_f32).max(1.1_f32);
    painter.circle_filled(egui::Pos2::new(c.x, dot_y), dot_r, color);
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
    fn test_tick_toast_promotes_ready_outcome_after_min_display() {
        let (_tx, rx) = mpsc::channel();
        let mut help = HelpMenu::new();
        help.toast = Some(UpdateToast::Checking {
            started: Instant::now() - Duration::from_secs(2),
            rx,
            ready: Some(UpdateCheckOutcome { status: UpdateStatus::UpToDate }),
        });
        help.tick_toast();
        assert!(matches!(
            help.toast,
            Some(UpdateToast::Done { result: ToastResult::NoUpdates, .. })
        ));
    }

    #[test]
    fn test_tick_toast_dismisses_done_after_timeout() {
        let mut help = HelpMenu::new();
        help.toast = Some(UpdateToast::Done {
            result: ToastResult::NoUpdates,
            since: Instant::now() - Duration::from_secs(10),
        });
        help.tick_toast();
        assert!(help.toast.is_none());
    }

    #[test]
    fn test_toast_result_from_available() {
        let r = toast_result_from_outcome(UpdateCheckOutcome {
            status: UpdateStatus::Available {
                remote_label: "v9.9.9".into(),
                url: "https://example.com".into(),
                source: UpdateSource::Release,
            },
        });
        assert!(matches!(
            r,
            ToastResult::Available { label, source: UpdateSource::Release, .. }
                if label == "v9.9.9"
        ));
    }

    #[test]
    fn test_toast_result_from_apply_success() {
        let r = toast_result_from_apply(ApplyUpdateOutcome::Success {
            binary_path: "/tmp/rmux".into(),
            installed_ref: "main".into(),
        });
        assert!(matches!(
            r,
            ToastResult::Installed { installed_ref, .. } if installed_ref == "main"
        ));
    }

    #[test]
    fn test_install_success_opens_restart_prompt() {
        let (_tx, rx) = mpsc::channel();
        let mut help = HelpMenu::new();
        help.toast = Some(UpdateToast::Installing {
            started: Instant::now() - Duration::from_secs(1),
            label: "main".into(),
            rx,
            ready: Some(ApplyUpdateOutcome::Success {
                binary_path: "/tmp/rmux".into(),
                installed_ref: "abc1234".into(),
            }),
        });
        help.tick_toast();
        assert!(matches!(
            help.toast,
            Some(UpdateToast::Done { result: ToastResult::Installed { .. }, .. })
        ));
        assert_eq!(
            help.restart_prompt,
            Some(RestartPrompt {
                binary_path: "/tmp/rmux".into(),
                installed_ref: "abc1234".into(),
            })
        );
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
