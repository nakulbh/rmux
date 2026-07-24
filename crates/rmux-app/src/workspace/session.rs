//! Session save/restore — cmux-style workspace history.
//!
//! Persists the window chrome, workspace list, recursive pane tree, and
//! per-terminal working directories so relaunching rmux rebuilds the same
//! layout. Live process memory is not checkpointed; shells spawn fresh in
//! the saved cwd.
//!
//! # On-disk layout
//!
//! - Primary: `{state}/rmux/session.json`
//! - Manual restore backup: `{state}/rmux/session-previous.json`
//!
//! Empty sessions remove the primary file. Corrupt/unusable primary falls
//! back to the previous snapshot (mirrors cmux).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::WorkspaceManager;
use super::model::{Workspace, WorkspaceId};
use super::splits::{PaneId, PaneNode, SplitDirection, SplitId};
use super::surface::{Surface, SurfaceId};
use crate::browser::BrowserPane;
use crate::ui::TerminalPane;

/// Schema version written into every snapshot.
pub const SESSION_SCHEMA_VERSION: u32 = 1;

/// Default autosave interval (seconds), matching cmux's ~8s timer.
pub const DEFAULT_AUTOSAVE_SECS: u64 = 8;

/// Env var to force-disable restore (tests / recovery).
pub const ENV_DISABLE_RESTORE: &str = "RMUX_DISABLE_SESSION_RESTORE";

/// Env var overriding the session directory (tests).
pub const ENV_STATE_DIR: &str = "RMUX_STATE_DIR";

// ── Snapshot DTOs ──────────────────────────────────────────────────────────

/// Root session snapshot persisted as JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionSnapshot {
    pub version: u32,
    pub created_at: f64,
    #[serde(default)]
    pub window: WindowSnapshot,
    pub active_workspace_index: usize,
    pub workspaces: Vec<WorkspaceSnapshot>,
}

/// Window chrome (single window today).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowSnapshot {
    #[serde(default = "default_inner_size")]
    pub inner_size: [f32; 2],
    #[serde(default = "default_true")]
    pub sidebar_visible: bool,
    #[serde(default)]
    pub right_sidebar_visible: bool,
    #[serde(default)]
    pub notification_panel_visible: bool,
}

fn default_inner_size() -> [f32; 2] {
    [1200.0, 800.0]
}

fn default_true() -> bool {
    true
}

impl Default for WindowSnapshot {
    fn default() -> Self {
        Self {
            inner_size: default_inner_size(),
            sidebar_visible: true,
            right_sidebar_visible: false,
            notification_panel_visible: false,
        }
    }
}

/// One workspace in the session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceSnapshot {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub name_is_custom: bool,
    #[serde(default)]
    pub process_title: String,
    pub active_pane_id: u64,
    #[serde(default)]
    pub zoomed_pane_id: Option<u64>,
    pub layout: NodeSnapshot,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub progress: Option<f32>,
    /// Next surface id counter for this workspace after restore.
    #[serde(default = "default_next_surface_id")]
    pub next_surface_id: u64,
}

fn default_next_surface_id() -> u64 {
    1
}

/// Recursive pane tree node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeSnapshot {
    Leaf {
        id: u64,
        #[serde(default)]
        active_surface: usize,
        #[serde(default)]
        surfaces: Vec<SurfaceSnapshot>,
    },
    Browser {
        id: u64,
        #[serde(default)]
        url: String,
        #[serde(default)]
        title: Option<String>,
    },
    Split {
        id: u64,
        direction: SplitDirectionSnap,
        #[serde(default)]
        ratios: Vec<f32>,
        children: Vec<NodeSnapshot>,
    },
}

/// Split orientation wire format.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SplitDirectionSnap {
    Horizontal,
    Vertical,
}

impl From<SplitDirection> for SplitDirectionSnap {
    fn from(d: SplitDirection) -> Self {
        match d {
            SplitDirection::Horizontal => Self::Horizontal,
            SplitDirection::Vertical => Self::Vertical,
        }
    }
}

impl From<SplitDirectionSnap> for SplitDirection {
    fn from(d: SplitDirectionSnap) -> Self {
        match d {
            SplitDirectionSnap::Horizontal => Self::Horizontal,
            SplitDirectionSnap::Vertical => Self::Vertical,
        }
    }
}

