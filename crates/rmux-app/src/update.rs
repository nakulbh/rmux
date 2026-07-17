//! GitHub-backed update check for rmux.
//!
//! # Strategy (hybrid)
//!
//! 1. Query **latest GitHub Release** (`/repos/{owner}/{repo}/releases/latest`).
//!    If a newer semver tag exists than the local `CARGO_PKG_VERSION`, report
//!    [`UpdateStatus::Available`].
//! 2. If there are no releases (404) or the latest release is not newer, query
//!    the tip of **`main`** (`/repos/{owner}/{repo}/commits/main`) and compare
//!    the short SHA to the SHA embedded at build time (`RMUX_GIT_SHA`).
//!
//! Network I/O is intentionally synchronous and meant to run on a background
//! thread so the egui UI never blocks. Pure helpers (`parse_semver`,
//! `is_remote_newer`, `sha_matches`) are unit-tested without HTTP.

/// Public GitHub repository used for releases and commit checks.
pub const GITHUB_OWNER: &str = "nakulbh";
/// See [`GITHUB_OWNER`].
pub const GITHUB_REPO: &str = "rmux";
/// Human-facing releases page.
pub const RELEASES_URL: &str = "https://github.com/nakulbh/rmux/releases";
/// Install script URL (always from `main` so the installer itself is current).
///
/// Only referenced by the Unix installer path (`apply_update_unix`).
#[cfg(not(windows))]
pub const INSTALL_SCRIPT_URL: &str =
    "https://raw.githubusercontent.com/nakulbh/rmux/main/scripts/install.sh";

/// Local package version compiled into the binary.
pub fn local_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Short git SHA captured by `build.rs` (may be empty for non-git builds).
pub fn local_git_sha() -> &'static str {
    env!("RMUX_GIT_SHA")
}

/// Outcome of a single update check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    /// Local build matches or is newer than remote.
    UpToDate,
    /// A newer release tag or a newer `main` commit is available.
    Available {
        /// Remote label for the toast (`v0.2.0` or short SHA).
        remote_label: String,
        /// URL to open (release page or commit).
        url: String,
        /// How the remote was discovered.
        source: UpdateSource,
    },
    /// Network / parse failure — do not pretend the user is up to date.
    Error(String),
}

/// Which remote signal produced [`UpdateStatus::Available`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateSource {
    /// GitHub Releases tag newer than local package version.
    Release,
    /// Tip of `main` differs from the embedded build SHA.
    MainCommit,
}

/// Full result returned to the UI (status only; UI owns toast timing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateCheckOutcome {
    pub status: UpdateStatus,
}

/// Run a blocking check against the GitHub API.
///
/// Safe to call from a `std::thread`. Uses a short timeout so a hung network
/// cannot leave the spinner forever (UI also has its own max wait).
pub fn check_for_updates() -> UpdateCheckOutcome {
    check_for_updates_with(GitHubClient::default())
}

/// Injectable client for tests.
trait ReleaseClient {
    fn latest_release(&self) -> Result<Option<RemoteRelease>, String>;
    fn main_commit_sha(&self) -> Result<String, String>;
}

#[derive(Debug, Clone)]
struct RemoteRelease {
    tag: String,
    html_url: String,
}

/// Default HTTP client hitting `api.github.com`.
struct GitHubClient {
    owner: String,
    repo: String,
}

impl Default for GitHubClient {
    fn default() -> Self {
        Self { owner: GITHUB_OWNER.into(), repo: GITHUB_REPO.into() }
    }
}

