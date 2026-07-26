//! cmux-parity workspace sidebar row snapshot.
//!
//! Mirrors the data model behind cmux's `SidebarWorkspaceSnapshotBuilder.Snapshot`
//! and `SidebarWorkspaceRowModel` — only the fields we can fill without Ghostty /
//! their full GitHub probe service. The sidebar paints slots in the same order
//! as cmux's `SidebarWorkspaceRowCellView.applyModel`:
//!
//! 1. Title (+ unread badge / close)
//! 2. Notification subtitle (`latestNotificationText`)
//! 3. Progress bar
//! 4. Path lines — one `branch · dir` per distinct terminal cwd (cmux multi-row)
//! 5. Pull-request row (best-effort via `gh`)
//! 6. Listening ports

use std::path::Path;
use std::process::Command;

use super::title::{MAX_WORKSPACE_TITLE_CHARS, compose_auto_title};

/// Pull-request chip shown under a workspace row (cmux `PullRequestDisplay`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestDisplay {
    pub number: u32,
    pub label: String,
    pub url: String,
    pub is_open: bool,
}

/// Immutable render value for one sidebar workspace card (cmux row snapshot).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceSidebarSnapshot {
    /// Primary title (process / path / custom).
    pub title: String,
    /// `latestNotificationText` — unread notification preview.
    pub latest_notification: Option<String>,
    /// Explicit API status (`sidebar.set_status`), used as progress label.
    pub status: Option<String>,
    /// `0.0..=1.0` progress bar.
    pub progress: Option<f32>,
    /// Git branch for the focused / primary path line (`showsGitBranch`).
    pub git_branch: Option<String>,
    /// Short directory candidates for the primary branch line (longest first).
    pub directory_candidates: Vec<String>,
    /// Unique path lines from all terminals (`branch · dir`), focused first.
    ///
    /// cmux shows one muted row per distinct pane path under the title —
    /// not only the focused terminal.
    pub path_lines: Vec<String>,
    /// Compact primary `branch · dir` (first of [`Self::path_lines`]) for
    /// older layout helpers.
    pub branch_directory_text: Option<String>,
    /// Best-effort PR chip from `gh`.
    pub pull_request: Option<PullRequestDisplay>,
    /// Listening ports (cmux port chips).
    pub ports: Vec<u16>,
    /// Unread notification count.
    pub unread_count: usize,
    /// True when a coding agent is the foreground process on any pane.
    pub shows_agent_activity: bool,
}

/// Max path rows under a workspace card (cmux-style cap so the list stays
/// scannable when a workspace has many splits/tabs).
pub const MAX_PATH_LINES: usize = 6;

impl WorkspaceSidebarSnapshot {
    /// Build a snapshot from live workspace + terminal aggregates.
    #[allow(clippy::too_many_arguments)] // mirrors cmux Snapshot field pack
    pub fn build(
        title: impl Into<String>,
        status: Option<&str>,
        progress: Option<f32>,
        ports: &[u16],
        unread: usize,
        latest_notification: Option<&str>,
        git_branch: Option<&str>,
        cwd: Option<&Path>,
        fg_command: Option<&str>,
        pull_request: Option<PullRequestDisplay>,
        path_lines: &[String],
    ) -> Self {
        let title = title.into();
        let directory_candidates = directory_candidates_for(cwd);

        // Prefer pre-collected multi-pane lines; fall back to single focused
        // idle path when the collector returned nothing yet.
        let mut path_lines: Vec<String> = path_lines
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| truncate(s, MAX_WORKSPACE_TITLE_CHARS))
            .collect();

        if path_lines.is_empty() {
            let path_ctx = compose_auto_title(None, cwd, git_branch);
            if !path_ctx.is_empty() && path_ctx != "Terminal" {
                path_lines.push(path_ctx);
            }
        }

        // Drop lines that exactly match the primary title (avoids
        // `main · ~/x` under a title that is already that string). Keep them
        // when there are *multiple* path rows — the first still orients the
        // user even if it matches a path-style title.
        if path_lines.len() == 1 && path_lines[0] == title {
            // Single duplicate of title → hide dedicated path slot.
            path_lines.clear();
        }