/// One terminal tab inside a leaf.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SurfaceSnapshot {
    pub id: u64,
    pub title: String,
    /// True when the title was user-set (not the default `Terminal N`).
    #[serde(default)]
    pub title_is_custom: bool,
    #[serde(default)]
    pub cwd: Option<String>,
    /// Phase B: optional truncated scrollback text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scrollback: Option<String>,
}

impl SessionSnapshot {
    /// Whether this snapshot can seed a session (at least one workspace).
    pub fn is_usable(&self) -> bool {
        self.version == SESSION_SCHEMA_VERSION && !self.workspaces.is_empty()
    }

    /// Fingerprint for autosave skip (stable JSON without `created_at`).
    pub fn content_fingerprint(&self) -> String {
        let mut clone = self.clone();
        clone.created_at = 0.0;
        serde_json::to_string(&clone).unwrap_or_default()
    }
}

// ── Store ──────────────────────────────────────────────────────────────────

/// Result of inspecting a snapshot file.
#[derive(Debug, Clone, PartialEq)]
pub enum LoadOutcome {
    Loaded(SessionSnapshot),
    Missing,
    Unusable,
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("session I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("session JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("session restore failed: {0}")]
    Restore(String),
}

/// File-backed session store (primary + previous backup).
#[derive(Debug, Clone)]
pub struct SessionStore {
    dir: PathBuf,
}

impl SessionStore {
    /// Default store under the platform state directory.
    pub fn default_store() -> Self {
        Self { dir: default_state_dir() }
    }

    /// Store rooted at an explicit directory (tests / `RMUX_STATE_DIR`).
    #[allow(dead_code)] // used by unit tests and future CLI `restore-session`
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    #[allow(dead_code)] // diagnostics / tests
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn primary_path(&self) -> PathBuf {
        self.dir.join("session.json")
    }

    pub fn previous_path(&self) -> PathBuf {
        self.dir.join("session-previous.json")
    }

    /// Inspect `path` without side effects.
    pub fn load_outcome(&self, path: &Path) -> LoadOutcome {
        if !path.exists() {
            return LoadOutcome::Missing;
        }
        match fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str::<SessionSnapshot>(&text) {
                Ok(snap) if snap.is_usable() => LoadOutcome::Loaded(snap),
                _ => LoadOutcome::Unusable,
            },
            Err(_) => LoadOutcome::Unusable,
        }
    }

    /// Load primary if usable, else previous when primary is unusable.
    pub fn load_startup(&self) -> LoadOutcome {
        match self.load_outcome(&self.primary_path()) {
            LoadOutcome::Loaded(s) => LoadOutcome::Loaded(s),
            LoadOutcome::Missing => LoadOutcome::Missing,
            LoadOutcome::Unusable => match self.load_outcome(&self.previous_path()) {
                LoadOutcome::Loaded(s) => {
                    tracing::warn!("primary session unusable; using session-previous.json");
                    LoadOutcome::Loaded(s)
                }
                other => other,
            },
        }
    }

    /// Load the manual-restore (`Cmd+Shift+O`) snapshot.
    pub fn load_previous(&self) -> Option<SessionSnapshot> {
        match self.load_outcome(&self.previous_path()) {
            LoadOutcome::Loaded(s) => Some(s),
            _ => None,
        }
    }

    /// Atomic save to primary; sync previous when primary is usable.
    pub fn save(&self, snapshot: &SessionSnapshot) -> Result<(), SessionError> {
        if snapshot.workspaces.is_empty() {
            self.remove_primary();
            return Ok(());
        }
        fs::create_dir_all(&self.dir)?;
        let primary = self.primary_path();
        let data = serde_json::to_vec_pretty(snapshot)?;
        // Skip write when identical (cmux optimization).
        if let Ok(existing) = fs::read(&primary)
            && existing == data
        {
            return Ok(());
        }
        let tmp = self.dir.join(format!(".session.{}.tmp", std::process::id()));
        fs::write(&tmp, &data)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        }
        fs::rename(&tmp, &primary)?;
        // Mirror usable primary into previous (manual restore + crash recovery).
        let prev = self.previous_path();
        if let Err(err) = fs::copy(&primary, &prev) {
            tracing::warn!(error = %err, "failed to sync session-previous.json");
        }
        Ok(())
    }

    pub fn remove_primary(&self) {
        let _ = fs::remove_file(self.primary_path());
    }
}