impl ReleaseClient for GitHubClient {
    fn latest_release(&self) -> Result<Option<RemoteRelease>, String> {
        let url =
            format!("https://api.github.com/repos/{}/{}/releases/latest", self.owner, self.repo);
        let body = http_get_json(&url)?;
        // 404 is represented as an error body with message "Not Found".
        if body.get("message").and_then(|m| m.as_str()) == Some("Not Found") {
            return Ok(None);
        }
        let tag = body
            .get("tag_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "release response missing tag_name".to_string())?
            .to_string();
        let html_url =
            body.get("html_url").and_then(|v| v.as_str()).unwrap_or(RELEASES_URL).to_string();
        Ok(Some(RemoteRelease { tag, html_url }))
    }

    fn main_commit_sha(&self) -> Result<String, String> {
        let url = format!("https://api.github.com/repos/{}/{}/commits/main", self.owner, self.repo);
        let body = http_get_json(&url)?;
        let sha = body
            .get("sha")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "commit response missing sha".to_string())?;
        Ok(short_sha(sha))
    }
}

fn http_get_json(url: &str) -> Result<serde_json::Value, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(12))
        .user_agent(&format!(
            "rmux/{} (+https://github.com/{}/{})",
            local_version(),
            GITHUB_OWNER,
            GITHUB_REPO
        ))
        .build();

    let response = agent
        .get(url)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .call()
        .map_err(|e| format!("request failed: {e}"))?;

    let status = response.status();
    let body: serde_json::Value = response.into_json().map_err(|e| format!("invalid JSON: {e}"))?;

    // Treat HTTP 404 as empty release set (handled by caller for releases).
    if status == 404 {
        return Ok(serde_json::json!({ "message": "Not Found" }));
    }
    if !(200..300).contains(&status) {
        let msg = body.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error");
        return Err(format!("GitHub API HTTP {status}: {msg}"));
    }
    Ok(body)
}

fn check_for_updates_with(client: impl ReleaseClient) -> UpdateCheckOutcome {
    let local_ver = local_version();
    let local_sha = local_git_sha();

    // 1) Prefer formal releases when present.
    match client.latest_release() {
        Ok(Some(release)) => {
            if is_remote_newer(local_ver, &release.tag) {
                return UpdateCheckOutcome {
                    status: UpdateStatus::Available {
                        remote_label: release.tag,
                        url: release.html_url,
                        source: UpdateSource::Release,
                    },
                };
            }
            // Latest release is not newer than local — treat as up to date.
            // (Do not nag about main tip drift for versioned installs.)
            return UpdateCheckOutcome { status: UpdateStatus::UpToDate };
        }
        Ok(None) => {
            // No releases published yet — fall through to commit compare.
        }
        Err(err) => {
            // Release endpoint failed — try main as a fallback signal.
            tracing::warn!(%err, "latest release check failed; trying main commit");
            return match client.main_commit_sha() {
                Ok(remote_sha) => commit_outcome(local_sha, &remote_sha),
                Err(err2) => UpdateCheckOutcome {
                    status: UpdateStatus::Error(format!("{err}; also {err2}")),
                },
            };
        }
    }

    // 2) No releases yet: compare embedded build SHA to tip of main.
    match client.main_commit_sha() {
        Ok(remote_sha) => commit_outcome(local_sha, &remote_sha),
        Err(err) => {
            tracing::warn!(%err, "main commit check failed");
            UpdateCheckOutcome { status: UpdateStatus::Error(err) }
        }
    }
}

fn commit_outcome(local_sha: &str, remote_sha: &str) -> UpdateCheckOutcome {
    if local_sha.is_empty() {
        // Non-git / stripped builds can't compare SHAs — don't claim up-to-date falsely.
        return UpdateCheckOutcome {
            status: UpdateStatus::Error(
                "build has no embedded git SHA; reinstall from GitHub to enable commit checks"
                    .into(),
            ),
        };
    }
    if sha_matches(local_sha, remote_sha) {
        UpdateCheckOutcome { status: UpdateStatus::UpToDate }
    } else {
        UpdateCheckOutcome {
            status: UpdateStatus::Available {
                remote_label: remote_sha.to_string(),
                url: format!("https://github.com/{GITHUB_OWNER}/{GITHUB_REPO}/commit/{remote_sha}"),
                source: UpdateSource::MainCommit,
            },
        }
    }
}

