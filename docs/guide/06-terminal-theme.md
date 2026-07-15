# 06. Terminal theme

Theme maps terminal colors to pixels.

Terminal apps ask for ANSI colors.

rmux resolves them to `egui::Color32`.

File: `crates/rmux-terminal/src/theme.rs`.

Tiny helper:

```rust
use egui::Color32;

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}
```

Why helper?

Theme definitions stay compact.

No repeated `Color32::from_rgb(...)` noise.

Theme names:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum NamedTheme {
    #[default]
    OneDark,
    Dracula,
    SolarizedDark,
    SolarizedLight,
    GruvboxDark,
    CatppuccinMocha,
    TokyoNight,
}
```

Why enum?

Known fixed choices.

Compiler checks every match handles every theme.

All choices for UI picker:

```rust
pub fn all() -> &'static [NamedTheme] {
    &[
        NamedTheme::OneDark,
        NamedTheme::Dracula,
        NamedTheme::SolarizedDark,
        NamedTheme::SolarizedLight,
        NamedTheme::GruvboxDark,
        NamedTheme::CatppuccinMocha,
        NamedTheme::TokyoNight,
    ]
}
```

Why static slice?

No allocation.

UI can iterate themes every frame cheaply.

Human labels:

```rust
pub fn label(&self) -> &'static str {
    match self {
        NamedTheme::OneDark => "One Dark",
        NamedTheme::Dracula => "Dracula",
        NamedTheme::SolarizedDark => "Solarized Dark",
        NamedTheme::SolarizedLight => "Solarized Light",
        NamedTheme::GruvboxDark => "Gruvbox Dark",
        NamedTheme::CatppuccinMocha => "Catppuccin Mocha",
        NamedTheme::TokyoNight => "Tokyo Night",
    }
}
```

Why not use debug name?

`CatppuccinMocha` is Rust name.

`Catppuccin Mocha` is UI text.

Color palette struct:

```rust
pub struct TerminalTheme {
    pub black: Color32,
    pub red: Color32,
    pub green: Color32,
    pub yellow: Color32,
    pub blue: Color32,
    pub magenta: Color32,
    pub cyan: Color32,
    pub white: Color32,
    pub bright_black: Color32,
    pub bright_red: Color32,
    pub bright_green: Color32,
    pub bright_yellow: Color32,
    pub bright_blue: Color32,
    pub bright_magenta: Color32,
    pub bright_cyan: Color32,
    pub bright_white: Color32,
    pub foreground: Color32,
    pub background: Color32,
    pub cursor: Color32,
    pub selection_bg: Color32,
}
```

Terminal apps use 16 ANSI colors plus defaults.

`foreground` and `background` cover normal text.

`cursor` and `selection_bg` cover UI overlays.

Named theme conversion:

```rust
pub fn named(&self, name: NamedTheme) -> Self {
    match name {
        NamedTheme::OneDark => Self::one_dark(),
        NamedTheme::Dracula => Self::dracula(),
        NamedTheme::SolarizedDark => Self::solarized_dark(),
        NamedTheme::SolarizedLight => Self::solarized_light(),
        NamedTheme::GruvboxDark => Self::gruvbox_dark(),
        NamedTheme::CatppuccinMocha => Self::catppuccin_mocha(),
        NamedTheme::TokyoNight => Self::tokyo_night(),
    }
}
```

Why return `Self`?

Theme is plain data.

Copy palette into terminal state.

Renderer uses resolved colors from snapshot.

Flow:

```text
NamedTheme -> TerminalTheme -> TermState -> GridSnapshot -> TerminalRenderer
```

[Prev: Terminal renderer](05-terminal-renderer.md) | [Next: OSC notifications](07-osc-notifications.md)
