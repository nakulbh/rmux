use egui::Color32;

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

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

impl NamedTheme {
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

    /// Human-readable name for UI display (e.g. the settings theme picker).
    pub fn label(&self) -> &'static str {
        match self {
            NamedTheme::OneDark => "Dark",
            NamedTheme::Dracula => "Dracula",
            NamedTheme::SolarizedDark => "Solarized Dark",
            NamedTheme::SolarizedLight => "Solarized Light",
            NamedTheme::GruvboxDark => "Gruvbox Dark",
            NamedTheme::CatppuccinMocha => "Catppuccin Mocha",
            NamedTheme::TokyoNight => "Tokyo Night",
        }
    }
}

#[derive(Clone, Copy, Debug)]
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

impl TerminalTheme {
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

    /// cmux / Ghostty-inspired deep black (app default).
    ///
    /// Near-black background with high-contrast text and saturated ANSI
    /// colors so the shell and prompt read cleanly on pure dark chrome.
    pub fn one_dark() -> Self {
        Self {
            black: rgb(0x1c, 0x1c, 0x1e),
            red: rgb(0xff, 0x6b, 0x6b),
            green: rgb(0x6b, 0xc4, 0x6d),
            yellow: rgb(0xe5, 0xc0, 0x7b),
            blue: rgb(0x6c, 0xb3, 0xfa),
            magenta: rgb(0xc6, 0x8a, 0xee),
            cyan: rgb(0x56, 0xcd, 0xd8),
            white: rgb(0xe0, 0xe0, 0xe0),
            bright_black: rgb(0x5a, 0x5a, 0x5e),
            bright_red: rgb(0xff, 0x8a, 0x8a),
            bright_green: rgb(0x8a, 0xd8, 0x8c),
            bright_yellow: rgb(0xf0, 0xd0, 0x90),
            bright_blue: rgb(0x8c, 0xc4, 0xff),
            bright_magenta: rgb(0xd8, 0xa8, 0xf5),
            bright_cyan: rgb(0x7a, 0xe0, 0xe8),
            bright_white: rgb(0xff, 0xff, 0xff),
            foreground: rgb(0xe6, 0xe6, 0xe8),
            background: rgb(0x0c, 0x0c, 0x0e),
            cursor: rgb(0xe6, 0xe6, 0xe8),
            selection_bg: rgb(0x2c, 0x2c, 0x32),
        }
    }

    pub fn dracula() -> Self {
        Self {
            black: rgb(0x21, 0x22, 0x2C),
            red: rgb(0xFF, 0x55, 0x55),
            green: rgb(0x50, 0xFA, 0x7B),
            yellow: rgb(0xF1, 0xFA, 0x8C),
            blue: rgb(0xBD, 0x93, 0xF9),
            magenta: rgb(0xFF, 0x79, 0xC6),
            cyan: rgb(0x8B, 0xE9, 0xFD),
            white: rgb(0xF8, 0xF8, 0xF2),
            bright_black: rgb(0x62, 0x72, 0xA4),
            bright_red: rgb(0xFF, 0x6E, 0x6E),
            bright_green: rgb(0x69, 0xFF, 0x94),
            bright_yellow: rgb(0xFF, 0xFF, 0xA5),
            bright_blue: rgb(0xD6, 0xAC, 0xFF),
            bright_magenta: rgb(0xFF, 0x92, 0xDF),
            bright_cyan: rgb(0xA4, 0xFF, 0xFF),
            bright_white: rgb(0xFF, 0xFF, 0xFF),
            foreground: rgb(0xF8, 0xF8, 0xF2),
            background: rgb(0x28, 0x2A, 0x36),
            cursor: rgb(0xF8, 0xF8, 0xF2),
            selection_bg: rgb(0x44, 0x47, 0x5A),
        }
    }

