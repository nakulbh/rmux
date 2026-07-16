//! Theme system for rmux.
//!
//! Default palette matches **cmux's deep-black dark chrome**: near-black
//! surfaces, subtle elevated cards, and a single bright accent for active
//! states. Other named terminal themes derive the UI chrome from their
//! terminal colors via [`Palette::from_terminal`].
//!
//! Apply once per frame at the top of `update()`:
//! ```ignore
//! crate::ui::theme::Theme::dark().apply(ctx);
//! ```

use std::sync::{OnceLock, RwLock};

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};
use rmux_terminal::{NamedTheme, TerminalTheme};

/// Shorthand for an opaque sRGB color from 8-bit channels.
const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Linear-blend two colors: `t=0.0` returns `a`, `t=1.0` returns `b`.
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let lerp =
        |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(lerp(a.r(), b.r()), lerp(a.g(), b.g()), lerp(a.b(), b.b()))
}

/// Perceptual luminance (0..=255), used to pick readable text on a
/// colored fill (e.g. text atop the accent color).
fn luminance(c: Color32) -> f32 {
    0.299_f32 * c.r() as f32 + 0.587_f32 * c.g() as f32 + 0.114_f32 * c.b() as f32
}

/// Semantic color tokens (cmux deep-black default).
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    // --- Surfaces (darkest → lightest) ---
    /// Window root and gaps between panes. `#0c0c0e`
    pub app_bg: Color32,
    /// Center pane / terminal background. `#0c0c0e`
    pub terminal_bg: Color32,
    /// Left sidebar and right notification panel fill. `#141416`
    pub sidebar_bg: Color32,
    /// Cards, buttons, inputs, badges. `#1a1a1c`
    pub panel_bg: Color32,
    /// Hover + selected background everywhere. `#252528`
    pub panel_active_bg: Color32,
    /// Top bar, status bar, overlays. `#0f0f11`
    pub chrome_bg: Color32,
    /// Active tab fill (matches content bg). `#0c0c0e`
    pub tab_active_bg: Color32,

    // --- Lines ---
    /// All standard 1px borders/separators/dividers. `#2a2a2e`
    pub border: Color32,
    /// Hairline under top bar and above status bar. `#2a2a2e`
    pub chrome_border: Color32,

    // --- Text ---
    /// Primary text, active labels. `#e8e8ea`
    pub text_primary: Color32,
    /// Secondary text, inactive labels, icons. `#8b8b90`
    pub text_muted: Color32,
    /// Timestamps, placeholders, hints. `#5c5c62`
    pub text_disabled: Color32,

    // --- Accent + status ---
    /// Selection, active cards, badges — macOS-style blue like cmux. `#0a84ff`
    pub accent: Color32,
    /// Text on accent-filled elements. `#ffffff`
    pub accent_fg: Color32,
    /// Additions, success. `#6bc46d`
    pub success: Color32,
    /// Deletions, errors, exited processes. `#ff6b6b`
    pub danger: Color32,
    /// "Working" status, pending. `#e5c07b`
    pub warning: Color32,
    /// "Waiting"/attention ring blue. `#6cb3fa`
    pub info: Color32,

    // --- Terminal ---
    /// Terminal cursor overlay.
    pub terminal_cursor: Color32,
    /// Terminal selection background.
    pub terminal_selection_bg: Color32,
}

impl Palette {
    /// cmux-style deep black chrome (default app look).
    pub fn dark() -> Self {
        let app_bg = rgb(0x0c, 0x0c, 0x0e);
        let terminal_bg = rgb(0x0c, 0x0c, 0x0e);
        let sidebar_bg = rgb(0x14, 0x14, 0x16);
        let panel_bg = rgb(0x1a, 0x1a, 0x1c);
        let panel_active_bg = rgb(0x25, 0x25, 0x28);
        let chrome_bg = rgb(0x0f, 0x0f, 0x11);
        let chrome_border = rgb(0x2a, 0x2a, 0x2e);
        let border = rgb(0x2a, 0x2a, 0x2e);
        let text_primary = rgb(0xe8, 0xe8, 0xea);
        let text_muted = rgb(0x8b, 0x8b, 0x90);
        let text_disabled = rgb(0x5c, 0x5c, 0x62);
        let accent = rgb(0x0a, 0x84, 0xff);
        let accent_fg = rgb(0xff, 0xff, 0xff);
        let danger = rgb(0xff, 0x6b, 0x6b);

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
            success: rgb(0x6b, 0xc4, 0x6d),
            danger,
            warning: rgb(0xe5, 0xc0, 0x7b),
            info: rgb(0x6c, 0xb3, 0xfa),
            terminal_cursor: rgb(0xe6, 0xe6, 0xe8),
            terminal_selection_bg: rgb(0x2c, 0x2c, 0x32),
        }
    }

    /// Derive a full UI palette from a terminal color theme, so the whole
    /// app chrome (sidebar, top bar, status bar, cards) recolors along with
    /// the terminal when the user picks a theme in Settings — not just the
    /// terminal grid. Surfaces are stepped from `background` towards
    /// `foreground` by small amounts to keep the Arbor three-surface depth
    /// model (content / chrome / interaction) regardless of which theme's
    /// exact colors are plugged in.
    pub fn from_terminal(theme: &TerminalTheme) -> Self {
        let bg = theme.background;
        let fg = theme.foreground;
        let accent = theme.blue;
        let accent_fg = if luminance(accent) > 140.0_f32 { Color32::BLACK } else { Color32::WHITE };

        Self {
            app_bg: bg,
            terminal_bg: bg,
            sidebar_bg: mix(bg, fg, 0.06_f32),
            panel_bg: mix(bg, fg, 0.05_f32),
            panel_active_bg: mix(bg, fg, 0.10_f32),
            chrome_bg: mix(bg, fg, 0.14_f32),
            tab_active_bg: bg,
            border: mix(bg, fg, 0.10_f32),
            chrome_border: mix(bg, fg, 0.16_f32),
            text_primary: fg,
            text_muted: mix(fg, bg, 0.35_f32),
            text_disabled: mix(fg, bg, 0.55_f32),
            accent,
            accent_fg,
            success: theme.green,
            danger: theme.red,
            warning: theme.yellow,
            info: theme.cyan,
            terminal_cursor: theme.cursor,
            terminal_selection_bg: theme.selection_bg,
        }
    }
}

