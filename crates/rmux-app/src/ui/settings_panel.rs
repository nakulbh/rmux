//! Settings panel — floating window with app preferences.
//!
//! Currently holds the terminal color theme picker. Opened via the gear
//! icon in the top bar. Renders as a movable, closable `egui::Window`
//! rather than a side panel since it's a one-off dialog, not a persistent
//! view like the sidebar or notification panel.

use crate::ui::theme;
use rmux_terminal::NamedTheme;

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

        egui::Window::new("Settings")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(240.0_f32)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(palette.panel_bg)
                    .stroke(egui::Stroke::new(1.0_f32, palette.border)),
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