/// Resolve the session state directory.
pub fn default_state_dir() -> PathBuf {
    if let Ok(override_dir) = std::env::var(ENV_STATE_DIR) {
        let trimmed = override_dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    platform_state_dir().join("rmux")
}

fn platform_state_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home_dir()
            .map(|h| h.join("Library").join("Application Support"))
            .unwrap_or_else(|| PathBuf::from("."))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(home_dir)
            .unwrap_or_else(|| PathBuf::from("."))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
            let t = xdg.trim();
            if !t.is_empty() {
                return PathBuf::from(t);
            }
        }
        home_dir().map(|h| h.join(".local").join("state")).unwrap_or_else(|| PathBuf::from("."))
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

/// Whether automatic restore should run on this launch.
pub fn should_attempt_restore(session_override: Option<&str>) -> bool {
    if session_override.is_some() {
        return true; // explicit path always loads
    }
    if std::env::var(ENV_DISABLE_RESTORE).as_deref() == Ok("1") {
        return false;
    }
    true
}

fn now_unix() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

// ── Capture ────────────────────────────────────────────────────────────────

/// Context for capturing UI chrome alongside the workspace tree.
#[derive(Debug, Clone, Default)]
pub struct CaptureUiState {
    pub inner_size: [f32; 2],
    pub sidebar_visible: bool,
    pub right_sidebar_visible: bool,
    pub notification_panel_visible: bool,
}

/// Capture the live manager into a session snapshot.
pub fn capture_session(manager: &WorkspaceManager, ui: CaptureUiState) -> SessionSnapshot {
    let workspaces: Vec<WorkspaceSnapshot> =
        manager.workspaces().iter().map(capture_workspace).collect();
    SessionSnapshot {
        version: SESSION_SCHEMA_VERSION,
        created_at: now_unix(),
        window: WindowSnapshot {
            inner_size: ui.inner_size,
            sidebar_visible: ui.sidebar_visible,
            right_sidebar_visible: ui.right_sidebar_visible,
            notification_panel_visible: ui.notification_panel_visible,
        },
        active_workspace_index: manager.active_index(),
        workspaces,
    }
}

fn capture_workspace(ws: &Workspace) -> WorkspaceSnapshot {
    WorkspaceSnapshot {
        id: ws.id.0,
        name: ws.name.clone(),
        name_is_custom: ws.name_is_custom,
        process_title: ws.process_title.clone(),
        active_pane_id: ws.active_pane.0,
        zoomed_pane_id: ws.zoomed_pane.map(|p| p.0),
        layout: capture_node(&ws.root),
        status: ws.status.clone(),
        progress: ws.progress,
        next_surface_id: ws.next_surface_id,
    }
}

fn capture_node(node: &PaneNode) -> NodeSnapshot {
    match node {
        PaneNode::Leaf { id, active_surface, surfaces, terminal, .. } => {
            let mut surface_snaps = Vec::new();
            if !surfaces.is_empty() {
                for s in surfaces {
                    surface_snaps.push(capture_surface(s));
                }
            } else if let Some(term) = terminal.as_ref() {
                // Legacy single-terminal leaf.
                surface_snaps.push(SurfaceSnapshot {
                    id: 1,
                    title: "Terminal 1".to_owned(),
                    title_is_custom: false,
                    cwd: term.working_directory().map(|p| p.to_string_lossy().into_owned()),
                    scrollback: None,
                });
            } else {
                // Empty placeholder leaf — restore will spawn a default shell.
                surface_snaps.push(SurfaceSnapshot {
                    id: 1,
                    title: "Terminal 1".to_owned(),
                    title_is_custom: false,
                    cwd: None,
                    scrollback: None,
                });
            }
            NodeSnapshot::Leaf {
                id: id.0,
                active_surface: (*active_surface).min(surface_snaps.len().saturating_sub(1)),
                surfaces: surface_snaps,
            }
        }
        PaneNode::Browser { id, browser } => NodeSnapshot::Browser {
            id: id.0,
            url: browser.url().to_owned(),
            title: {
                let t = browser.title();
                if t.is_empty() { None } else { Some(t.to_owned()) }
            },
        },
        PaneNode::Split { id, direction, children, sizes } => NodeSnapshot::Split {
            id: id.0,
            direction: (*direction).into(),
            ratios: sizes.clone(),
            children: children.iter().map(capture_node).collect(),
        },
    }
}

