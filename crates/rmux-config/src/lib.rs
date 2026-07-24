#![forbid(unsafe_code)]
//! Configuration management for rmux.
//!
//! Loads and saves the rmux configuration from platform-appropriate
//! directories. Defines the config schema and provides path helpers.
//!
//! # Modules
//!
//! - `schema` — Configuration types and deserialization

pub mod schema;

pub use schema::{AppearanceConfig, Config, SessionConfig, TerminalConfig};

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

/// Errors from config load / save.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// I/O failure reading or writing the config file.
    #[error("config I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON parse / serialize failure.
    #[error("config JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// Config directory could not be resolved on this platform.
    #[error("could not resolve config directory")]
    NoConfigDir,
}

/// Result alias for config operations.
pub type ConfigResult<T> = Result<T, ConfigError>;

/// Platform config directory for rmux (`…/rmux`), creating nothing.
///
/// - Linux: `~/.config/rmux`
/// - macOS: `~/Library/Application Support/rmux`
/// - Windows: `%APPDATA%\rmux`
pub fn config_dir() -> ConfigResult<PathBuf> {
    let base = dirs::config_dir().ok_or(ConfigError::NoConfigDir)?;
    Ok(base.join("rmux"))
}

/// Full path to `rmux.json`.
pub fn config_path() -> ConfigResult<PathBuf> {
    Ok(config_dir()?.join("rmux.json"))
}

/// Expand a leading `~/` to the user's home directory.
///
/// Other paths are returned unchanged (as owned [`PathBuf`]s).
pub fn expand_tilde(path: &str) -> PathBuf {
    let trimmed = path.trim();
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    } else if trimmed == "~"
        && let Some(home) = dirs::home_dir()
    {
        return home;
    }
    PathBuf::from(trimmed)
}

/// Load config from the default path, or return defaults if missing.
///
/// A missing file is not an error — returns [`Config::default`].
/// Corrupt JSON is an error so the user can fix the file.
pub fn load() -> ConfigResult<Config> {
    let path = config_path()?;
    load_from(&path)
}

/// Load config from an explicit path.
///
/// Missing file → defaults. Existing but invalid → error.
pub fn load_from(path: &Path) -> ConfigResult<Config> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let content = fs::read_to_string(path)?;
    let config = serde_json::from_str(&content)?;
    Ok(config)
}

/// Save config to the default path, creating the directory if needed.
pub fn save(config: &Config) -> ConfigResult<()> {
    let path = config_path()?;
    save_to(config, &path)
}

/// Save config to an explicit path, creating parent directories as needed.
pub fn save_to(config: &Config, path: &Path) -> ConfigResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(config)?;
    fs::write(path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use schema::AppearanceConfig;

    #[test]
    fn test_expand_tilde_home() {
        let expanded = expand_tilde("~/Pictures/bg.jpg");
        let s = expanded.to_string_lossy();
        assert!(!s.starts_with('~'), "tilde should expand: {s}");
        assert!(s.ends_with("Pictures/bg.jpg") || s.ends_with("Pictures\\bg.jpg"));
    }

    #[test]
    fn test_expand_absolute_unchanged() {
        let p = expand_tilde("/tmp/art.png");
        assert_eq!(p, PathBuf::from("/tmp/art.png"));
    }

    #[test]
    fn test_load_missing_returns_default() {
        let dir = std::env::temp_dir().join(format!("rmux-config-test-{}", std::process::id()));
        let path = dir.join("missing.json");
        let _ = fs::remove_file(&path);
        let cfg = load_from(&path).expect("missing ok");
        assert!(!cfg.appearance.background_enabled);
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("rmux-config-rt-{}", std::process::id()));
        let path = dir.join("rmux.json");
        let _ = fs::remove_dir_all(&dir);

        let cfg = Config {
            terminal: TerminalConfig::default(),
            appearance: AppearanceConfig {
                background_enabled: true,
                background_image: Some("~/Wallpapers/athens.jpg".into()),
                background_opacity: 0.6,
                sidebar_opacity: 0.5,
            },
            session: SessionConfig::default(),
        };
        save_to(&cfg, &path).expect("save");
        let loaded = load_from(&path).expect("load");
        assert!(loaded.appearance.background_enabled);
        assert_eq!(loaded.appearance.background_image.as_deref(), Some("~/Wallpapers/athens.jpg"));
        assert!((loaded.appearance.background_opacity - 0.6).abs() < f32::EPSILON);
        assert!((loaded.appearance.sidebar_opacity - 0.5).abs() < f32::EPSILON);

        let _ = fs::remove_dir_all(&dir);
    }
}