/// Process-wide selected theme, read by [`palette()`] every frame across
/// every UI module. Set via [`set_named_theme`] whenever the user picks a
/// theme in Settings (`RmuxApp::set_terminal_theme`).
static CURRENT_NAMED_THEME: OnceLock<RwLock<NamedTheme>> = OnceLock::new();

fn current_named_theme_lock() -> &'static RwLock<NamedTheme> {
    CURRENT_NAMED_THEME.get_or_init(|| RwLock::new(NamedTheme::default()))
}

/// Set the app-wide theme. Every subsequent [`palette()`] call — across
/// the sidebar, top bar, status bar, notification/settings panels, and
/// terminal chrome — reflects it starting next frame.
pub fn set_named_theme(named: NamedTheme) {
    *current_named_theme_lock().write().unwrap() = named;
}

/// The currently active named theme (default: One Dark).
pub fn current_named_theme() -> NamedTheme {
    *current_named_theme_lock().read().unwrap()
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
    /// The current app theme: dark chrome, radius 6, palette derived from
    /// whichever [`NamedTheme`] the user has selected (default: One Dark).
    pub fn dark() -> Self {
        Self { palette: palette(), radius: 6.0_f32, dark: true }
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

/// Convenience: get the palette for the currently selected theme. The
/// default (`OneDark` / "Dark") uses the hand-tuned cmux deep-black
/// chrome; every other theme derives UI colors from its terminal palette
/// via [`Palette::from_terminal`].
pub fn palette() -> Palette {
    match current_named_theme() {
        NamedTheme::OneDark => Palette::dark(),
        other => Palette::from_terminal(&TerminalTheme::default().named(other)),
    }
}

/// Corner radius for cards, buttons, badges, and inputs — the single
/// source of truth so these surfaces feel like one rounded-card system
/// (matching cmux) instead of a scatter of mismatched hardcoded radii.
pub fn radius_sm() -> u8 {
    Theme::dark().radius_sm() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mix_endpoints() {
        let a = rgb(0, 0, 0);
        let b = rgb(255, 255, 255);
        assert_eq!(mix(a, b, 0.0_f32), a);
        assert_eq!(mix(a, b, 1.0_f32), b);
    }

    #[test]
    fn test_luminance_orders_black_below_white() {
        assert!(luminance(rgb(0, 0, 0)) < luminance(rgb(255, 255, 255)));
    }

    #[test]
    fn test_from_terminal_distinguishes_surfaces_from_background() {
        for named in NamedTheme::all() {
            let terminal_theme = TerminalTheme::default().named(*named);
            let palette = Palette::from_terminal(&terminal_theme);
            assert_ne!(palette.app_bg, palette.text_primary);
            assert_ne!(palette.panel_bg, palette.chrome_bg, "{named:?} surfaces must differ");
            assert_ne!(palette.accent, palette.accent_fg, "{named:?} accent text must contrast");
        }
    }

    /// `set_named_theme`/`palette` share one process-wide static, and
    /// `cargo test` runs `#[test]` fns concurrently on separate threads —
    /// so anything exercising that global must live in a single test to
    /// avoid racing with another test's mutation between a set and its
    /// assertion. Always ends by resetting to the default (`OneDark`) so
    /// this doesn't leak state into whichever test runs next.
    #[test]
    fn test_set_named_theme_round_trips_and_drives_palette() {
        set_named_theme(NamedTheme::Dracula);
        assert_eq!(current_named_theme(), NamedTheme::Dracula);

        set_named_theme(NamedTheme::TokyoNight);
        let tokyo = palette();

        set_named_theme(NamedTheme::OneDark);
        let one_dark = palette();

        assert_eq!(current_named_theme(), NamedTheme::OneDark);
        assert_ne!(tokyo.app_bg, one_dark.app_bg);
        assert_eq!(one_dark.app_bg, Palette::dark().app_bg);
    }
}