fn capture_surface(s: &Surface) -> SurfaceSnapshot {
    let title_is_custom = !is_default_surface_title(&s.title);
    SurfaceSnapshot {
        id: s.id.0,
        title: s.title.clone(),
        title_is_custom,
        cwd: s.terminal.working_directory().map(|p| p.to_string_lossy().into_owned()),
        scrollback: None,
    }
}

fn is_default_surface_title(title: &str) -> bool {
    title.is_empty()
        || title
            .strip_prefix("Terminal ")
            .is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit()))
}

// ── Restore ────────────────────────────────────────────────────────────────

/// Options controlling how terminals are spawned during restore.
pub struct RestoreOptions {
    pub cols: u16,
    pub rows: u16,
    pub font_size: f32,
    pub theme: rmux_terminal::NamedTheme,
}

impl Default for RestoreOptions {
    fn default() -> Self {
        Self {
            cols: 80,
            rows: 24,
            font_size: crate::ui::DEFAULT_FONT_SIZE,
            theme: rmux_terminal::NamedTheme::default(),
        }
    }
}

/// Rebuild a [`WorkspaceManager`] from a snapshot, spawning shells in saved cwds.
pub fn restore_session(
    snapshot: &SessionSnapshot,
    opts: &RestoreOptions,
) -> Result<WorkspaceManager, SessionError> {
    if !snapshot.is_usable() {
        return Err(SessionError::Restore("snapshot is empty or wrong version".into()));
    }

    let mut max_ws = 0_u64;
    let mut max_pane = 0_u64;
    let mut max_split = 0_u64;
    for ws in &snapshot.workspaces {
        max_ws = max_ws.max(ws.id);
        walk_id_max(&ws.layout, &mut max_pane, &mut max_split);
    }

    let mut workspaces = Vec::with_capacity(snapshot.workspaces.len());
    for ws_snap in &snapshot.workspaces {
        let root = restore_node(&ws_snap.layout, opts)?;
        // Ensure active pane exists in the tree; fall back to first leaf.
        let active = if root.find_pane(PaneId(ws_snap.active_pane_id)).is_some() {
            PaneId(ws_snap.active_pane_id)
        } else {
            first_leaf_id(&root).unwrap_or(PaneId(ws_snap.active_pane_id))
        };
        let zoomed = ws_snap
            .zoomed_pane_id
            .and_then(|z| if root.find_pane(PaneId(z)).is_some() { Some(PaneId(z)) } else { None });
        let mut ws = Workspace {
            id: WorkspaceId(ws_snap.id),
            name: ws_snap.name.clone(),
            name_is_custom: ws_snap.name_is_custom,
            process_title: if ws_snap.process_title.is_empty() {
                ws_snap.name.clone()
            } else {
                ws_snap.process_title.clone()
            },
            path_context: None,
            path_contexts: Vec::new(),
            pull_request: None,
            shows_agent_activity: false,
            root,
            active_pane: active,
            status: ws_snap.status.clone(),
            progress: ws_snap.progress,
            git_branch: None,
            git_status: None,
            ports: Vec::new(),
            zoomed_pane: zoomed,
            next_surface_id: ws_snap.next_surface_id.max(2),
        };
        // Bump next_surface_id past any restored surface ids.
        let mut max_surface = ws.next_surface_id;
        bump_surface_counter(&ws.root, &mut max_surface);
        ws.next_surface_id = max_surface;
        workspaces.push(ws);
    }

    let active_index = snapshot.active_workspace_index.min(workspaces.len().saturating_sub(1));

    Ok(WorkspaceManager::from_restored(
        workspaces,
        active_index,
        max_ws.saturating_add(1).max(1),
        max_pane.saturating_add(1).max(1),
        max_split.saturating_add(1).max(1),
    ))
}

