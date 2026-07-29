//! Off-thread title/metadata probes (`ps`, `/proc`, `lsof`, `git`, `gh`).
//!
//! Everything in here used to run **inline on the egui update thread**, once
//! per pane, every ~45 frames — plus once per frame per pane whenever the
//! session autosave fingerprint was unchanged. Each probe forks a process:
//!
//! * `ps -ax -o pid=,ppid=,args=` — tens of ms on a busy Linux box
//! * `/proc/<pid>/cwd` (Linux) or `lsof` (macOS) — `lsof` is ~50–150 ms
//! * `git rev-parse` — cold-cache repos are slow
//! * `gh pr view` — a **network** call; seconds when GitHub or DNS is slow
//!
//! With a handful of panes that is enough to blow the frame budget every
//! second (nvim `j/k` and agent prompt typing visibly hitch) and enough to
//! trip the OS "application is not responding" watchdog.
//!
//! This module moves all of it onto a single worker thread:
//!
//! * The UI enqueues the list of live shell pids about once a second.
//! * The worker takes **one** `ps` snapshot for the whole batch instead of
//!   one fork per pane.
//! * `git` / `gh` results are memoised per directory, with `gh` on a long
//!   interval and an automatic back-off when it turns out to be slow.
//! * Results land in a shared map; the UI thread only ever takes a lock and
//!   clones cached strings.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use super::sidebar_snapshot::{PullRequestDisplay, pull_request_for_cwd};
use super::title::git_branch_for_cwd;

/// How often the UI asks for a fresh batch of probes.
pub const PROBE_INTERVAL: Duration = Duration::from_millis(900);

/// Re-run `git rev-parse` for a directory at most this often.
const GIT_TTL: Duration = Duration::from_secs(3);

/// Re-resolve a process cwd at most this often.
///
/// Linux reads a `/proc/<pid>/cwd` symlink, which is essentially free, so it
/// refreshes every batch. macOS has to fork `lsof` (~100 ms), so it is rate
/// limited — tab labels lag a `cd` by a couple of seconds instead of burning a
/// process per pane per second.
const CWD_TTL: Duration =
    if cfg!(target_os = "macos") { Duration::from_millis(2500) } else { Duration::ZERO };

/// Re-run `gh pr view` for a directory at most this often.
const PR_TTL: Duration = Duration::from_secs(120);

/// A single `gh` invocation slower than this marks the directory as slow.
const PR_SLOW_THRESHOLD: Duration = Duration::from_secs(3);

/// How long a slow directory is skipped for `gh` probes.
const PR_BACKOFF: Duration = Duration::from_secs(600);

/// Everything the UI wants to know about one shell process.
#[derive(Debug, Clone, Default)]
pub struct PaneProbe {
    /// Shell cwd (tab labels, new-split inheritance).
    pub cwd: Option<PathBuf>,
    /// Full untruncated foreground command line (agent resume capture).
    pub fg_args: Option<String>,
    /// Sidebar-friendly truncated foreground command.
    pub fg_title: Option<String>,
    /// Git branch for `cwd`.
    pub git_branch: Option<String>,
    /// PR chip for `cwd`.
    pub pull_request: Option<PullRequestDisplay>,
}

/// Handle to the background probe worker.
///
/// Cloning is cheap; the UI keeps one instance in [`crate::app::RmuxApp`].
pub struct TitleProbe {
    tx: mpsc::Sender<Vec<u32>>,
    results: Arc<Mutex<HashMap<u32, PaneProbe>>>,
    busy: Arc<AtomicBool>,
    /// Bumped once per completed batch so the UI can skip unchanged frames.
    generation: Arc<AtomicU64>,
    /// Generation the UI has already applied.
    applied_generation: u64,
    last_request: Instant,
}

