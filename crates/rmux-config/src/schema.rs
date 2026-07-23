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
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    /// Terminal emulation settings.
    #[serde(default)]
    pub terminal: TerminalConfig,
    /// Session save/restore (cmux-style workspace history).
    #[serde(default)]
    pub session: SessionConfig,
}

/// Session history configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionConfig {
    /// Restore the last session on launch (default true).
    #[serde(default = "default_true")]
    pub auto_restore: bool,
    /// Autosave interval in seconds (default 8, matching cmux).
    #[serde(default = "default_autosave_secs")]
    pub autosave_secs: u64,
    /// Include terminal scrollback on quit save (Phase B; currently unused).
    #[serde(default)]
    pub include_scrollback: bool,
    /// Auto-run agent resume commands on restore (Phase C; currently unused).
    #[serde(default = "default_true")]
    pub auto_resume_agents: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            auto_restore: true,
            autosave_secs: default_autosave_secs(),
            include_scrollback: false,
            auto_resume_agents: true,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_autosave_secs() -> u64 {
    8
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
        assert!(config.session.auto_restore);
        assert_eq!(config.session.autosave_secs, 8);
    }

    #[test]
    fn test_session_config_defaults() {
        let c = SessionConfig::default();
        assert!(c.auto_restore);
        assert!(c.auto_resume_agents);
        assert!(!c.include_scrollback);
    }
}
