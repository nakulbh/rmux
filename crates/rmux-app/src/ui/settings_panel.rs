//! Settings panel — floating window with app preferences.
//!
//! Holds the terminal color theme picker and workspace wallpaper controls
//! (background image path + terminal opacity). Opened via the gear icon
//! in the **top-right** of the chrome bar.

use crate::ui::theme;
use rmux_config::AppearanceConfig;
use rmux_terminal::NamedTheme;

/// Horizontal inset from the window's right edge.
const ANCHOR_RIGHT_PAD: f32 = 12.0_f32;
/// Vertical offset below the top bar (settings gear lives in the bar).
const ANCHOR_TOP_PAD: f32 = 8.0_f32;

/// Changes requested by the settings panel this frame.
#[derive(Debug, Default, Clone)]
pub struct SettingsChanges {
    /// New theme when the user picked a different one.
    pub theme: Option<NamedTheme>,
    /// Updated appearance when wallpaper / opacity was edited.
    pub appearance: Option<AppearanceConfig>,
    /// User asked to re-load the wallpaper texture from the current path.
    pub reload_wallpaper: bool,
}

/// The settings panel state and renderer.
#[derive(Debug, Default)]
pub struct SettingsPanel {
    /// Whether the panel is currently open.
    pub open: bool,
    /// Draft path string shown in the text field (may differ from saved config
    /// until the user applies / blurs).
    draft_image_path: String,
    /// Whether `draft_image_path` has been seeded from config this open session.
    draft_synced: bool,
}

impl SettingsPanel {
    /// Create a new panel (closed by default).
    pub fn new() -> Self {
        Self::default()
    }