/// Parse `1.2.3` or `v1.2.3` (extra pre-release suffix ignored for compare).
pub fn parse_semver(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches('v').trim_start_matches('V');
    // Drop pre-release / build metadata: 1.2.3-beta+meta → 1.2.3
    let core = s.split(['-', '+']).next().unwrap_or(s);
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

/// True when `remote` is a higher semver than `local`.
pub fn is_remote_newer(local: &str, remote: &str) -> bool {
    match (parse_semver(local), parse_semver(remote)) {
        (Some(l), Some(r)) => r > l,
        _ => false,
    }
}

/// Compare short or full SHAs (prefix match either way).
pub fn sha_matches(local: &str, remote: &str) -> bool {
    let local = local.trim().to_ascii_lowercase();
    let remote = remote.trim().to_ascii_lowercase();
    if local.is_empty() || remote.is_empty() {
        return false;
    }
    local == remote || remote.starts_with(&local) || local.starts_with(&remote)
}

fn short_sha(full: &str) -> String {
    full.chars().take(7).collect()
}

/// Spawn a background check; result is sent on the returned receiver.
pub fn spawn_check() -> std::sync::mpsc::Receiver<UpdateCheckOutcome> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("rmux-update-check".into())
        .spawn(move || {
            let outcome = check_for_updates();
            tracing::info!(?outcome.status, "update check finished");
            let _ = tx.send(outcome);
        })
        .ok();
    rx
}

// ─── Apply update (install into the system) ─────────────────────────────────

/// Result of running the official installer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyUpdateOutcome {
    /// Binary (and optional desktop integration) installed successfully.
    ///
    /// Constructed by the Unix installer path. Matched on all platforms (and
    /// in tests); on Windows `apply_update` currently only returns `Failed`.
    #[allow(dead_code)]
    Success {
        /// Path we expect the user to relaunch (`~/.local/bin/rmux` or current exe).
        binary_path: String,
        /// Git ref that was installed (`main` or a release tag).
        installed_ref: String,
    },
    /// Installer failed (network, build, missing tools, …).
    Failed { message: String },
}

/// Map an update signal to the git ref `install.sh` should clone (`RMUX_VERSION`).
pub fn install_ref_for(source: UpdateSource, remote_label: &str) -> String {
    match source {
        // Tags are `v0.2.0`; install.sh accepts branch or tag names.
        UpdateSource::Release => remote_label.to_string(),
        UpdateSource::MainCommit => "main".to_string(),
    }
}