fn walk_id_max(node: &NodeSnapshot, max_pane: &mut u64, max_split: &mut u64) {
    match node {
        NodeSnapshot::Leaf { id, .. } | NodeSnapshot::Browser { id, .. } => {
            *max_pane = (*max_pane).max(*id);
        }
        NodeSnapshot::Split { id, children, .. } => {
            *max_split = (*max_split).max(*id);
            for c in children {
                walk_id_max(c, max_pane, max_split);
            }
        }
    }
}

fn bump_surface_counter(node: &PaneNode, max_surface: &mut u64) {
    match node {
        PaneNode::Leaf { surfaces, .. } => {
            for s in surfaces {
                *max_surface = (*max_surface).max(s.id.0.saturating_add(1));
            }
        }
        PaneNode::Browser { .. } => {}
        PaneNode::Split { children, .. } => {
            for c in children {
                bump_surface_counter(c, max_surface);
            }
        }
    }
}

fn first_leaf_id(node: &PaneNode) -> Option<PaneId> {
    match node {
        PaneNode::Leaf { id, .. } | PaneNode::Browser { id, .. } => Some(*id),
        PaneNode::Split { children, .. } => children.iter().find_map(first_leaf_id),
    }
}

fn restore_node(node: &NodeSnapshot, opts: &RestoreOptions) -> Result<PaneNode, SessionError> {
    match node {
        NodeSnapshot::Leaf { id, active_surface, surfaces } => {
            let mut live_surfaces = Vec::with_capacity(surfaces.len().max(1));
            if surfaces.is_empty() {
                let term = spawn_terminal(None, opts)?;
                live_surfaces.push(Surface::new(SurfaceId(1), "Terminal 1", term));
            } else {
                for s in surfaces {
                    let cwd = s.cwd.as_ref().map(PathBuf::from);
                    let cwd_ref = cwd.as_deref().filter(|p| p.is_dir());
                    let term = spawn_terminal(cwd_ref, opts)?;
                    let title = if s.title.is_empty() {
                        format!("Terminal {}", s.id)
                    } else {
                        s.title.clone()
                    };
                    live_surfaces.push(Surface::new(SurfaceId(s.id), title, term));
                }
            }
            let active = (*active_surface).min(live_surfaces.len().saturating_sub(1));
            Ok(PaneNode::Leaf {
                id: PaneId(*id),
                terminal: Box::new(None),
                active_surface: active,
                surfaces: live_surfaces,
            })
        }
        NodeSnapshot::Browser { id, url, .. } => {
            let mut browser = BrowserPane::new();
            if !url.is_empty() && url != "about:blank" {
                let _ = browser.navigate(url);
            }
            browser.set_open(true);
            Ok(PaneNode::new_browser(PaneId(*id), browser))
        }
        NodeSnapshot::Split { id, direction, ratios, children } => {
            if children.is_empty() {
                return Err(SessionError::Restore("split with no children".into()));
            }
            let mut restored = Vec::with_capacity(children.len());
            for c in children {
                restored.push(restore_node(c, opts)?);
            }
            let mut sizes = ratios.clone();
            if sizes.len() != restored.len() {
                let n = restored.len() as f32;
                sizes = vec![1.0 / n; restored.len()];
            } else {
                // Normalize ratios so they sum to ~1.
                let sum: f32 = sizes.iter().sum();
                if sum > f32::EPSILON {
                    for s in &mut sizes {
                        *s /= sum;
                    }
                }
            }
            Ok(PaneNode::Split {
                id: SplitId(*id),
                direction: (*direction).into(),
                children: restored,
                sizes,
            })
        }
    }
}

