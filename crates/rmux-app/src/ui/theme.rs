//! shadcn-inspired theme system for rmux.
//!
//! Implements the shadcn v4 (new-york / OKLCH) dark palette, adapted for egui.
//! Centralizes all color constants, spacing, and typography so UI modules
//! don't hardcode magic numbers.
//!
//! Apply once per frame at the top of `update()`:
//! ```ignore
//! crate::ui::theme::Theme::dark().apply(ctx);
//! ```

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};

/// Convert OKLCH(L, C, H) to sRGB `Color32`.
///
/// `l` in 0..=1, `c` chroma (~0..0.4), `h` hue in degrees.
/// Out-of-gamut channels are clamped.
fn oklch(l: f32, c: f32, h: f32) -> Color32 {
    oklch_a(l, c, h, 1.0)
}

/// OKLCH with explicit alpha.
fn oklch_a(l: f32, c: f32, h: f32, alpha: f32) -> Color32 {
    let h_rad = h.to_radians();
    let a = c * h_rad.cos();
    let b = c * h_rad.sin();

    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_35 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (lc, mc, sc) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);

    let r = 4.076_741_7 * lc - 3.307_711_6 * mc + 0.230_969_93 * sc;
    let g = -1.268_438 * lc + 2.609_757_4 * mc - 0.341_319_4 * sc;
    let bl = -0.004_196_09 * lc - 0.703_418_6 * mc + 1.707_614_7 * sc;

    let to_u8 = |lin: f32| -> u8 {
        let lin = lin.clamp(0.0, 1.0);
        let srgb =
            if lin <= 0.003_130_8 { 12.92 * lin } else { 1.055 * lin.powf(1.0 / 2.4) - 0.055 };
        (srgb * 255.0).round().clamp(0.0, 255.0) as u8
    };

    Color32::from_rgba_unmultiplied(
        to_u8(r),
        to_u8(g),
        to_u8(bl),
        (alpha * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

/// Semantic shadcn color tokens.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub background: Color32,
    pub foreground: Color32,
    pub card: Color32,
    pub card_foreground: Color32,
    pub popover: Color32,
    pub popover_foreground: Color32,
    pub primary: Color32,
    pub primary_foreground: Color32,
    pub secondary: Color32,
    pub secondary_foreground: Color32,
    pub muted: Color32,
    pub muted_foreground: Color32,
    pub accent: Color32,
    pub accent_foreground: Color32,
    pub destructive: Color32,
    pub destructive_foreground: Color32,
    pub border: Color32,
    pub input: Color32,
    pub ring: Color32,
}

impl Palette {
    /// shadcn dark theme (new-york, OKLCH).
    pub fn dark() -> Self {
        Self {
            background: oklch(0.145, 0.0, 0.0),
            foreground: oklch(0.985, 0.0, 0.0),
            card: oklch(0.205, 0.0, 0.0),
            card_foreground: oklch(0.985, 0.0, 0.0),
            popover: oklch(0.205, 0.0, 0.0),
            popover_foreground: oklch(0.985, 0.0, 0.0),
            primary: oklch(0.922, 0.0, 0.0),
            primary_foreground: oklch(0.205, 0.0, 0.0),
            secondary: oklch(0.269, 0.0, 0.0),
            secondary_foreground: oklch(0.985, 0.0, 0.0),
            muted: oklch(0.269, 0.0, 0.0),
            muted_foreground: oklch(0.708, 0.0, 0.0),
            accent: oklch(0.269, 0.0, 0.0),
            accent_foreground: oklch(0.985, 0.0, 0.0),
            destructive: oklch(0.704, 0.191, 22.216),
            destructive_foreground: oklch(0.985, 0.0, 0.0),
            border: oklch_a(1.0, 0.0, 0.0, 0.10),
            input: oklch_a(1.0, 0.0, 0.0, 0.15),
            ring: oklch(0.556, 0.0, 0.0),
        }
    }
}

/// Complete theme: palette + radius + dark flag.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub palette: Palette,
    pub radius: f32,
    pub dark: bool,
}

impl Theme {
    /// shadcn dark theme.
    pub fn dark() -> Self {
        Self { palette: Palette::dark(), radius: 10.0, dark: true }
    }

    /// Small radius (buttons, inputs).
    #[allow(dead_code)]
    pub fn radius_sm(&self) -> f32 {
        (self.radius - 4.0).max(0.0)
    }

    /// Medium radius (default for most components).
    pub fn radius_md(&self) -> f32 {
        (self.radius - 2.0).max(0.0)
    }

    /// Large radius (cards).
    #[allow(dead_code)]
    pub fn radius_lg(&self) -> f32 {
        self.radius
    }

    /// Push theme into the egui context: fonts, type scale, spacing, colors.
    ///
    /// Safe to call once per frame — only updates style/visuals, which egui
    /// diffs internally.
    pub fn apply(&self, ctx: &egui::Context) {
        let p = &self.palette;

        ctx.style_mut(|s| {
            // Typography scale: 14px body, 12px small, 20px heading
            s.text_styles = [
                (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
                (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
                (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
                (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
                (TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional)),
            ]
            .into();

            // 4px spacing grid
            s.spacing.item_spacing = egui::vec2(8.0, 8.0);
            s.spacing.button_padding = egui::vec2(16.0, 8.0);
            s.spacing.window_margin = egui::Margin::same(4);
            s.spacing.interact_size.y = 36.0;

            // Visuals
            let v = &mut s.visuals;
            v.dark_mode = self.dark;
            v.panel_fill = p.background;
            v.window_fill = p.card;
            v.window_stroke = Stroke::new(1.0, p.border);
            v.extreme_bg_color = p.input;
            v.faint_bg_color = p.muted;
            v.override_text_color = Some(p.foreground);
            v.hyperlink_color = p.primary;
            v.selection.bg_fill = p.primary.gamma_multiply(0.35);
            v.selection.stroke = Stroke::new(1.0, p.ring);

            // Widget visuals
            v.widgets.noninteractive.bg_fill = p.background;
            v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
            v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.foreground);
            for w in [
                &mut v.widgets.inactive,
                &mut v.widgets.hovered,
                &mut v.widgets.active,
                &mut v.widgets.open,
            ] {
                w.bg_fill = p.secondary;
                w.weak_bg_fill = p.secondary;
                w.bg_stroke = Stroke::new(1.0, p.border);
                w.fg_stroke = Stroke::new(1.0, p.foreground);
                w.corner_radius = CornerRadius::same(self.radius_md() as u8);
            }
            // Accent for hover/active
            for w in [&mut v.widgets.hovered, &mut v.widgets.active] {
                w.bg_fill = p.accent;
                w.weak_bg_fill = p.accent;
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
