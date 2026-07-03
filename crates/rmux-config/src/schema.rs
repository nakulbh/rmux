//! Configuration schema types for rmux.
//!
//! Defines the structure of `rmux.json` configuration files,
//! including terminal, browser, notification, and shortcut settings.
//!
//! Will be fully implemented in Phase 1.

use serde::{Deserialize, Serialize};

/// Top-level rmux configuration.
///
/// Loaded from the platform-specific config directory:
/// - Linux: `~/.config/rmux/rmux.json`
/// - macOS: `~/Library/Application Support/rmux/rmux.json`
/// - Windows: `%APPDATA%\rmux\rmux.json`
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// Terminal emulation settings.
    #[serde(default)]
    pub terminal: TerminalConfig,
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

fn default_font_family() -> String {
    "monospace".to_owned()
}

fn default_font_size() -> f32 {
    14.0
}

fn default_max_scrollback() -> usize {
    10_000
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
    }
}