/// Default install location used by `scripts/install.sh`.
#[cfg(not(windows))]
pub fn default_install_dir() -> std::path::PathBuf {
    dirs_home()
        .map(|h| h.join(".local").join("bin"))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

#[cfg(not(windows))]
fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

/// Expected path of the installed binary after a successful update.
#[cfg(not(windows))]
pub fn installed_binary_path() -> std::path::PathBuf {
    let mut p = default_install_dir();
    p.push("rmux");
    p
}

/// Run the official install script (blocking). Intended for a background thread.
///
/// Uses `curl | bash` with `RMUX_VERSION` set so the same path as a manual
/// reinstall is used. Requires network + a Rust toolchain (install.sh can
/// bootstrap rustup).
pub fn apply_update(source: UpdateSource, remote_label: &str) -> ApplyUpdateOutcome {
    let installed_ref = install_ref_for(source, remote_label);
    tracing::info!(%installed_ref, "applying update via install.sh");

    #[cfg(windows)]
    {
        // install.sh is bash/curl oriented; Windows users should use the
        // documented one-liner from a Git Bash / WSL environment for now.
        let _ = installed_ref;
        ApplyUpdateOutcome::Failed {
            message: "in-app update is not supported on Windows yet — run the install script from Git Bash or WSL".into(),
        }
    }

    #[cfg(not(windows))]
    {
        apply_update_unix(&installed_ref)
    }
}

#[cfg(not(windows))]
fn apply_update_unix(installed_ref: &str) -> ApplyUpdateOutcome {
    use std::process::Command;

    // Prefer curl, fall back to wget (same as install.sh).
    let fetch = if command_exists("curl") {
        format!("curl -fsSL '{INSTALL_SCRIPT_URL}'")
    } else if command_exists("wget") {
        format!("wget -qO- '{INSTALL_SCRIPT_URL}'")
    } else {
        return ApplyUpdateOutcome::Failed {
            message: "need curl or wget to download the installer".into(),
        };
    };

    if !command_exists("bash") {
        return ApplyUpdateOutcome::Failed {
            message: "bash is required to run the installer".into(),
        };
    }

    let pipeline = format!("{fetch} | bash");
    let output = Command::new("bash")
        .arg("-lc")
        .arg(&pipeline)
        .env("RMUX_VERSION", installed_ref)
        .env("RMUX_REPO", format!("https://github.com/{GITHUB_OWNER}/{GITHUB_REPO}.git"))
        // Keep desktop integration so Dock / .desktop stay in sync.
        .env("RMUX_SKIP_DESKTOP", "0")
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let binary = installed_binary_path();
            if !binary.is_file() {
                // Installer reported success but binary missing — still surface a soft error.
                let stderr = String::from_utf8_lossy(&out.stderr);
                return ApplyUpdateOutcome::Failed {
                    message: format!(
                        "installer finished but binary not found at {} ({})",
                        binary.display(),
                        truncate_msg(&stderr, 200)
                    ),
                };
            }
            tracing::info!(path = %binary.display(), %installed_ref, "update installed");
            ApplyUpdateOutcome::Success {
                binary_path: binary.display().to_string(),
                installed_ref: installed_ref.to_string(),
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            let detail = if !stderr.trim().is_empty() { stderr } else { stdout };
            ApplyUpdateOutcome::Failed {
                message: format!(
                    "installer exited with {}: {}",
                    out.status,
                    truncate_msg(&detail, 280)
                ),
            }
        }
        Err(err) => {
            ApplyUpdateOutcome::Failed { message: format!("failed to spawn installer: {err}") }
        }
    }
}

