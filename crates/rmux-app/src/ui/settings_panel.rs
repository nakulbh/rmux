//! Settings panel — floating window with app preferences.
//!
//! Currently holds the terminal color theme picker. Opened via the gear
//! icon in the **top-right** of the chrome bar. Anchored to the right edge
//! under the settings button so it never appears on the left.

use crate::ui::theme;
use rmux_terminal::NamedTheme;

/// Horizontal inset from the window's right edge.
const ANCHOR_RIGHT_PAD: f32 = 12.0_f32;
/// Vertical offset below the top bar (settings gear lives in the bar).
const ANCHOR_TOP_PAD: f32 = 8.0_f32;

/// The settings panel state and renderer.
#[derive(Debug, Default)]
pub struct SettingsPanel {
    /// Whether the panel is currently open.
    pub open: bool,
}

impl SettingsPanel {
    /// Create a new panel (closed by default).
    pub fn new() -> Self {
        Self::default()
    }

    /// Render the panel if open.
    ///
    /// Returns `Some(theme)` when the user picked a different theme this
    /// frame; the caller (`app.rs`) applies it to every terminal pane.
    pub fn show(&mut self, ctx: &egui::Context, current_theme: NamedTheme) -> Option<NamedTheme> {
        if !self.open {
            return None;
        }

        let palette = theme::palette();
        let mut open = self.open;
        let mut picked = current_theme;

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
            .default_width(260.0_f32)
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
            });

        self.open = open;
        (picked != current_theme).then_some(picked)
    }
}