    /// Render the panel if open.
    ///
    /// Returns changes the caller (`app.rs`) should apply and persist.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        current_theme: NamedTheme,
        appearance: &AppearanceConfig,
        wallpaper_status: Option<&str>,
    ) -> SettingsChanges {
        let mut changes = SettingsChanges::default();
        if !self.open {
            self.draft_synced = false;
            return changes;
        }

        // Seed the draft path when the panel opens.
        if !self.draft_synced {
            self.draft_image_path = appearance.background_image.clone().unwrap_or_default();
            self.draft_synced = true;
        }

        let palette = theme::palette();
        let mut open = self.open;
        let mut picked = current_theme;
        let mut enabled = appearance.background_enabled;
        let mut opacity = appearance.clamped_opacity();
        let mut sidebar_opacity = appearance.clamped_sidebar_opacity();
        let mut transparent_sidebar = sidebar_opacity < 0.999;
        let mut browse_clicked = false;
        let mut clear_clicked = false;
        let mut apply_clicked = false;

        // Position under the top-right settings control — not the default
        // center/left placement egui uses for unnamed windows.
        let top_offset = theme::metrics::TOP_BAR_HEIGHT + ANCHOR_TOP_PAD;

        egui::Window::new("Settings")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .movable(false)
            // Pin under the top-right gear; do not use default left/center placement.
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-ANCHOR_RIGHT_PAD, top_offset))
            .default_width(300.0_f32)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(palette.panel_bg)
                    .stroke(egui::Stroke::new(1.0_f32, palette.border))
                    .corner_radius(egui::CornerRadius::same(10))
                    .shadow(egui::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(120),
                    }),
            )
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new("Terminal Theme").color(palette.text_muted).size(11.0_f32),
                );
                ui.add_space(6.0_f32);
                for named in NamedTheme::all() {
                    ui.selectable_value(&mut picked, *named, named.label());
                }

                ui.add_space(14.0_f32);
                ui.separator();
                ui.add_space(10.0_f32);

                ui.label(
                    egui::RichText::new("Workspace Background")
                        .color(palette.text_muted)
                        .size(11.0_f32),
                );
                ui.add_space(4.0_f32);
                ui.label(
                    egui::RichText::new(
                        "One image behind every terminal — consistent across splits and agents.",
                    )
                    .color(palette.text_disabled)
                    .size(10.5_f32),
                );
                ui.add_space(8.0_f32);

                ui.checkbox(&mut enabled, "Enable background image");
                ui.add_space(6.0_f32);

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Image").color(palette.text_muted).size(11.0_f32));
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.draft_image_path)
                            .desired_width(160.0_f32)
                            .hint_text("~/Pictures/wallpaper.jpg"),
                    );
                    // Enter applies the path; live typing only updates the draft field.
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        apply_clicked = true;
                    }
                });

                ui.horizontal(|ui| {
                    if ui.button("Browse…").clicked() {
                        browse_clicked = true;
                    }
                    if ui.button("Apply").clicked() {
                        apply_clicked = true;
                    }
                    if ui.button("Clear").clicked() {
                        clear_clicked = true;
                    }
                });

                if let Some(status) = wallpaper_status {
                    ui.add_space(4.0_f32);
                    ui.label(
                        egui::RichText::new(status).color(palette.danger).size(10.5_f32),
                    );
                }

                ui.add_space(8.0_f32);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Terminal opacity")
                            .color(palette.text_muted)
                            .size(11.0_f32),
                    );
                    ui.add(
                        // Floor 0.25 so the UI never becomes pure wallpaper.
                        egui::Slider::new(&mut opacity, 0.25..=1.0)
                            .show_value(true)
                            .min_decimals(2)
                            .max_decimals(2),
                    );
                });
                ui.label(
                    egui::RichText::new("Lower = more image visible through terminals (min 0.25).")
                        .color(palette.text_disabled)
                        .size(10.5_f32),
                );

                ui.add_space(12.0_f32);
                ui.checkbox(&mut transparent_sidebar, "Transparent sidebar");
                ui.label(
                    egui::RichText::new(
                        "Glass left panel so the wallpaper shows through (workspace cards stay solid).",
                    )
                    .color(palette.text_disabled)
                    .size(10.5_f32),
                );
                if transparent_sidebar {
                    ui.add_space(6.0_f32);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Sidebar opacity")
                                .color(palette.text_muted)
                                .size(11.0_f32),
                        );
                        ui.add(
                            egui::Slider::new(&mut sidebar_opacity, 0.15..=0.95)
                                .show_value(true)
                                .min_decimals(2)
                                .max_decimals(2),
                        );
                    });
                }
            });

        self.open = open;

        // Native file dialog (blocks briefly; only on Browse click).
        if browse_clicked
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif"])
                .set_title("Choose workspace background")
                .pick_file()
        {
            self.draft_image_path = path.display().to_string();
            apply_clicked = true;
        }

        if clear_clicked {
            self.draft_image_path.clear();
            apply_clicked = true;
        }

        // Map the transparent-sidebar toggle to a concrete opacity.
        // When turning on without a prior translucent value, use a glass default.
        let resolved_sidebar_opacity = if transparent_sidebar {
            if sidebar_opacity >= 0.999 { 0.55 } else { sidebar_opacity.clamp(0.15, 0.95) }
        } else {
            1.0
        };

        let mut new_appearance = appearance.clone();
        let mut appearance_changed = false;

        if enabled != appearance.background_enabled {
            new_appearance.background_enabled = enabled;
            appearance_changed = true;
        }
        if (opacity - appearance.clamped_opacity()).abs() > f32::EPSILON {
            new_appearance.background_opacity = opacity;
            appearance_changed = true;
        }
        if (resolved_sidebar_opacity - appearance.clamped_sidebar_opacity()).abs() > f32::EPSILON {
            new_appearance.sidebar_opacity = resolved_sidebar_opacity;
            appearance_changed = true;
        }
        if apply_clicked {
            let trimmed = self.draft_image_path.trim();
            let next = if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) };
            if next != appearance.background_image {
                new_appearance.background_image = next;
                appearance_changed = true;
            }
            changes.reload_wallpaper = true;
        }

        if appearance_changed {
            changes.appearance = Some(new_appearance);
        }

        if picked != current_theme {
            changes.theme = Some(picked);
        }

        changes
    }
}