        let branch_directory_text = path_lines.first().cloned();

        Self {
            title,
            latest_notification: latest_notification
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| truncate(s, MAX_WORKSPACE_TITLE_CHARS)),
            status: status
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| truncate(s, MAX_WORKSPACE_TITLE_CHARS)),
            progress: progress.filter(|p| p.is_finite()),
            git_branch: git_branch
                .map(str::trim)
                .filter(|s| !s.is_empty() && *s != "HEAD")
                .map(str::to_string),
            directory_candidates,
            path_lines,
            branch_directory_text,
            pull_request,
            ports: ports.to_vec(),
            unread_count: unread,
            shows_agent_activity: fg_command.is_some_and(is_coding_agent_command),
        }
    }

    /// Whether any path line should paint under the title.
    pub fn shows_branch_line(&self) -> bool {
        !self.path_lines.is_empty()
    }

    /// Path rows to paint (already deduped / capped).
    pub fn visible_path_lines(&self) -> &[String] {
        &self.path_lines
    }

    /// Whether any auxiliary slot below the title is visible.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn has_auxiliary_slots(&self) -> bool {
        self.latest_notification.is_some()
            || self.progress.is_some()
            || self.shows_branch_line()
            || self.pull_request.is_some()
            || !self.ports.is_empty()
            || self.shows_agent_activity
    }
}

/// Ordered directory display candidates: longest → shortest (cmux
/// `directoryCandidates` / ViewThatFits).
fn directory_candidates_for(cwd: Option<&Path>) -> Vec<String> {
    let Some(cwd) = cwd else {
        return Vec::new();
    };
    let full = crate::ui::format_cwd_tab_title(cwd);
    let mut out = vec![full.clone()];
    // Shorter fallbacks: last 2 components, then basename.
    let components: Vec<&str> = full.split('/').filter(|s| !s.is_empty()).collect();
    if components.len() >= 2 {
        let short = format!("…/{}", components[components.len() - 2..].join("/"));
        if short != full {
            out.push(short);
        }
    }
    if let Some(base) = components.last() {
        let base = (*base).to_string();
        if base != full && !out.contains(&base) {
            out.push(base);
        }
    }
    out
}

/// Best-effort PR for `cwd` via GitHub CLI (cmux pull-request probe lite).
pub fn pull_request_for_cwd(cwd: &Path) -> Option<PullRequestDisplay> {
    if !cwd.is_dir() {
        return None;
    }
    let output = Command::new("gh")
        .args(["pr", "view", "--json", "number,url,title,state"])
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let number = v.get("number")?.as_u64()? as u32;
    let url = v.get("url")?.as_str()?.to_string();
    let state = v.get("state").and_then(|s| s.as_str()).unwrap_or("");
    let is_open = state.eq_ignore_ascii_case("OPEN");
    let title = v.get("title").and_then(|s| s.as_str()).unwrap_or("");
    let label = if title.is_empty() {
        format!("PR #{number}")
    } else {
        truncate(&format!("PR #{number} · {title}"), MAX_WORKSPACE_TITLE_CHARS)
    };
    Some(PullRequestDisplay { number, label, url, is_open })
}

pub fn is_coding_agent_command(cmd: &str) -> bool {
    let token = cmd.split_whitespace().next().unwrap_or(cmd);
    let base = token.rsplit('/').next().unwrap_or(token).to_ascii_lowercase();
    matches!(
        base.as_str(),
        "claude"
            | "codex"
            | "cursor"
            | "gemini"
            | "grok"
            | "aider"
            | "continue"
            | "windsurf"
            | "amp"
            | "opencode"
    ) || base.contains("claude")
        || base.contains("codex")
}

