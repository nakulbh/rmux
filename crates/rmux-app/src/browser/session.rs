//! Persist browser pane URL + history across restarts (Phase 4.4 / E3.2).
//!
//! Cookies live in the Chromium profile dir (`chromium/runtime::profile_dir`).
//! This module only stores navigation state for reopening panes.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

const SESSION_VERSION: u32 = 1;
/// Cap restored browser panes so a huge session file cannot flood the UI.
const MAX_RESTORE_PANES: usize = 8;

/// One browser pane's navigable state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserPaneSnapshot {
    pub url: String,
    #[serde(default)]
    pub history: Vec<String>,
    #[serde(default)]
    pub history_index: usize,
    #[serde(default)]
    pub title: String,
}

/// On-disk session file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserSessionFile {
    pub version: u32,
    #[serde(default)]
    pub browsers: Vec<BrowserPaneSnapshot>,
}

impl Default for BrowserSessionFile {
    fn default() -> Self {
        Self { version: SESSION_VERSION, browsers: Vec::new() }
    }
}

/// Default path: `~/.config/rmux/browser_session.json` (or platform equivalent).
#[must_use]
pub fn session_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("rmux").join("browser_session.json");
    }
    if let Ok(home) = std::env::var("HOME") {
        #[cfg(target_os = "macos")]
        {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("rmux")
                .join("browser_session.json");
        }
        #[cfg(not(target_os = "macos"))]
        {
            return PathBuf::from(home).join(".config").join("rmux").join("browser_session.json");
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("rmux").join("browser_session.json");
    }
    PathBuf::from("browser_session.json")
}

/// Load session file if present.
pub fn load() -> Result<Option<BrowserSessionFile>> {
    let path = session_path();
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("read browser session {}", path.display()))?;
    let file: BrowserSessionFile = serde_json::from_str(&raw)
        .with_context(|| format!("parse browser session {}", path.display()))?;
    if file.version > SESSION_VERSION {
        warn!(
            version = file.version,
            expected = SESSION_VERSION,
            "browser session version newer than supported — ignoring"
        );
        return Ok(None);
    }
    Ok(Some(file))
}

/// Save browser snapshots (creates parent directories).
pub fn save(browsers: &[BrowserPaneSnapshot]) -> Result<()> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create session dir {}", parent.display()))?;
    }
    let file = BrowserSessionFile {
        version: SESSION_VERSION,
        browsers: browsers.iter().take(MAX_RESTORE_PANES).cloned().collect(),
    };
    let raw = serde_json::to_string_pretty(&file).context("serialize browser session")?;
    std::fs::write(&path, raw).with_context(|| format!("write browser session {}", path.display()))?;
    info!(path = %path.display(), count = file.browsers.len(), "Saved browser session");
    Ok(())
}

/// Filter snapshots worth restoring (skip blank about:blank-only tabs).
#[must_use]
pub fn filter_restorable(browsers: Vec<BrowserPaneSnapshot>) -> Vec<BrowserPaneSnapshot> {
    browsers
        .into_iter()
        .filter(|b| is_restorable_url(&b.url))
        .take(MAX_RESTORE_PANES)
        .collect()
}

fn is_restorable_url(url: &str) -> bool {
    let u = url.trim();
    !u.is_empty() && u != "about:blank" && !u.starts_with("data:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_skips_blank() {
        let snaps = vec![
            BrowserPaneSnapshot {
                url: "about:blank".into(),
                history: vec!["about:blank".into()],
                history_index: 0,
                title: String::new(),
            },
            BrowserPaneSnapshot {
                url: "https://example.com".into(),
                history: vec!["about:blank".into(), "https://example.com".into()],
                history_index: 1,
                title: "Example".into(),
            },
        ];
        let f = filter_restorable(snaps);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].url, "https://example.com");
    }

    #[test]
    fn roundtrip_json() {
        let file = BrowserSessionFile {
            version: 1,
            browsers: vec![BrowserPaneSnapshot {
                url: "https://example.com".into(),
                history: vec!["https://example.com".into()],
                history_index: 0,
                title: "Example Domain".into(),
            }],
        };
        let s = serde_json::to_string(&file).unwrap();
        let back: BrowserSessionFile = serde_json::from_str(&s).unwrap();
        assert_eq!(back, file);
    }
}
