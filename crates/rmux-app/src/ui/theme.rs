//! Arbor/cmux-inspired theme system for rmux.
//!
//! Implements the Arbor "One Dark" palette (see `docs/UI_REDESIGN.md`):
//! a three-surface depth model (content / chrome / interaction) separated
//! by 1px borders, one accent color for all "active" states, and status
//! colors reserved strictly for semantics. Centralizes color tokens,
//! metrics, and typography so UI modules don't hardcode magic numbers.
//!
//! Apply once per frame at the top of `update()`:
//! ```ignore
//! crate::ui::theme::Theme::dark().apply(ctx);
//! ```

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};

/// Shorthand for an opaque sRGB color from 8-bit channels.
const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Semantic color tokens (Arbor One Dark).
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    // --- Surfaces (darkest → lightest) ---
    /// Window root and gaps between panes. `#282c33`
    pub app_bg: Color32,
    /// Center pane / terminal background. `#282c34`
    pub terminal_bg: Color32,
    /// Left sidebar and right notification panel fill. `#2f343e`
    pub sidebar_bg: Color32,
    /// Cards, buttons, inputs, badges. `#2e343e`
    pub panel_bg: Color32,
    /// Hover + selected background everywhere. `#363c46`
    pub panel_active_bg: Color32,
    /// Top bar, status bar, overlays. `#3b414d`
    pub chrome_bg: Color32,
    /// Active tab fill (matches content bg). `#282c33`
    pub tab_active_bg: Color32,

    // --- Lines ---
    /// All standard 1px borders/separators/dividers. `#363c46`
    pub border: Color32,
    /// Hairline under top bar and above status bar. `#464b57`
    pub chrome_border: Color32,

    // --- Text ---
    /// Primary text, active labels. `#c8ccd4`
    pub text_primary: Color32,
    /// Secondary text, inactive labels, icons. `#838994`
    pub text_muted: Color32,
    /// Timestamps, placeholders, hints. `#696b77`
    pub text_disabled: Color32,

    // --- Accent + status ---
    /// Selection borders, focus, caret, badges, progress. `#74ade8`
    pub accent: Color32,
    /// Text on accent-filled elements. `#1d2127`
    pub accent_fg: Color32,
    /// Additions, success. `#72d69c`
    pub success: Color32,
    /// Deletions, errors, exited processes. `#eb6f92`
    pub danger: Color32,
    /// "Working" status, pending. `#e5c07b`
    pub warning: Color32,
    /// "Waiting"/attention ring blue. `#61afef`
    pub info: Color32,

    // --- Terminal ---
    /// Terminal cursor overlay. `#ebdbb2`
    pub terminal_cursor: Color32,
    /// Terminal selection background. `#3e4451`
    pub terminal_selection_bg: Color32,
}

impl Palette {
    /// Arbor One Dark (see `docs/UI_REDESIGN.md` for the token table).
    pub fn dark() -> Self {
        let app_bg = rgb(0x28, 0x2c, 0x33);
        let terminal_bg = rgb(0x28, 0x2c, 0x34);
        let sidebar_bg = rgb(0x2f, 0x34, 0x3e);
        let panel_bg = rgb(0x2e, 0x34, 0x3e);
        let panel_active_bg = rgb(0x36, 0x3c, 0x46);
        let chrome_bg = rgb(0x3b, 0x41, 0x4d);
        let chrome_border = rgb(0x46, 0x4b, 0x57);
        let border = rgb(0x36, 0x3c, 0x46);
        let text_primary = rgb(0xc8, 0xcc, 0xd4);
        let text_muted = rgb(0x83, 0x89, 0x94);
        let text_disabled = rgb(0x69, 0x6b, 0x77);
        let accent = rgb(0x74, 0xad, 0xe8);
        let accent_fg = rgb(0x1d, 0x21, 0x27);
        let danger = rgb(0xeb, 0x6f, 0x92);

        Self {
            app_bg,
            terminal_bg,
            sidebar_bg,
            panel_bg,
            panel_active_bg,
            chrome_bg,
            tab_active_bg: app_bg,
            border,
            chrome_border,
            text_primary,
            text_muted,
            text_disabled,
            accent,
            accent_fg,
            success: rgb(0x72, 0xd6, 0x9c),
            danger,
            warning: rgb(0xe5, 0xc0, 0x7b),
            info: rgb(0x61, 0xaf, 0xef),
            terminal_cursor: rgb(0xeb, 0xdb, 0xb2),
            terminal_selection_bg: rgb(0x3e, 0x44, 0x51),
        }
    }
}