impl TitleProbe {
    /// Spawn the worker thread.
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<Vec<u32>>();
        let results: Arc<Mutex<HashMap<u32, PaneProbe>>> = Arc::new(Mutex::new(HashMap::new()));
        let busy = Arc::new(AtomicBool::new(false));

        let generation = Arc::new(AtomicU64::new(0));

        let worker_results = Arc::clone(&results);
        let worker_busy = Arc::clone(&busy);
        let worker_generation = Arc::clone(&generation);
        std::thread::Builder::new()
            .name("rmux-title-probe".into())
            .spawn(move || {
                let mut cache = DirCache::default();
                while let Ok(pids) = rx.recv() {
                    let batch = probe_batch(&pids, &mut cache);
                    if let Ok(mut slot) = worker_results.lock() {
                        // Replace wholesale so pids of closed panes drop out.
                        *slot = batch;
                    }
                    worker_generation.fetch_add(1, Ordering::Release);
                    worker_busy.store(false, Ordering::Release);
                }
            })
            .ok();

        // `last_request` starts in the past so the first frame kicks a probe.
        Self {
            tx,
            results,
            busy,
            generation,
            applied_generation: 0,
            last_request: Instant::now().checked_sub(PROBE_INTERVAL).unwrap_or_else(Instant::now),
        }
    }

    /// Enqueue `pids` if the interval elapsed and no batch is in flight.
    ///
    /// Returns `true` when a batch was submitted.
    pub fn maybe_request(&mut self, pids: Vec<u32>) -> bool {
        if pids.is_empty() {
            return false;
        }
        if self.last_request.elapsed() < PROBE_INTERVAL {
            return false;
        }
        // Never queue a second batch behind a slow `gh` / `ps`.
        if self.busy.swap(true, Ordering::AcqRel) {
            return false;
        }
        self.last_request = Instant::now();
        if self.tx.send(pids).is_err() {
            self.busy.store(false, Ordering::Release);
            return false;
        }
        true
    }

    /// Take the latest results, but only once per completed batch.
    ///
    /// Returns `None` when the UI has already seen the newest batch, so the
    /// caller can skip rebuilding sidebar aggregates on every frame.
    pub fn take_fresh(&mut self) -> Option<HashMap<u32, PaneProbe>> {
        let current = self.generation.load(Ordering::Acquire);
        if current == self.applied_generation {
            return None;
        }
        self.applied_generation = current;
        Some(self.results.lock().map(|m| m.clone()).unwrap_or_default())
    }
}

/// Per-directory memo for the slow `git` / `gh` probes.
#[derive(Default)]
struct DirCache {
    /// Per-pid cwd, rate limited by [`CWD_TTL`].
    cwd: HashMap<u32, (Instant, Option<PathBuf>)>,
    git: HashMap<PathBuf, (Instant, Option<String>)>,
    pr: HashMap<PathBuf, (Instant, Option<PullRequestDisplay>)>,
    /// Directories where `gh` was slow; skipped until the instant passes.
    pr_backoff: HashMap<PathBuf, Instant>,
}

impl DirCache {
    fn process_cwd(&mut self, pid: u32) -> Option<PathBuf> {
        if !CWD_TTL.is_zero()
            && let Some((at, value)) = self.cwd.get(&pid)
            && at.elapsed() < CWD_TTL
        {
            return value.clone();
        }
        let value = rmux_terminal::process_cwd(pid);
        self.cwd.insert(pid, (Instant::now(), value.clone()));
        value
    }

    fn git_branch(&mut self, cwd: &Path) -> Option<String> {
        if let Some((at, value)) = self.git.get(cwd)
            && at.elapsed() < GIT_TTL
        {
            return value.clone();
        }
        let value = git_branch_for_cwd(cwd);
        self.git.insert(cwd.to_path_buf(), (Instant::now(), value.clone()));
        value
    }