fn truncate(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    if max <= 1 {
        return "…".to_string();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

/// Build unique idle path lines from a sequence of `(cwd, branch)` pairs.
///
/// `focused_first` — when set, that pair is inserted first (cmux puts the
/// active surface's path at the top of the metadata stack).
pub fn unique_path_lines(
    focused: Option<(Option<&Path>, Option<&str>)>,
    others: impl IntoIterator<Item = (Option<std::path::PathBuf>, Option<String>)>,
    max: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut push = |cwd: Option<&Path>, branch: Option<&str>| {
        if lines.len() >= max {
            return;
        }
        let line = compose_auto_title(None, cwd, branch);
        if line.is_empty() || line == "Terminal" {
            return;
        }
        if !lines.iter().any(|existing| existing == &line) {
            lines.push(truncate(&line, MAX_WORKSPACE_TITLE_CHARS));
        }
    };

    if let Some((cwd, branch)) = focused {
        push(cwd, branch);
    }
    for (cwd, branch) in others {
        push(cwd.as_deref(), branch.as_deref());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_snapshot_shows_path_when_title_is_process() {
        let snap = WorkspaceSidebarSnapshot::build(
            "cargo run -p rmux",
            None,
            None,
            &[],
            0,
            None,
            Some("main"),
            Some(Path::new("/tmp/rmux")),
            Some("cargo run -p rmux"),
            None,
            &[],
        );
        assert!(snap.shows_branch_line(), "process title should show path line");
        assert!(!snap.path_lines.is_empty());
    }

    #[test]
    fn test_snapshot_hides_single_duplicate_path_line() {
        let path_title = compose_auto_title(None, Some(Path::new("/tmp/rmux")), Some("main"));
        let snap = WorkspaceSidebarSnapshot::build(
            path_title.clone(),
            None,
            None,
            &[],
            0,
            None,
            Some("main"),
            Some(Path::new("/tmp/rmux")),
            None,
            None,
            std::slice::from_ref(&path_title),
        );
        assert!(!snap.shows_branch_line(), "single path line equal to title should hide");
    }

    #[test]
    fn test_snapshot_keeps_multiple_path_lines() {
        let lines =
            vec!["main · ~/a".to_string(), "feat/x · ~/b".to_string(), "feat/y · ~/c".to_string()];
        let snap = WorkspaceSidebarSnapshot::build(
            "custom workspace",
            None,
            None,
            &[],
            0,
            None,
            Some("main"),
            Some(Path::new("/tmp/a")),
            None,
            None,
            &lines,
        );
        assert_eq!(snap.path_lines.len(), 3);
        assert!(snap.shows_branch_line());
    }

    #[test]
    fn test_unique_path_lines_dedupes_and_prefers_focused() {
        let focused_cwd = PathBuf::from("/Users/me/proj-a");
        let other_cwd = PathBuf::from("/Users/me/proj-b");
        let lines = unique_path_lines(
            Some((Some(focused_cwd.as_path()), Some("main"))),
            [
                (Some(other_cwd.clone()), Some("feat/x".to_string())),
                (Some(focused_cwd.clone()), Some("main".to_string())), // dup of focused
                (Some(other_cwd), Some("feat/x".to_string())),         // dup
            ],
            MAX_PATH_LINES,
        );
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("main") || lines[0].contains("proj-a"));
    }

    #[test]
    fn test_agent_detection() {
        assert!(is_coding_agent_command("claude --resume"));
        assert!(is_coding_agent_command("/usr/local/bin/codex"));
        assert!(!is_coding_agent_command("cargo build"));
    }

    #[test]
    fn test_directory_candidates_shorten() {
        let deep = PathBuf::from("/Users/me/Developer/PersonalProjects/rmux");
        let c = directory_candidates_for(Some(&deep));
        assert!(!c.is_empty());
    }

    #[test]
    fn test_notification_slot() {
        let snap = WorkspaceSidebarSnapshot::build(
            "main · ~/x",
            None,
            None,
            &[],
            2,
            Some("Claude is waiting for your input"),
            Some("main"),
            Some(Path::new("/tmp/x")),
            None,
            None,
            &["feat/other · ~/y".to_string()],
        );
        assert_eq!(snap.latest_notification.as_deref(), Some("Claude is waiting for your input"));
        assert_eq!(snap.unread_count, 2);
        assert!(snap.has_auxiliary_slots());
        assert_eq!(snap.path_lines.len(), 1);
    }
}