#[cfg(not(windows))]
fn command_exists(name: &str) -> bool {
    std::process::Command::new("sh")
        .args(["-c", &format!("command -v {name} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(any(test, not(windows)))]
fn truncate_msg(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        t.to_string()
    } else {
        let head: String = t.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

/// Spawn a background install; result is sent on the returned receiver.
pub fn spawn_apply_update(
    source: UpdateSource,
    remote_label: String,
) -> std::sync::mpsc::Receiver<ApplyUpdateOutcome> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("rmux-apply-update".into())
        .spawn(move || {
            let outcome = apply_update(source, &remote_label);
            tracing::info!(?outcome, "apply update finished");
            let _ = tx.send(outcome);
        })
        .ok();
    rx
}

/// Relaunch the installed binary (caller should close the current window after).
///
/// Prefer `binary_path` from a successful install; fall back to `current_exe`.
/// On macOS, if `~/Applications/rmux.app` exists, uses `open -n` so Launch
/// Services starts a fresh GUI instance the same way Dock / Spotlight do.
pub fn relaunch(binary_path: &str) -> Result<(), String> {
    use std::process::Command;

    #[cfg(target_os = "macos")]
    {
        if let Some(app) = macos_app_bundle() {
            Command::new("open")
                .args(["-n", "-a"])
                .arg(&app)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| format!("failed to open {}: {e}", app.display()))?;
            tracing::info!(path = %app.display(), "relaunched via macOS app bundle");
            return Ok(());
        }
    }

    let path = if std::path::Path::new(binary_path).is_file() {
        binary_path.to_string()
    } else {
        std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?.display().to_string()
    };

    // Detach so the new process survives when we exit.
    Command::new(&path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to relaunch {path}: {e}"))?;

    tracing::info!(%path, "relaunched updated binary");
    Ok(())
}

/// `~/Applications/rmux.app` when present (install.sh desktop integration).
#[cfg(target_os = "macos")]
fn macos_app_bundle() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    let app = std::path::PathBuf::from(home).join("Applications").join("rmux.app");
    app.is_dir().then_some(app)
}

// ─── tests (no network) ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockClient {
        release: Mutex<Result<Option<RemoteRelease>, String>>,
        main_sha: Mutex<Result<String, String>>,
    }

    impl ReleaseClient for MockClient {
        fn latest_release(&self) -> Result<Option<RemoteRelease>, String> {
            self.release.lock().expect("lock").clone()
        }
        fn main_commit_sha(&self) -> Result<String, String> {
            self.main_sha.lock().expect("lock").clone()
        }
    }

    #[test]
    fn parse_semver_accepts_v_prefix() {
        assert_eq!(parse_semver("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_semver("0.1.0"), Some((0, 1, 0)));
        assert_eq!(parse_semver("2.0"), Some((2, 0, 0)));
        assert_eq!(parse_semver("1.0.0-beta.1"), Some((1, 0, 0)));
        assert!(parse_semver("not-a-version").is_none());
    }

    #[test]
    fn is_remote_newer_compares_tuples() {
        assert!(is_remote_newer("0.1.0", "v0.2.0"));
        assert!(is_remote_newer("0.1.0", "0.1.1"));
        assert!(!is_remote_newer("0.2.0", "0.1.9"));
        assert!(!is_remote_newer("1.0.0", "1.0.0"));
        assert!(!is_remote_newer("bad", "also-bad"));
    }

    #[test]
    fn sha_matches_prefix() {
        assert!(sha_matches("abc1234", "abc1234def"));
        assert!(sha_matches("abc1234def", "abc1234"));
        assert!(!sha_matches("abc1234", "deadbeef"));
        assert!(!sha_matches("", "abc"));
    }

    #[test]
    fn release_newer_reports_available() {
        // Force local version path: we can't override env!("CARGO_PKG_VERSION"),
        // so craft a remote tag that's always newer than any 0.x (99.0.0).
        let client = MockClient {
            release: Mutex::new(Ok(Some(RemoteRelease {
                tag: "v99.0.0".into(),
                html_url: "https://example.com/r".into(),
            }))),
            main_sha: Mutex::new(Ok("deadbee".into())),
        };
        let out = check_for_updates_with(client);
        match out.status {
            UpdateStatus::Available { remote_label, source, .. } => {
                assert_eq!(remote_label, "v99.0.0");
                assert_eq!(source, UpdateSource::Release);
            }
            other => panic!("expected Available, got {other:?}"),
        }
    }

    #[test]
    fn matching_main_sha_is_up_to_date_when_no_newer_release() {
        let local = local_git_sha();
        if local.is_empty() {
            // CI without .git — skip.
            return;
        }
        let client = MockClient {
            release: Mutex::new(Ok(None)),
            main_sha: Mutex::new(Ok(local.to_string())),
        };
        let out = check_for_updates_with(client);
        assert_eq!(out.status, UpdateStatus::UpToDate);
    }

    #[test]
    fn different_main_sha_is_available() {
        let client = MockClient {
            release: Mutex::new(Ok(None)),
            // Use a SHA that won't match the local build (unless extremely unlucky).
            main_sha: Mutex::new(Ok("fffffff".into())),
        };
        let out = check_for_updates_with(client);
        // If local SHA is empty, we get Error instead — both are fine for coverage.
        assert!(matches!(
            out.status,
            UpdateStatus::Available { source: UpdateSource::MainCommit, .. }
                | UpdateStatus::Error(_)
        ));
    }

    #[test]
    fn install_ref_for_release_uses_tag() {
        assert_eq!(install_ref_for(UpdateSource::Release, "v0.2.0"), "v0.2.0");
    }

    #[test]
    fn install_ref_for_main_commit_is_main() {
        assert_eq!(install_ref_for(UpdateSource::MainCommit, "abc1234"), "main");
    }

    #[test]
    fn truncate_msg_short_unchanged() {
        assert_eq!(truncate_msg("hello", 10), "hello");
    }

    #[test]
    fn truncate_msg_long_ellipsis() {
        let s = truncate_msg("abcdefghijklmnopqrstuvwxyz", 10);
        assert!(s.ends_with('…'));
        assert!(s.chars().count() <= 10);
    }
}