    fn pull_request(&mut self, cwd: &Path) -> Option<PullRequestDisplay> {
        if let Some((at, value)) = self.pr.get(cwd)
            && at.elapsed() < PR_TTL
        {
            return value.clone();
        }
        if let Some(until) = self.pr_backoff.get(cwd) {
            if Instant::now() < *until {
                // Keep serving the last known value while backing off.
                return self.pr.get(cwd).and_then(|(_, v)| v.clone());
            }
            self.pr_backoff.remove(cwd);
        }

        let started = Instant::now();
        let value = pull_request_for_cwd(cwd);
        let elapsed = started.elapsed();
        if elapsed >= PR_SLOW_THRESHOLD {
            tracing::debug!(
                dir = %cwd.display(),
                ms = elapsed.as_millis(),
                "`gh pr view` was slow; backing off"
            );
            self.pr_backoff.insert(cwd.to_path_buf(), Instant::now() + PR_BACKOFF);
        }
        self.pr.insert(cwd.to_path_buf(), (Instant::now(), value.clone()));
        value
    }

    /// Forget directories / pids no longer in use so the maps stay small.
    fn retain(&mut self, live: &[PathBuf], live_pids: &[u32]) {
        self.cwd.retain(|k, _| live_pids.contains(k));
        self.git.retain(|k, _| live.contains(k));
        self.pr.retain(|k, _| live.contains(k));
        self.pr_backoff.retain(|k, _| live.contains(k));
    }
}

/// Probe every pid using a single `ps` snapshot, then per-directory metadata.
fn probe_batch(pids: &[u32], cache: &mut DirCache) -> HashMap<u32, PaneProbe> {
    let rows = rmux_terminal::process_table();
    let mut out = HashMap::with_capacity(pids.len());
    let mut live_dirs = Vec::new();

    for &pid in pids {
        let fg_args = rmux_terminal::pick_foreground_args(pid, &rows);
        let fg_title = fg_args.as_deref().map(rmux_terminal::clean_process_title);
        let cwd = cache.process_cwd(pid);

        let (git_branch, pull_request) = match cwd.as_deref() {
            Some(dir) => {
                live_dirs.push(dir.to_path_buf());
                let branch = cache.git_branch(dir);
                // Only worth asking GitHub about actual repositories.
                let pr = if branch.is_some() { cache.pull_request(dir) } else { None };
                (branch, pr)
            }
            None => (None, None),
        };

        out.insert(pid, PaneProbe { cwd, fg_args, fg_title, git_branch, pull_request });
    }

    cache.retain(&live_dirs, pids);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_of_own_pid_reports_a_cwd() {
        // The probe worker path is exercised directly (no thread) so the test
        // stays deterministic: our own process must have a resolvable cwd on
        // Linux and macOS.
        let mut cache = DirCache::default();
        let pid = std::process::id();
        let out = probe_batch(&[pid], &mut cache);
        let entry = out.get(&pid).expect("entry for own pid");
        if cfg!(any(target_os = "linux", target_os = "macos")) {
            assert!(entry.cwd.is_some(), "own cwd should resolve");
        }
    }

    #[test]
    fn empty_request_is_not_submitted() {
        let mut probe = TitleProbe::spawn();
        assert!(!probe.maybe_request(Vec::new()));
    }

    #[test]
    fn request_is_rate_limited() {
        let mut probe = TitleProbe::spawn();
        assert!(probe.maybe_request(vec![std::process::id()]));
        // Immediate second call is inside the interval → refused.
        assert!(!probe.maybe_request(vec![std::process::id()]));
    }

    #[test]
    fn dir_cache_retain_drops_unused_entries() {
        let mut cache = DirCache::default();
        cache.git.insert(PathBuf::from("/a"), (Instant::now(), Some("main".into())));
        cache.git.insert(PathBuf::from("/b"), (Instant::now(), None));
        cache.retain(&[PathBuf::from("/a")], &[]);
        assert!(cache.git.contains_key(Path::new("/a")));
        assert!(!cache.git.contains_key(Path::new("/b")));
    }
}