    pub fn solarized_dark() -> Self {
        Self {
            black: rgb(0x07, 0x36, 0x42),
            red: rgb(0xDC, 0x32, 0x2F),
            green: rgb(0x85, 0x99, 0x00),
            yellow: rgb(0xB5, 0x89, 0x00),
            blue: rgb(0x26, 0x8B, 0xD2),
            magenta: rgb(0xD3, 0x36, 0x82),
            cyan: rgb(0x2A, 0xA1, 0x98),
            white: rgb(0xEE, 0xE8, 0xD5),
            bright_black: rgb(0x00, 0x2B, 0x36),
            bright_red: rgb(0xCB, 0x4B, 0x16),
            bright_green: rgb(0x58, 0x6E, 0x75),
            bright_yellow: rgb(0x65, 0x7B, 0x83),
            bright_blue: rgb(0x83, 0x94, 0x96),
            bright_magenta: rgb(0x6C, 0x71, 0xC4),
            bright_cyan: rgb(0x93, 0xA1, 0xA1),
            bright_white: rgb(0xFD, 0xF6, 0xE3),
            foreground: rgb(0x83, 0x94, 0x96),
            background: rgb(0x00, 0x2B, 0x36),
            cursor: rgb(0x83, 0x94, 0x96),
            selection_bg: rgb(0x07, 0x36, 0x42),
        }
    }

    pub fn solarized_light() -> Self {
        Self {
            black: rgb(0xEE, 0xE8, 0xD5),
            red: rgb(0xDC, 0x32, 0x2F),
            green: rgb(0x85, 0x99, 0x00),
            yellow: rgb(0xB5, 0x89, 0x00),
            blue: rgb(0x26, 0x8B, 0xD2),
            magenta: rgb(0xD3, 0x36, 0x82),
            cyan: rgb(0x2A, 0xA1, 0x98),
            white: rgb(0x07, 0x36, 0x42),
            bright_black: rgb(0xFD, 0xF6, 0xE3),
            bright_red: rgb(0xCB, 0x4B, 0x16),
            bright_green: rgb(0x58, 0x6E, 0x75),
            bright_yellow: rgb(0x65, 0x7B, 0x83),
            bright_blue: rgb(0x83, 0x94, 0x96),
            bright_magenta: rgb(0x6C, 0x71, 0xC4),
            bright_cyan: rgb(0x93, 0xA1, 0xA1),
            bright_white: rgb(0x00, 0x2B, 0x36),
            foreground: rgb(0x65, 0x7B, 0x83),
            background: rgb(0xFD, 0xF6, 0xE3),
            cursor: rgb(0x65, 0x7B, 0x83),
            selection_bg: rgb(0xEE, 0xE8, 0xD5),
        }
    }

    pub fn gruvbox_dark() -> Self {
        Self {
            black: rgb(0x28, 0x28, 0x28),
            red: rgb(0xCC, 0x24, 0x1D),
            green: rgb(0x98, 0x97, 0x1A),
            yellow: rgb(0xD7, 0x99, 0x21),
            blue: rgb(0x45, 0x85, 0x88),
            magenta: rgb(0xB1, 0x62, 0x86),
            cyan: rgb(0x68, 0x9D, 0x6A),
            white: rgb(0xA8, 0x99, 0x84),
            bright_black: rgb(0x92, 0x83, 0x74),
            bright_red: rgb(0xFB, 0x49, 0x34),
            bright_green: rgb(0xB8, 0xBB, 0x26),
            bright_yellow: rgb(0xFA, 0xBD, 0x2F),
            bright_blue: rgb(0x83, 0xA5, 0x98),
            bright_magenta: rgb(0xD3, 0x86, 0x9B),
            bright_cyan: rgb(0x8E, 0xC0, 0x7C),
            bright_white: rgb(0xEB, 0xDB, 0xB2),
            foreground: rgb(0xEB, 0xDB, 0xB2),
            background: rgb(0x28, 0x28, 0x28),
            cursor: rgb(0xEB, 0xDB, 0xB2),
            selection_bg: rgb(0x3C, 0x38, 0x36),
        }
    }