fn spawn_terminal(cwd: Option<&Path>, opts: &RestoreOptions) -> Result<TerminalPane, SessionError> {
    let mut term = TerminalPane::spawn_with_cwd(opts.cols, opts.rows, opts.font_size, cwd)
        .map_err(|e| SessionError::Restore(format!("PTY spawn failed: {e}")))?;
    term.set_theme(rmux_terminal::TerminalTheme::default().named(opts.theme));
    Ok(term)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_store() -> (SessionStore, PathBuf) {
        let n = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("rmux-session-test-{n}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        (SessionStore::at(&dir), dir)
    }

    #[test]
    fn test_snapshot_roundtrip_json() {
        let snap = SessionSnapshot {
            version: SESSION_SCHEMA_VERSION,
            created_at: 1.0,
            window: WindowSnapshot::default(),
            active_workspace_index: 0,
            workspaces: vec![WorkspaceSnapshot {
                id: 1,
                name: "main · ~/proj".into(),
                name_is_custom: false,
                process_title: "main · ~/proj".into(),
                active_pane_id: 1,
                zoomed_pane_id: None,
                layout: NodeSnapshot::Leaf {
                    id: 1,
                    active_surface: 0,
                    surfaces: vec![SurfaceSnapshot {
                        id: 1,
                        title: "Terminal 1".into(),
                        title_is_custom: false,
                        cwd: Some("/tmp".into()),
                        scrollback: None,
                    }],
                },
                status: None,
                progress: None,
                next_surface_id: 2,
            }],
        };
        let json = serde_json::to_string_pretty(&snap).unwrap();
        let back: SessionSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back, snap);
        assert!(back.is_usable());
    }

    #[test]
    fn test_empty_snapshot_not_usable() {
        let snap = SessionSnapshot {
            version: SESSION_SCHEMA_VERSION,
            created_at: 0.0,
            window: WindowSnapshot::default(),
            active_workspace_index: 0,
            workspaces: vec![],
        };
        assert!(!snap.is_usable());
    }

    #[test]
    fn test_store_save_load_and_previous() {
        let (store, dir) = temp_store();
        let snap = SessionSnapshot {
            version: SESSION_SCHEMA_VERSION,
            created_at: 2.0,
            window: WindowSnapshot { sidebar_visible: false, ..WindowSnapshot::default() },
            active_workspace_index: 0,
            workspaces: vec![WorkspaceSnapshot {
                id: 3,
                name: "ws".into(),
                name_is_custom: true,
                process_title: "ws".into(),
                active_pane_id: 5,
                zoomed_pane_id: None,
                layout: NodeSnapshot::Split {
                    id: 10,
                    direction: SplitDirectionSnap::Horizontal,
                    ratios: vec![0.4, 0.6],
                    children: vec![
                        NodeSnapshot::Leaf {
                            id: 5,
                            active_surface: 0,
                            surfaces: vec![SurfaceSnapshot {
                                id: 1,
                                title: "a".into(),
                                title_is_custom: true,
                                cwd: None,
                                scrollback: None,
                            }],
                        },
                        NodeSnapshot::Browser {
                            id: 6,
                            url: "https://example.com".into(),
                            title: None,
                        },
                    ],
                },
                status: Some("ok".into()),
                progress: Some(0.5),
                next_surface_id: 2,
            }],
        };
        store.save(&snap).unwrap();
        assert!(store.primary_path().exists());
        assert!(store.previous_path().exists());
        match store.load_startup() {
            LoadOutcome::Loaded(loaded) => {
                assert_eq!(loaded.workspaces[0].name, "ws");
                assert!(!loaded.window.sidebar_visible);
                assert_eq!(loaded.workspaces[0].active_pane_id, 5);
            }
            other => panic!("expected loaded, got {other:?}"),
        }
        let prev = store.load_previous().expect("previous");
        assert_eq!(prev.workspaces[0].id, 3);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_unusable_primary_falls_back_to_previous() {
        let (store, dir) = temp_store();
        let good = SessionSnapshot {
            version: SESSION_SCHEMA_VERSION,
            created_at: 1.0,
            window: WindowSnapshot::default(),
            active_workspace_index: 0,
            workspaces: vec![WorkspaceSnapshot {
                id: 1,
                name: "good".into(),
                name_is_custom: false,
                process_title: "good".into(),
                active_pane_id: 1,
                zoomed_pane_id: None,
                layout: NodeSnapshot::Leaf {
                    id: 1,
                    active_surface: 0,
                    surfaces: vec![SurfaceSnapshot {
                        id: 1,
                        title: "t".into(),
                        title_is_custom: false,
                        cwd: None,
                        scrollback: None,
                    }],
                },
                status: None,
                progress: None,
                next_surface_id: 2,
            }],
        };
        store.save(&good).unwrap();
        // Corrupt primary
        fs::write(store.primary_path(), b"{not json").unwrap();
        match store.load_startup() {
            LoadOutcome::Loaded(s) => assert_eq!(s.workspaces[0].name, "good"),
            other => panic!("expected fallback loaded, got {other:?}"),
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wrong_version_unusable() {
        let (store, dir) = temp_store();
        let mut bad = SessionSnapshot {
            version: 99,
            created_at: 0.0,
            window: WindowSnapshot::default(),
            active_workspace_index: 0,
            workspaces: vec![WorkspaceSnapshot {
                id: 1,
                name: "x".into(),
                name_is_custom: false,
                process_title: "x".into(),
                active_pane_id: 1,
                zoomed_pane_id: None,
                layout: NodeSnapshot::Leaf { id: 1, active_surface: 0, surfaces: vec![] },
                status: None,
                progress: None,
                next_surface_id: 1,
            }],
        };
        // Write raw so is_usable fails on load
        fs::create_dir_all(store.dir()).unwrap();
        fs::write(store.primary_path(), serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(matches!(store.load_startup(), LoadOutcome::Missing | LoadOutcome::Unusable));
        bad.version = SESSION_SCHEMA_VERSION;
        let _ = bad;
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_capture_restore_layout_roundtrip() {
        // Capture from a live manager (single default workspace), then restore
        // and compare structural fields (ids may rematerialize the same).
        let manager = WorkspaceManager::new();
        // Attach a real terminal so cwd capture works.
        let pane = manager.active().active_pane;
        let mut manager = manager;
        if let Ok(mut term) = TerminalPane::spawn_with_cwd(40, 12, 14.0, Some(Path::new("/tmp"))) {
            term.set_theme(rmux_terminal::TerminalTheme::default());
            manager.active_mut().set_terminal(pane, term);
        }
        let snap = capture_session(
            &manager,
            CaptureUiState {
                inner_size: [800.0, 600.0],
                sidebar_visible: true,
                right_sidebar_visible: false,
                notification_panel_visible: false,
            },
        );
        assert!(!snap.workspaces.is_empty());
        assert_eq!(snap.window.inner_size, [800.0, 600.0]);

        let opts = RestoreOptions { cols: 40, rows: 12, font_size: 14.0, ..Default::default() };
        let restored = restore_session(&snap, &opts).expect("restore");
        assert_eq!(restored.workspace_count(), snap.workspaces.len());
        assert_eq!(restored.active().name, snap.workspaces[0].name);
        // Layout should have at least one leaf with a surface.
        assert!(restored.active().root.find_pane(restored.active().active_pane).is_some());
    }

    #[test]
    fn test_split_direction_serde() {
        let n = NodeSnapshot::Split {
            id: 1,
            direction: SplitDirectionSnap::Vertical,
            ratios: vec![0.5, 0.5],
            children: vec![
                NodeSnapshot::Leaf { id: 2, active_surface: 0, surfaces: vec![] },
                NodeSnapshot::Leaf { id: 3, active_surface: 0, surfaces: vec![] },
            ],
        };
        let j = serde_json::to_string(&n).unwrap();
        assert!(j.contains("vertical"));
        let back: NodeSnapshot = serde_json::from_str(&j).unwrap();
        assert_eq!(back, n);
    }

    #[test]
    fn test_fingerprint_ignores_created_at() {
        let mut a = SessionSnapshot {
            version: SESSION_SCHEMA_VERSION,
            created_at: 1.0,
            window: WindowSnapshot::default(),
            active_workspace_index: 0,
            workspaces: vec![WorkspaceSnapshot {
                id: 1,
                name: "n".into(),
                name_is_custom: false,
                process_title: "n".into(),
                active_pane_id: 1,
                zoomed_pane_id: None,
                layout: NodeSnapshot::Leaf { id: 1, active_surface: 0, surfaces: vec![] },
                status: None,
                progress: None,
                next_surface_id: 1,
            }],
        };
        let mut b = a.clone();
        b.created_at = 999.0;
        assert_eq!(a.content_fingerprint(), b.content_fingerprint());
        a.workspaces[0].name = "other".into();
        assert_ne!(a.content_fingerprint(), b.content_fingerprint());
    }
}
