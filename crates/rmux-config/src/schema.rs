//! Configuration schema types for rmux.
//!
//! Defines the structure of `rmux.json` configuration files,
//! including terminal, appearance (wallpaper / transparency), and related settings.

use serde::{Deserialize, Serialize};

/// Top-level rmux configuration.
///
/// Loaded from the platform-specific config directory:
/// - Linux: `~/.config/rmux/rmux.json`
/// - macOS: `~/Library/Application Support/rmux/rmux.json`
/// - Windows: `%APPDATA%\rmux\rmux.json`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Terminal emulation settings.
    #[serde(default)]
    pub terminal: TerminalConfig,

    /// Appearance: wallpaper image and terminal background opacity.
    #[serde(default)]
    pub appearance: AppearanceConfig,
}

/// Terminal emulation configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TerminalConfig {
    /// Shell to spawn (defaults to `$SHELL` or `/bin/sh`).
    #[serde(default)]
    pub shell: Option<String>,

    /// Font family for terminal text (must be monospace).
    #[serde(default = "default_font_family")]
    pub font_family: String,

    /// Font size in points.
    #[serde(default = "default_font_size")]
    pub font_size: f32,

    /// Maximum scrollback lines per pane.
    #[serde(default = "default_max_scrollback")]
    pub max_scrollback_lines: usize,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell: None,
            font_family: default_font_family(),
            font_size: default_font_size(),
            max_scrollback_lines: default_max_scrollback(),
        }
    }
}

/// Workspace wallpaper and terminal transparency.
///
/// When enabled with an image path, a single wallpaper is painted behind
/// the entire terminal workspace so every pane (and TUI agents like
/// OpenCode / Claude Code) share the same continuous background. Default
/// terminal cell backgrounds are drawn with [`Self::background_opacity`]
/// so the image shows through consistently.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppearanceConfig {
    /// When `true` and [`Self::background_image`] is set, paint the
    /// wallpaper and apply terminal background transparency.
    #[serde(default = "default_background_enabled")]
    pub background_enabled: bool,

    /// Absolute (or `~`-prefixed) path to a wallpaper image (PNG, JPEG, WebP, GIF).
    #[serde(default)]
    pub background_image: Option<String>,

    /// Opacity of the default terminal background over the wallpaper.
    ///
    /// `0.0` = fully transparent (image fully visible), `1.0` = fully opaque
    /// (classic solid terminal). Typical range for readable agents is `0.55`–`0.85`.
    #[serde(default = "default_background_opacity")]
    pub background_opacity: f32,

    /// Opacity of the left workspace sidebar fill over the wallpaper.
    ///
    /// `1.0` = solid sidebar (default). Lower values give a glass sidebar like
    /// Ghostty/cmux over the shared wallpaper (cards stay more opaque).
    #[serde(default = "default_sidebar_opacity")]
    pub sidebar_opacity: f32,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            background_enabled: default_background_enabled(),
            background_image: None,
            background_opacity: default_background_opacity(),
            sidebar_opacity: default_sidebar_opacity(),
        }
    }
}

impl AppearanceConfig {
    /// Clamp terminal background opacity into `[0.0, 1.0]`.
    pub fn clamped_opacity(&self) -> f32 {
        self.background_opacity.clamp(0.0, 1.0)
    }

    /// Clamp sidebar opacity into `[0.0, 1.0]`.
    pub fn clamped_sidebar_opacity(&self) -> f32 {
        self.sidebar_opacity.clamp(0.0, 1.0)
    }

    /// Whether wallpaper + transparency should be active this frame.
    pub fn wallpaper_active(&self) -> bool {
        self.background_enabled
            && self.background_image.as_ref().is_some_and(|p| !p.trim().is_empty())
    }
}

fn default_font_family() -> String {
    "monospace".to_owned()
}

fn default_font_size() -> f32 {
    14.0
}

fn default_max_scrollback() -> usize {
    10_000
}

fn default_background_enabled() -> bool {
    false
}

fn default_background_opacity() -> f32 {
    0.72
}

fn default_sidebar_opacity() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_config_defaults() {
        let config = TerminalConfig::default();
        assert_eq!(config.font_family, "monospace");
        assert_eq!(config.font_size, 14.0);
        assert_eq!(config.max_scrollback_lines, 10_000);
        assert!(config.shell.is_none());
    }

    #[test]
    fn test_config_deserialize_empty() {
        let json = r#"{"terminal":{}}"#;
        let config: Config = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(config.terminal.font_family, "monospace");
        assert!(!config.appearance.background_enabled);
        assert!(config.appearance.background_image.is_none());
        assert!((config.appearance.background_opacity - 0.72).abs() < f32::EPSILON);
        assert!((config.appearance.sidebar_opacity - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_appearance_wallpaper_active() {
        let mut a = AppearanceConfig::default();
        assert!(!a.wallpaper_active());
        a.background_enabled = true;
        assert!(!a.wallpaper_active());
        a.background_image = Some("/tmp/art.jpg".into());
        assert!(a.wallpaper_active());
        a.background_image = Some("   ".into());
        assert!(!a.wallpaper_active());
    }

    #[test]
    fn test_appearance_roundtrip() {
        let cfg = Config {
            terminal: TerminalConfig::default(),
            appearance: AppearanceConfig {
                background_enabled: true,
                background_image: Some("~/Pictures/school-of-athens.jpg".into()),
                background_opacity: 0.65,
                sidebar_opacity: 0.55,
            },
        };
        let json = serde_json::to_string(&cfg).expect("serialize");
        let back: Config = serde_json::from_str(&json).expect("deserialize");
        assert!(back.appearance.background_enabled);
        assert_eq!(
            back.appearance.background_image.as_deref(),
            Some("~/Pictures/school-of-athens.jpg")
        );
        assert!((back.appearance.background_opacity - 0.65).abs() < f32::EPSILON);
        assert!((back.appearance.sidebar_opacity - 0.55).abs() < f32::EPSILON);
    }

    #[test]
    fn test_clamped_opacity() {
        let a = AppearanceConfig { background_opacity: 1.5, ..Default::default() };
        assert!((a.clamped_opacity() - 1.0).abs() < f32::EPSILON);
        let b = AppearanceConfig { background_opacity: -0.2, ..Default::default() };
        assert!((b.clamped_opacity() - 0.0).abs() < f32::EPSILON);
        let c = AppearanceConfig { sidebar_opacity: 2.0, ..Default::default() };
        assert!((c.clamped_sidebar_opacity() - 1.0).abs() < f32::EPSILON);
    }
}