/// UI metrics shared across modules (see `docs/UI_REDESIGN.md`).
#[allow(dead_code)]
pub mod metrics {
    /// Top chrome bar height.
    pub const TOP_BAR_HEIGHT: f32 = 34.0;
    /// Bottom status bar height.
    pub const STATUS_BAR_HEIGHT: f32 = 26.0;
    /// Sidebar default width.
    pub const SIDEBAR_DEFAULT_WIDTH: f32 = 240.0;
    /// Sidebar min width.
    pub const SIDEBAR_MIN_WIDTH: f32 = 200.0;
    /// Sidebar max width.
    pub const SIDEBAR_MAX_WIDTH: f32 = 320.0;
    /// Standard button height.
    pub const BUTTON_HEIGHT: f32 = 24.0;
    /// Standard input height.
    pub const INPUT_HEIGHT: f32 = 28.0;
}

/// Complete theme: palette + radius + dark flag.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub palette: Palette,
    pub radius: f32,
    pub dark: bool,
}

impl Theme {
    /// Arbor One Dark theme.
    pub fn dark() -> Self {
        Self { palette: Palette::dark(), radius: 2.0_f32, dark: true }
    }

    /// Small radius — rows, buttons, inputs, cards, tabs.
    #[allow(dead_code)]
    pub fn radius_sm(&self) -> f32 {
        self.radius
    }

    /// Medium radius — popovers, overlays, zoom indicator.
    #[allow(dead_code)]
    pub fn radius_md(&self) -> f32 {
        6.0_f32
    }

    /// Large radius — reserved for modals.
    #[allow(dead_code)]
    pub fn radius_lg(&self) -> f32 {
        8.0_f32
    }

    /// Push theme into the egui context: fonts, type scale, spacing, colors.
    ///
    /// Safe to call once per frame — only updates style/visuals, which egui
    /// diffs internally.
    pub fn apply(&self, ctx: &egui::Context) {
        let p = &self.palette;

        ctx.style_mut(|s| {
            // Typography scale: 12px dominant, 14px headings (dense chrome)
            s.text_styles = [
                (TextStyle::Small, FontId::new(10.0_f32, FontFamily::Proportional)),
                (TextStyle::Body, FontId::new(12.0_f32, FontFamily::Proportional)),
                (TextStyle::Button, FontId::new(12.0_f32, FontFamily::Proportional)),
                (TextStyle::Monospace, FontId::new(12.0_f32, FontFamily::Monospace)),
                (TextStyle::Heading, FontId::new(14.0_f32, FontFamily::Proportional)),
            ]
            .into();

            // 4px spacing grid, 24px control rhythm
            s.spacing.item_spacing = egui::vec2(4.0_f32, 4.0_f32);
            s.spacing.button_padding = egui::vec2(8.0_f32, 4.0_f32);
            s.spacing.window_margin = egui::Margin::same(4);
            s.spacing.interact_size.y = 24.0_f32;

            // Visuals: three-surface depth model, zero shadows
            let v = &mut s.visuals;
            v.dark_mode = self.dark;
            v.panel_fill = p.app_bg;
            v.window_fill = p.panel_bg;
            v.window_stroke = Stroke::new(1.0_f32, p.border);
            v.extreme_bg_color = p.panel_bg;
            v.faint_bg_color = p.panel_active_bg;
            v.override_text_color = Some(p.text_primary);
            v.hyperlink_color = p.accent;
            v.selection.bg_fill = p.accent.gamma_multiply(0.35_f32);
            v.selection.stroke = Stroke::new(1.0_f32, p.accent);
            v.window_shadow = egui::Shadow::NONE;
            v.popup_shadow = egui::Shadow::NONE;

            // Widget visuals: flat panels, 1px borders, 2px radii
            v.widgets.noninteractive.bg_fill = p.app_bg;
            v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
            v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
            for w in [
                &mut v.widgets.inactive,
                &mut v.widgets.hovered,
                &mut v.widgets.active,
                &mut v.widgets.open,
            ] {
                w.bg_fill = p.panel_bg;
                w.weak_bg_fill = p.panel_bg;
                w.bg_stroke = Stroke::new(1.0_f32, p.border);
                w.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
                w.corner_radius = CornerRadius::same(self.radius_sm() as u8);
            }
            // Interaction surface for hover/active
            for w in [&mut v.widgets.hovered, &mut v.widgets.active] {
                w.bg_fill = p.panel_active_bg;
                w.weak_bg_fill = p.panel_active_bg;
            }
        });
    }

    /// Get the active theme from the egui context (fallback: dark).
    #[allow(dead_code)]
    pub fn current(ctx: &egui::Context) -> Theme {
        // Just return dark for now; can be extended with context storage
        let _ = ctx;
        Theme::dark()
    }
}

/// Convenience: get the dark palette.
pub fn palette() -> Palette {
    Palette::dark()
}
