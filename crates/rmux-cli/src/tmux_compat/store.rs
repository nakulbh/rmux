//! Persistent fake-tmux ↔ rmux pane mapping for `__tmux-compat`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};

/// On-disk store under `~/.rmuxterm/tmux-compat-store.json`.
#[derive(Debug, Clone)]
pub struct TmuxCompatStore {
    path: PathBuf,
    /// Fake tmux pane id (`%0`) → (workspace_id, pane_id).
    panes: Map<String, Value>,
    next_fake: u64,
    /// Active fake pane key (`%N`).
    active: String,
}

impl TmuxCompatStore {
    /// Load from disk or create empty.
    pub fn load() -> Result<Self> {
        let path = store_path().context("HOME not set; cannot locate ~/.rmuxterm")?;
        Self::load_from(&path)
    }

    /// Load from an explicit path (tests).
    pub fn load_from(path: &Path) -> Result<Self> {
        if path.exists() {
            let text =
                fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
            if !text.trim().is_empty() {
                let value: Value = serde_json::from_str(&text)
                    .with_context(|| format!("parse {}", path.display()))?;
                let panes =
                    value.get("panes").and_then(Value::as_object).cloned().unwrap_or_default();
                let next_fake = value.get("next_fake").and_then(Value::as_u64).unwrap_or(0);
                let active = value.get("active").and_then(Value::as_str).unwrap_or("%0").to_owned();
                return Ok(Self { path: path.to_path_buf(), panes, next_fake, active });
            }
        }
        Ok(Self {
            path: path.to_path_buf(),
            panes: Map::new(),
            next_fake: 0,
            active: "%0".to_owned(),
        })
    }

    /// Persist to disk.
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
        }
        let value = json!({
            "panes": self.panes,
            "next_fake": self.next_fake,
            "active": self.active,
        });
        let text = serde_json::to_string_pretty(&value)?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, format!("{text}\n")).with_context(|| format!("write {}", tmp.display()))?;
        fs::rename(&tmp, &self.path)
            .with_context(|| format!("rename onto {}", self.path.display()))?;
        Ok(())
    }

    /// Ensure the current env pane is registered; returns its fake id.
    pub fn ensure_current_from_env(&mut self) -> Result<String> {
        let workspace_id = std::env::var("RMUX_WORKSPACE_ID").ok().and_then(|s| s.parse().ok());
        let pane_id = std::env::var("RMUX_PANE_ID").ok().and_then(|s| s.parse().ok());
        if let (Some(ws), Some(pane)) = (workspace_id, pane_id) {
            // Reuse existing mapping if this rmux pane already known.
            for (fake, val) in &self.panes {
                if val.get("pane_id").and_then(Value::as_u64) == Some(pane)
                    && val.get("workspace_id").and_then(Value::as_u64) == Some(ws)
                {
                    self.active = fake.clone();
                    return Ok(fake.clone());
                }
            }
            let fake = self.alloc_fake(ws, pane);
            self.active = fake.clone();
            return Ok(fake);
        }
        // No env: keep or create %0 placeholder.
        if self.panes.is_empty() {
            let fake = self.alloc_fake(0, 0);
            self.active = fake.clone();
            return Ok(fake);
        }
        Ok(self.active.clone())
    }

    /// Allocate a new fake pane id for a real rmux pane.
    pub fn alloc_fake(&mut self, workspace_id: u64, pane_id: u64) -> String {
        let fake = format!("%{}", self.next_fake);
        self.next_fake += 1;
        self.panes
            .insert(fake.clone(), json!({ "workspace_id": workspace_id, "pane_id": pane_id }));
        fake
    }

    /// Resolve a target like `%1`, `:.1`, or empty (active).
    pub fn resolve_target(&self, target: Option<&str>) -> Option<(u64, u64)> {
        let key = match target {
            None | Some("") | Some(".") | Some(":") => self.active.clone(),
            Some(t) if t.starts_with('%') => t.to_owned(),
            Some(t) if t.starts_with(':') => {
                let rest = t.trim_start_matches(':').trim_start_matches('.');
                if rest.starts_with('%') { rest.to_owned() } else { format!("%{rest}") }
            }
            Some(t) => format!("%{t}"),
        };
        self.lookup_fake(&key)
    }

    fn lookup_fake(&self, key: &str) -> Option<(u64, u64)> {
        let val = self.panes.get(key)?;
        let workspace_id = val.get("workspace_id").and_then(Value::as_u64)?;
        let pane_id = val.get("pane_id").and_then(Value::as_u64)?;
        Some((workspace_id, pane_id))
    }

    /// Mark fake pane as active.
    pub fn set_active(&mut self, fake: &str) {
        if self.panes.contains_key(fake) {
            self.active = fake.to_owned();
        }
    }

    /// Active fake id.
    #[must_use]
    pub fn active(&self) -> &str {
        &self.active
    }

    /// Remove a fake pane mapping.
    pub fn remove_fake(&mut self, fake: &str) {
        self.panes.remove(fake);
        if self.active == fake {
            self.active = self.panes.keys().next().cloned().unwrap_or_else(|| "%0".to_owned());
        }
    }

    /// Find fake key for a real pane_id.
    pub fn fake_for_pane(&self, pane_id: u64) -> Option<String> {
        for (fake, val) in &self.panes {
            if val.get("pane_id").and_then(Value::as_u64) == Some(pane_id) {
                return Some(fake.clone());
            }
        }
        None
    }

    /// All mapped fake panes.
    pub fn all_panes(&self) -> Vec<(String, u64, u64)> {
        let mut out = Vec::new();
        for (fake, val) in &self.panes {
            let ws = val.get("workspace_id").and_then(Value::as_u64).unwrap_or(0);
            let pane = val.get("pane_id").and_then(Value::as_u64).unwrap_or(0);
            out.push((fake.clone(), ws, pane));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}

fn store_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".rmuxterm").join("tmux-compat-store.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_and_resolve() {
        let dir = std::env::temp_dir().join(format!("rmux-tmux-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("store.json");
        let mut store = TmuxCompatStore::load_from(&path).unwrap();
        let f0 = store.alloc_fake(1, 10);
        assert_eq!(f0, "%0");
        let f1 = store.alloc_fake(1, 11);
        assert_eq!(f1, "%1");
        store.set_active("%1");
        assert_eq!(store.resolve_target(None), Some((1, 11)));
        assert_eq!(store.resolve_target(Some("%0")), Some((1, 10)));
        store.save().unwrap();
        let store2 = TmuxCompatStore::load_from(&path).unwrap();
        assert_eq!(store2.resolve_target(Some("%0")), Some((1, 10)));
        let _ = fs::remove_dir_all(&dir);
    }
}
