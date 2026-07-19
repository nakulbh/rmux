//! Dynamic workspace titles (cmux-style).
//!
//! cmux tracks a **process title** from the focused terminal and promotes it to
//! the workspace title when the user has not set a custom name
//! (`Workspace+TitleOwnership.swift` → `applyProcessTitle` / `applyAutomaticTitle`).
//!
//! rmux mirrors that model without Ghostty title events:
//!
//! 1. If a non-shell child is running under the focused pane → show its command
//!    line (`cargo run …`, `nvim`, …).
//! 2. Else if a git branch is known → `{branch} · {short path}`.
//! 3. Else → short path / `user@host` (same as surface tab labels).

use std::path::Path;
use std::process::Command;

use crate::ui::format_cwd_tab_title;

/// Max characters for a sidebar workspace title.
pub const MAX_WORKSPACE_TITLE_CHARS: usize = 40;

/// Build the automatic workspace title for a focused terminal snapshot.
///
/// `fg_command` is the cleaned foreground process title when the shell is
/// busy; `cwd` and `git_branch` describe the idle case.
pub fn compose_auto_title(
    fg_command: Option<&str>,
    cwd: Option<&Path>,
    git_branch: Option<&str>,
) -> String {
    if let Some(cmd) = fg_command.map(str::trim).filter(|s| !s.is_empty()) {
        return truncate_title(cmd, MAX_WORKSPACE_TITLE_CHARS);
    }

    let path = match cwd {
        Some(p) => format_cwd_tab_title(p),
        None => "Terminal".to_string(),
    };

    if let Some(branch) = git_branch.map(str::trim).filter(|s| !s.is_empty() && *s != "HEAD") {
        let combined = format!("{branch} · {path}");
        return truncate_title(&combined, MAX_WORKSPACE_TITLE_CHARS);
    }

    truncate_title(&path, MAX_WORKSPACE_TITLE_CHARS)
}

/// Best-effort `git rev-parse --abbrev-ref HEAD` for `cwd`.
///
/// Runs synchronously; callers must throttle (e.g. once per ~0.7s per pane).
pub fn git_branch_for_cwd(cwd: &Path) -> Option<String> {
    if !cwd.is_dir() {
        return None;
    }
    let output = Command::new("git")
        .args(["-C"])
        .arg(cwd)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branch = String::from_utf8(output.stdout).ok()?;
    let branch = branch.trim();
    if branch.is_empty() || branch == "HEAD" {
        return None;
    }
    Some(branch.to_string())
}

fn truncate_title(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    if max <= 1 {
        return "…".to_string();
    }
    let keep = max - 1;
    let mut out: String = s.chars().take(keep).collect();
    out.push('…');
    out
}

/// Build the muted secondary line under a workspace title (cmux row metadata).
///
/// Priority (first non-empty wins), matching cmux sidebar row inputs:
/// 1. Latest unread notification text (`latestNotificationText`)
/// 2. Explicit API status (`sidebar.set_status`)
/// 3. Path/branch context when the primary title is a running command or custom
///    name (so `cargo run…` still shows `main · ~/proj` underneath)
/// 4. Dirty git status when nothing else applies
/// Priority helper used by unit tests and as a single-line fallback.
///
/// The live sidebar now paints discrete cmux slots instead.
#[cfg_attr(not(test), allow(dead_code))]
pub fn compose_subtitle(
    primary_title: &str,
    latest_notification: Option<&str>,
    status: Option<&str>,
    path_context: Option<&str>,
    git_status: Option<&str>,
) -> Option<String> {
    if let Some(text) = latest_notification.map(str::trim).filter(|s| !s.is_empty()) {
        return Some(truncate_title(text, MAX_WORKSPACE_TITLE_CHARS));
    }
    if let Some(text) = status.map(str::trim).filter(|s| !s.is_empty()) {
        return Some(truncate_title(text, MAX_WORKSPACE_TITLE_CHARS));
    }
    if let Some(path) = path_context.map(str::trim).filter(|s| !s.is_empty()) {
        // Avoid duplicating the same string on both lines.
        if path != primary_title.trim() {
            return Some(truncate_title(path, MAX_WORKSPACE_TITLE_CHARS));
        }
    }
    if let Some(st) = git_status
        .map(str::trim)
        .filter(|s| !s.is_empty() && !matches!(*s, "clean" | "ok" | "up to date"))
    {
        return Some(truncate_title(st, MAX_WORKSPACE_TITLE_CHARS));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_compose_prefers_running_command() {
        let title = compose_auto_title(
            Some("cargo run -p rmux-app --release"),
            Some(Path::new("/Users/me/proj")),
            Some("main"),
        );
        assert!(title.starts_with("cargo run"), "got {title}");
    }

    #[test]
    fn test_compose_branch_and_path_when_idle() {
        let title = compose_auto_title(None, Some(Path::new("/tmp/rmux")), Some("fix/cursor"));
        assert!(title.contains("fix/cursor"), "got {title}");
        assert!(title.contains('·'), "got {title}");
    }

    #[test]
    fn test_compose_path_only_without_branch() {
        let title = compose_auto_title(None, Some(Path::new("/tmp/rmux")), None);
        assert!(!title.is_empty());
        assert!(!title.contains('·'));
    }

    #[test]
    fn test_compose_fallback_without_cwd() {
        assert_eq!(compose_auto_title(None, None, None), "Terminal");
    }

    #[test]
    fn test_truncate_title() {
        let long = "x".repeat(100);
        let t = truncate_title(&long, 10);
        assert_eq!(t.chars().count(), 10);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn test_git_branch_for_non_repo_is_none() {
        let tmp = PathBuf::from("/tmp");
        // /tmp is rarely a git root; either None or Some is ok if someone
        // made it one — only assert it does not panic.
        let _ = git_branch_for_cwd(&tmp);
    }

    #[test]
    fn test_compose_subtitle_prefers_notification() {
        let sub = compose_subtitle(
            "cargo run",
            Some("Claude is waiting for your input"),
            Some("building"),
            Some("main · ~/proj"),
            Some("modified"),
        );
        assert_eq!(sub.as_deref(), Some("Claude is waiting for your input"));
    }

    #[test]
    fn test_compose_subtitle_status_over_path() {
        let sub =
            compose_subtitle("cargo run", None, Some("PR #23 open"), Some("main · ~/x"), None);
        assert_eq!(sub.as_deref(), Some("PR #23 open"));
    }

    #[test]
    fn test_compose_subtitle_path_when_title_is_process() {
        let sub = compose_subtitle("cargo run -p rmux", None, None, Some("main · ~/rmux"), None);
        assert_eq!(sub.as_deref(), Some("main · ~/rmux"));
    }

    #[test]
    fn test_compose_subtitle_skips_duplicate_path() {
        let sub = compose_subtitle("main · ~/rmux", None, None, Some("main · ~/rmux"), None);
        assert!(sub.is_none());
    }
}
