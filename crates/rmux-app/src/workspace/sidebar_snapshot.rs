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
//! 4. Branch · directory line
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
    /// Git branch for the branch line (`showsGitBranch`).
    pub git_branch: Option<String>,
    /// Short directory candidates for the branch line (longest first).
    pub directory_candidates: Vec<String>,
    /// Compact `branch · dir` string when both are known.
    pub branch_directory_text: Option<String>,
    /// Best-effort PR chip from `gh`.
    pub pull_request: Option<PullRequestDisplay>,
    /// Listening ports (cmux port chips).
    pub ports: Vec<u16>,
    /// Unread notification count.
    pub unread_count: usize,
    /// True when a coding agent is the foreground process (spinner slot).
    pub shows_agent_activity: bool,
}

impl WorkspaceSidebarSnapshot {
    /// Build a snapshot from live workspace + focused terminal caches.
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
    ) -> Self {
        let title = title.into();
        let directory_candidates = directory_candidates_for(cwd);
        let branch_directory_text = {
            let path_ctx = compose_auto_title(None, cwd, git_branch);
            if path_ctx.is_empty() || path_ctx == "Terminal" {
                None
            } else if path_ctx == title {
                // Title already is the branch·path line — still keep structured
                // fields for layout, but the dedicated branch slot can hide.
                Some(path_ctx)
            } else {
                Some(path_ctx)
            }
        };

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
            branch_directory_text,
            pull_request,
            ports: ports.to_vec(),
            unread_count: unread,
            shows_agent_activity: fg_command.is_some_and(is_coding_agent_command),
        }
    }

    /// Whether the dedicated branch/dir slot should paint under the title.
    ///
    /// cmux always has a separate branch section when enabled; we hide it only
    /// when it would duplicate the primary title with no extra signal.
    pub fn shows_branch_line(&self) -> bool {
        match self.branch_directory_text.as_deref() {
            Some(text) if text != self.title => true,
            // Still show branch icon line when we have a branch but title is process.
            Some(_) if self.git_branch.is_some() && self.shows_process_like_title() => true,
            _ => false,
        }
    }

    fn shows_process_like_title(&self) -> bool {
        // Heuristic: path titles usually contain `·`, `~`, or `/`.
        let t = self.title.as_str();
        !(t.contains('·') || t.starts_with('~') || t.starts_with('/'))
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

fn is_coding_agent_command(cmd: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_snapshot_shows_branch_when_title_is_process() {
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
        );
        assert!(snap.shows_branch_line(), "process title should show branch line");
        assert!(snap.branch_directory_text.is_some());
    }

    #[test]
    fn test_snapshot_hides_duplicate_branch_line() {
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
        );
        // Title already is branch·path — dedicated line hidden.
        assert!(
            !snap.shows_branch_line() || snap.branch_directory_text.as_ref() == Some(&path_title)
        );
        if snap.branch_directory_text.as_ref() == Some(&path_title) {
            assert!(!snap.shows_branch_line());
        }
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
        );
        assert_eq!(snap.latest_notification.as_deref(), Some("Claude is waiting for your input"));
        assert_eq!(snap.unread_count, 2);
        assert!(snap.has_auxiliary_slots());
    }
}