    pub fn catppuccin_mocha() -> Self {
        Self {
            black: rgb(0x45, 0x47, 0x5A),
            red: rgb(0xF3, 0x8B, 0xA8),
            green: rgb(0xA6, 0xE3, 0xA1),
            yellow: rgb(0xF9, 0xE2, 0xAF),
            blue: rgb(0x89, 0xB4, 0xFA),
            magenta: rgb(0xF5, 0xC2, 0xE7),
            cyan: rgb(0x94, 0xE2, 0xD5),
            white: rgb(0xBA, 0xC2, 0xDE),
            bright_black: rgb(0x58, 0x5B, 0x70),
            bright_red: rgb(0xF3, 0x8B, 0xA8),
            bright_green: rgb(0xA6, 0xE3, 0xA1),
            bright_yellow: rgb(0xF9, 0xE2, 0xAF),
            bright_blue: rgb(0x89, 0xB4, 0xFA),
            bright_magenta: rgb(0xF5, 0xC2, 0xE7),
            bright_cyan: rgb(0x94, 0xE2, 0xD5),
            bright_white: rgb(0xA6, 0xAD, 0xC8),
            foreground: rgb(0xCD, 0xD6, 0xF4),
            background: rgb(0x1E, 0x1E, 0x2E),
            cursor: rgb(0xCD, 0xD6, 0xF4),
            selection_bg: rgb(0x45, 0x47, 0x5A),
        }
    }

    pub fn tokyo_night() -> Self {
        Self {
            black: rgb(0x1A, 0x1B, 0x26),
            red: rgb(0xF7, 0x76, 0x8E),
            green: rgb(0x9E, 0xCE, 0x6A),
            yellow: rgb(0xE0, 0xAF, 0x68),
            blue: rgb(0x7A, 0xA2, 0xF7),
            magenta: rgb(0xBB, 0x9A, 0xF7),
            cyan: rgb(0x7D, 0xCF, 0xE3),
            white: rgb(0xA9, 0xB1, 0xD6),
            bright_black: rgb(0x41, 0x43, 0x68),
            bright_red: rgb(0xF7, 0x76, 0x8E),
            bright_green: rgb(0x9E, 0xCE, 0x6A),
            bright_yellow: rgb(0xE0, 0xAF, 0x68),
            bright_blue: rgb(0x7A, 0xA2, 0xF7),
            bright_magenta: rgb(0xBB, 0x9A, 0xF7),
            bright_cyan: rgb(0x7D, 0xCF, 0xE3),
            bright_white: rgb(0xC0, 0xCA, 0xF5),
            foreground: rgb(0xC0, 0xCA, 0xF5),
            background: rgb(0x1A, 0x1B, 0x26),
            cursor: rgb(0xC0, 0xCA, 0xF5),
            selection_bg: rgb(0x33, 0x41, 0x57),
        }
    }
}

impl Default for TerminalTheme {
    fn default() -> Self {
        Self::one_dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_themes_have_distinct_colors() {
        let themes = NamedTheme::all();
        for &named in themes {
            let theme = TerminalTheme::default().named(named);
            assert_ne!(theme.foreground, theme.background);
            assert!(theme.foreground != Color32::TRANSPARENT);
            assert!(theme.background != Color32::TRANSPARENT);
        }
    }

    #[test]
    fn test_all_named_themes_have_unique_nonempty_labels() {
        let labels: Vec<&str> = NamedTheme::all().iter().map(|n| n.label()).collect();
        assert!(labels.iter().all(|l| !l.is_empty()));
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(unique.len(), labels.len(), "theme labels must be unique for the UI picker");
    }

    #[test]
    fn test_named_theme_default_is_one_dark() {
        assert_eq!(NamedTheme::default(), NamedTheme::OneDark);
    }

    #[test]
    fn test_one_dark_has_expected_bg() {
        let theme = TerminalTheme::one_dark();
        // Deep black default (cmux-style), not the old One Dark slate.
        assert_eq!(theme.background, rgb(0x0c, 0x0c, 0x0e));
    }

    #[test]
    fn test_dracula_has_expected_fg() {
        let theme = TerminalTheme::dracula();
        assert_eq!(theme.foreground, rgb(0xF8, 0xF8, 0xF2));
    }
}
