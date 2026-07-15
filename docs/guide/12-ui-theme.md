# 12. UI theme

Theme = color tokens plus egui style. Widgets ask theme for named colors.

File: `crates/rmux-app/src/ui/theme.rs`.

Top comment:

```rust
//! Arbor/cmux-inspired theme system for rmux.
//!
//! Implements the Arbor "One Dark" palette (see `docs/UI_REDESIGN.md`):
//! a three-surface depth model (content / chrome / interaction) separated
//! by 1px borders, one accent color for all "active" states, and status
//! colors reserved strictly for semantics. Centralizes color tokens,
//! metrics, and typography so UI modules don't hardcode magic numbers.
```

## Color helper

```rust
/// Shorthand for an opaque sRGB color from 8-bit channels.
const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}
```

## Palette struct

`Palette` names colors by job, not look. Good: `panel_bg`. Bad: `dark_gray_2`.

```rust
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
```

Text tokens:

```rust
/// Primary text, active labels. `#c8ccd4`
pub text_primary: Color32,
/// Secondary text, inactive labels, icons. `#838994`
pub text_muted: Color32,
/// Timestamps, placeholders, hints. `#696b77`
pub text_disabled: Color32,
```

Status tokens:

```rust
/// Additions, success. `#72d69c`
pub success: Color32,
/// Deletions, errors, exited processes. `#eb6f92`
pub danger: Color32,
/// "Working" status, pending. `#e5c07b`
pub warning: Color32,
/// "Waiting"/attention ring blue. `#61afef`
pub info: Color32,
```

## Default dark palette

`Palette::dark()` sets One Dark values.

```rust
pub fn dark() -> Self {
    let app_bg = rgb(0x28, 0x2c, 0x33);
    let terminal_bg = rgb(0x28, 0x2c, 0x34);
    let sidebar_bg = rgb(0x2f, 0x34, 0x3e);
    let panel_bg = rgb(0x2e, 0x34, 0x3e);
    let panel_active_bg = rgb(0x36, 0x3c, 0x46);
    let chrome_bg = rgb(0x3b, 0x41, 0x4d);
    let chrome_border = rgb(0x46, 0x4b, 0x57);
    let border = rgb(0x36, 0x3c, 0x46);
```

## Terminal theme drives chrome

When user picks terminal theme, sidebar and panels recolor too.

```rust
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
```

`mix(bg, fg, 0.06)` = slightly lighter than background.

## Current theme storage

One process-wide selected theme.

```rust
static CURRENT_NAMED_THEME: OnceLock<RwLock<NamedTheme>> = OnceLock::new();

fn current_named_theme_lock() -> &'static RwLock<NamedTheme> {
    CURRENT_NAMED_THEME.get_or_init(|| RwLock::new(NamedTheme::default()))
}
```

Set by app:

```rust
pub fn set_named_theme(named: NamedTheme) {
    *current_named_theme_lock().write().unwrap() = named;
}
```

UI modules call `theme::palette()`.

## Metrics

Sizes live beside colors. No magic numbers everywhere.

```rust
pub mod metrics {
    /// Top chrome bar height.
    pub const TOP_BAR_HEIGHT: f32 = 34.0;
    /// Bottom status bar height.
    pub const STATUS_BAR_HEIGHT: f32 = 26.0;
    /// Sidebar default width.
    pub const SIDEBAR_DEFAULT_WIDTH: f32 = 240.0;
```

## Theme::apply

`Theme::apply(ctx)` pushes style into egui each frame.

```rust
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
```

Visuals:

```rust
let v = &mut s.visuals;
v.dark_mode = self.dark;
v.panel_fill = p.app_bg;
v.window_fill = p.panel_bg;
v.window_stroke = Stroke::new(1.0_f32, p.border);
v.override_text_color = Some(p.text_primary);
v.hyperlink_color = p.accent;
```

Flow:

```text
NamedTheme -> TerminalTheme -> Palette -> Theme::apply(ctx) -> egui widgets
```

← **Prev: [11 — App State](11-app-state.md)**

→ **Next: [13 — UI Topbar Sidebar](13-ui-topbar-sidebar.md)**
