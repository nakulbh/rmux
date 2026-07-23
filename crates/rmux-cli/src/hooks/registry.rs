//! Agent identities and PATH detection for hook setup.

use std::env;
use std::path::{Path, PathBuf};

/// Supported agents for the MVP notification hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentId {
    /// Anthropic Claude Code CLI (`claude`).
    Claude,
    /// OpenCode CLI (`opencode`).
    OpenCode,
}

impl AgentId {
    /// Stable CLI name (`claude`, `opencode`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::OpenCode => "opencode",
        }
    }

    /// Binary name checked on `PATH` during setup.
    #[must_use]
    pub const fn binary_name(self) -> &'static str {
        self.as_str()
    }

    /// Human-readable display name for notifications.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::OpenCode => "OpenCode",
        }
    }

    /// Env var that disables this agent's hooks for one process.
    #[must_use]
    pub const fn disable_env(self) -> &'static str {
        match self {
            Self::Claude => "RMUX_CLAUDE_HOOKS_DISABLED",
            Self::OpenCode => "RMUX_OPENCODE_HOOKS_DISABLED",
        }
    }

    /// All agents supported by this MVP.
    #[must_use]
    pub const fn all() -> [Self; 2] {
        [Self::Claude, Self::OpenCode]
    }
}

/// Whether `binary` resolves on the current `PATH`.
#[must_use]
pub fn binary_on_path(binary: &str) -> bool {
    env::var_os("PATH")
        .map(|paths| {
            env::split_paths(&paths).any(|dir| {
                let candidate = dir.join(binary);
                candidate.is_file()
            })
        })
        .unwrap_or(false)
}

/// Resolve the path used to invoke `rmux-cli` from installed hooks.
///
/// Prefer the absolute path of the current executable so hooks work even
/// when `rmux-cli` is not on PATH. Falls back to the bare name.
#[must_use]
pub fn rmux_cli_command() -> String {
    env::current_exe()
        .ok()
        .filter(|p| p.is_file())
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "rmux-cli".to_owned())
}

/// Home directory (Unix `HOME`, Windows `USERPROFILE`).
#[must_use]
pub fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")).map(PathBuf::from)
}

/// Claude Code user settings path: `~/.claude/settings.json`.
#[must_use]
pub fn claude_settings_path() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".claude").join("settings.json"))
}

/// OpenCode config directory: `$OPENCODE_CONFIG_DIR` or `~/.config/opencode`.
#[must_use]
pub fn opencode_config_dir() -> Option<PathBuf> {
    if let Ok(dir) = env::var("OPENCODE_CONFIG_DIR") {
        let p = PathBuf::from(dir);
        if !p.as_os_str().is_empty() {
            return Some(p);
        }
    }
    home_dir().map(|h| h.join(".config").join("opencode"))
}

/// Escape a path for embedding in a double-quoted shell command string.
#[must_use]
pub fn shell_double_quote(path: &str) -> String {
    format!("\"{}\"", path.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Marker for OpenCode plugin file.
pub const OPENCODE_PLUGIN_MARKER: &str = "rmux-notify-plugin-marker v1";

/// Whether hooks are globally disabled via environment.
#[must_use]
pub fn hooks_globally_disabled() -> bool {
    env::var_os("RMUX_HOOKS_DISABLED").is_some_and(|v| v == "1")
}

/// Whether hooks for `agent` are disabled via environment.
#[must_use]
pub fn agent_hooks_disabled(agent: AgentId) -> bool {
    hooks_globally_disabled() || env::var_os(agent.disable_env()).is_some_and(|v| v == "1")
}

/// Ensure parent directories exist for `path`.
pub fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_names_are_stable() {
        assert_eq!(AgentId::Claude.as_str(), "claude");
        assert_eq!(AgentId::OpenCode.binary_name(), "opencode");
        assert_eq!(AgentId::Claude.display_name(), "Claude Code");
    }

    #[test]
    fn shell_double_quote_escapes() {
        assert_eq!(shell_double_quote(r#"/tmp/a"b"#), r#""/tmp/a\"b""#);
    }
}
