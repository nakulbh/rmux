//! PTY backend for terminal process management.
//!
//! Manages the pseudo-terminal (PTY) lifecycle: spawning a shell,
//! reading output, writing input, and handling resize events.
//! Built on `portable-pty` for cross-platform PTY support.

use portable_pty::{Child, ChildKiller, CommandBuilder, ExitStatus, MasterPty, PtySize};
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur during PTY operations.
#[derive(Error, Debug)]
pub enum PtyError {
    /// Failed to open a new PTY device.
    #[error("Failed to open PTY: {0}")]
    OpenPty(#[from] anyhow::Error),

    /// Failed to spawn the child process.
    #[error("Failed to spawn child process: {0}")]
    SpawnProcess(#[source] anyhow::Error),

    /// Failed to write input to the PTY.
    #[error("Failed to write to PTY: {0}")]
    WriteError(#[source] std::io::Error),

    /// Failed to resize the PTY.
    #[error("Failed to resize PTY: {0}")]
    ResizeError(#[source] anyhow::Error),

    /// Failed to take a reader/writer from the PTY.
    #[error("Failed to acquire PTY I/O: {0}")]
    IoSetup(#[source] anyhow::Error),
}

/// The result type for PTY operations.
pub type PtyResult<T> = Result<T, PtyError>;

/// Manages a PTY child process and its I/O streams.
///
/// Wraps `portable-pty::PtyPair` and provides a high-level API
/// for spawning a shell, reading terminal output, writing keyboard
/// input, and resizing the terminal.
///
/// # Examples
///
/// ```no_run
/// use rmux_terminal::PtyBackend;
///
/// let mut backend = PtyBackend::spawn(80, 24).unwrap();
/// assert!(backend.is_alive());
/// backend.write(b"echo hello\n").unwrap();
/// ```
pub struct PtyBackend {
    /// The spawned child process.
    child: Box<dyn Child + Send + 'static>,
    /// The master PTY (for resize and I/O).
    master: Box<dyn MasterPty + Send>,
    /// Cloned reader for PTY output.
    reader: Option<Box<dyn Read + Send>>,
    /// Writer for PTY input.
    writer: Option<Box<dyn Write + Send>>,
    /// Cloned child killer for signaling.
    child_killer: Box<dyn ChildKiller + Send>,
    /// Whether the child process has exited.
    exited: bool,
}

impl PtyBackend {
    /// Spawn a shell in a new PTY (cwd defaults to `$HOME` on Unix).
    ///
    /// See [`Self::spawn_with_cwd`] to inherit a sibling pane's directory.
    pub fn spawn(cols: u16, rows: u16) -> PtyResult<Self> {
        Self::spawn_with_cwd(cols, rows, None)
    }

    /// Spawn a shell in a new PTY, starting in `cwd` when provided.
    ///
    /// When `cwd` is `None` (or not a directory), falls back to `$HOME`
    /// on Unix, otherwise the process inherits the parent cwd.
    ///
    /// The shell is started as a **login** shell when the binary supports
    /// it (`-l` / `--login`). That loads profile scripts (`.zprofile`,
    /// `.bash_profile`, etc.) so tools installed via Homebrew, cargo, nvm,
    /// and similar are on `PATH` — including when rmux itself was launched
    /// from the Dock / Finder with macOS's sparse GUI environment.
    ///
    /// # Errors
    ///
    /// Returns [`PtyError::OpenPty`] if the PTY could not be created.
    /// Returns [`PtyError::SpawnProcess`] if the shell process could not be spawned.
    pub fn spawn_with_cwd(cols: u16, rows: u16, cwd: Option<&Path>) -> PtyResult<Self> {
        // Determine which shell to use
        let shell = std::env::var("SHELL").unwrap_or_else(|_| {
            #[cfg(unix)]
            {
                "/bin/sh".to_string()
            }
            #[cfg(not(unix))]
            {
                "cmd.exe".to_string()
            }
        });

        let pty_system = portable_pty::native_pty_system();

        let pty_size = PtySize { rows, cols, pixel_width: 0, pixel_height: 0 };

        let pair = pty_system.openpty(pty_size).map_err(PtyError::OpenPty)?;

        let mut cmd = CommandBuilder::new(&shell);
        configure_shell_env(&mut cmd, &shell);

        // Prefer the caller's cwd (sibling terminal path); else $HOME on Unix.
        if let Some(dir) = cwd.filter(|p| p.is_dir()) {
            cmd.cwd(dir);
        } else {
            #[cfg(unix)]
            if let Ok(home) = std::env::var("HOME") {
                cmd.cwd(home);
            }
        }

        let child = pair.slave.spawn_command(cmd).map_err(PtyError::SpawnProcess)?;

        let reader = pair.master.try_clone_reader().map_err(PtyError::IoSetup)?;

        let writer = pair.master.take_writer().map_err(PtyError::IoSetup)?;

        let child_killer = child.clone_killer();

        Ok(Self {
            child,
            master: pair.master,
            reader: Some(reader),
            writer: Some(writer),
            child_killer,
            exited: false,
        })
    }

    /// OS process id of the shell, if available.
    pub fn process_id(&self) -> Option<u32> {
        self.child.process_id()
    }

    /// Best-effort current working directory of the shell process.
    ///
    /// Used so new splits / tabs can open in the same directory the user
    /// already `cd`'d into, instead of always `$HOME`.
    pub fn working_directory(&self) -> Option<PathBuf> {
        let pid = self.process_id()?;
        cwd_of_process(pid)
    }

    /// Best-effort title of the command currently running under this shell.
    ///
    /// Walks the shell's child processes (via `ps`) and returns a cleaned
    /// command line for the first non-shell descendant. Used for cmux-style
    /// dynamic workspace names (`cargo run …`, `nvim`, etc.). Returns `None`
    /// when the shell is idle (only shell descendants) or probing fails.
    pub fn foreground_process_title(&self) -> Option<String> {
        let pid = self.process_id()?;
        foreground_process_title(pid)
    }

    /// Write input bytes to the PTY (keyboard input, paste, etc.).
    ///
    /// # Errors
    ///
    /// Returns [`PtyError::WriteError`] if the write failed.
    pub fn write(&mut self, data: &[u8]) -> PtyResult<()> {
        if let Some(ref mut writer) = self.writer {
            writer.write_all(data).map_err(PtyError::WriteError)?;
            writer.flush().map_err(PtyError::WriteError)?;
        }
        Ok(())
    }

    /// Try to read from the PTY without blocking.
    ///
    /// Returns `Some(n)` if data was read into `buf` (up to `buf.len()` bytes).
    /// Returns `None` if no data is available.
    ///
    /// On Unix, the reader returned by `portable-pty`'s `try_clone_reader()`
    /// uses non-blocking I/O internally, so this method will not block.
    pub fn try_read(&mut self, buf: &mut [u8]) -> Option<usize> {
        let reader = self.reader.as_mut()?;

        match reader.read(buf) {
            Ok(0) => None,
            Ok(n) => Some(n),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => None,
            Err(_) => None,
        }
    }

    /// Take the PTY reader for use in a background read thread.
    ///
    /// After calling this, `try_read` will always return `None`.
    /// The caller should spawn a thread that reads from the returned reader
    /// and sends data to the main thread via a channel.
    pub fn take_reader(&mut self) -> Option<Box<dyn Read + Send>> {
        self.reader.take()
    }

    /// Resize the PTY to new dimensions.
    ///
    /// # Errors
    ///
    /// Returns [`PtyError::ResizeError`] if the resize ioctl failed.
    pub fn resize(&mut self, cols: u16, rows: u16) -> PtyResult<()> {
        let size = PtySize { rows, cols, pixel_width: 0, pixel_height: 0 };
        self.master.resize(size).map_err(PtyError::ResizeError)?;
        Ok(())
    }

    /// Check if the child process is still running.
    pub fn is_alive(&self) -> bool {
        !self.exited
    }

    /// Get the exit status if the process has exited.
    ///
    /// Returns `None` if the process is still running.
    pub fn try_wait(&mut self) -> Option<ExitStatus> {
        let status = self.child.try_wait().ok().flatten();
        if status.is_some() {
            self.exited = true;
        }
        status
    }

    /// Kill the child process.
    pub fn kill(&mut self) -> std::io::Result<()> {
        self.child_killer.kill()
    }
}

impl std::fmt::Debug for PtyBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtyBackend").field("alive", &self.is_alive()).finish()
    }
}

/// Configure environment and argv for a user shell inside a PTY.
///
/// * Sets `TERM` / `COLORTERM` so apps get modern capabilities.
/// * Starts a login shell when supported so profile scripts populate `PATH`
///   (critical when the parent GUI process has a sparse Dock/Finder env).
/// * When the parent process itself has a sparse `PATH` (GUI launch), injects
///   the user's login-shell `PATH` so even non-login shells and child tools
///   see Homebrew / cargo / nvm.
fn configure_shell_env(cmd: &mut CommandBuilder, shell: &str) {
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");

    if let Some(path) = login_path_if_needed(shell) {
        cmd.env("PATH", path);
    }

    for arg in login_args_for_shell(shell) {
        cmd.arg(arg);
    }
}

/// Login-shell flags for common shells. Empty for shells that don't use them.
///
/// Login shells source profile files (`.zprofile`, `.bash_profile`, …) where
/// package managers install their `PATH` setup. Matches Terminal.app / iTerm2 /
/// WezTerm defaults on macOS.
fn login_args_for_shell(shell: &str) -> Vec<&'static str> {
    let name = Path::new(shell).file_name().and_then(|s| s.to_str()).unwrap_or(shell);

    match name {
        // POSIX / common Unix shells
        "bash" | "zsh" | "fish" | "ksh" | "dash" | "csh" | "tcsh" | "sh" => {
            vec!["-l"]
        }
        // Nushell
        "nu" | "nushell" => vec!["--login"],
        // Windows shells / unknown: leave argv alone
        _ => Vec::new(),
    }
}

/// When the current process `PATH` looks like a macOS/Linux GUI default,
/// query the user's login shell once and return its `PATH`.
///
/// Returns `None` when the inherited `PATH` already looks complete (started
/// from a terminal) so we don't override the user's environment.
fn login_path_if_needed(shell: &str) -> Option<String> {
    use std::sync::OnceLock;

    static CACHED: OnceLock<Option<String>> = OnceLock::new();

    CACHED
        .get_or_init(|| {
            let current = std::env::var("PATH").unwrap_or_default();
            if !path_looks_sparse(&current) {
                return None;
            }

            capture_login_path(shell).filter(|p| !p.is_empty() && p != &current)
        })
        .clone()
}

/// Heuristic: GUI defaults are short; developer shells usually include
/// Homebrew, cargo, or a longer list of entries.
fn path_looks_sparse(path: &str) -> bool {
    let entries: Vec<&str> = path.split(':').filter(|s| !s.is_empty()).collect();
    if entries.len() <= 6 {
        return true;
    }
    let has_user_tools = entries.iter().any(|p| {
        p.contains("homebrew")
            || p.contains("/.cargo/")
            || p.contains("/.local/bin")
            || p.contains("/.nvm/")
            || *p == "/usr/local/bin"
            || *p == "/opt/homebrew/bin"
    });
    !has_user_tools && entries.len() < 12
}

/// Run `shell -l -c 'printf %s "$PATH"'` (or nushell equivalent) and capture
/// the resulting PATH. Used only when the parent env looks sparse.
fn capture_login_path(shell: &str) -> Option<String> {
    let name = Path::new(shell).file_name().and_then(|s| s.to_str()).unwrap_or(shell);

    let mut cmd = std::process::Command::new(shell);
    // Clear PATH so profile scripts rebuild it from scratch (path_helper,
    // brew shellenv, etc.) instead of appending to the sparse GUI path.
    cmd.env_remove("PATH");

    match name {
        "nu" | "nushell" => {
            cmd.args(["--login", "-c", "print $env.PATH | str join ':'"]);
        }
        _ => {
            cmd.args(["-l", "-c", "printf %s \"$PATH\""]);
        }
    }

    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() { None } else { Some(path) }
}

/// Maximum characters kept for a process-title string (sidebar-friendly).
const MAX_PROCESS_TITLE_CHARS: usize = 48;

/// Best-effort title of a non-shell process running under `shell_pid`.
///
/// Uses a single `ps` snapshot so we avoid N subprocesses when scanning.
/// Returns `None` on non-Unix platforms or when no interesting child exists.
pub fn foreground_process_title(shell_pid: u32) -> Option<String> {
    #[cfg(unix)]
    {
        foreground_process_title_unix(shell_pid)
    }
    #[cfg(not(unix))]
    {
        let _ = shell_pid;
        None
    }
}

#[cfg(unix)]
fn foreground_process_title_unix(shell_pid: u32) -> Option<String> {
    let output =
        std::process::Command::new("ps").args(["-ax", "-o", "pid=,ppid=,args="]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let rows = parse_ps_pid_ppid_args(&text);
    pick_foreground_title(shell_pid, &rows)
}

/// Parse `ps -o pid=,ppid=,args=` lines into `(pid, ppid, args)`.
pub fn parse_ps_pid_ppid_args(stdout: &str) -> Vec<(u32, u32, String)> {
    let mut rows = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(pid_s) = parts.next() else { continue };
        let Some(ppid_s) = parts.next() else { continue };
        let Ok(pid) = pid_s.parse::<u32>() else { continue };
        let Ok(ppid) = ppid_s.parse::<u32>() else { continue };
        let args = parts.collect::<Vec<_>>().join(" ");
        if args.is_empty() {
            continue;
        }
        rows.push((pid, ppid, args));
    }
    rows
}

/// Choose a display title for the session rooted at `shell_pid`.
///
/// Prefers a direct non-shell child of the shell (the command the user ran).
/// Falls back to a deeper non-shell descendant so wrappers like `env` still
/// surface something useful.
pub fn pick_foreground_title(shell_pid: u32, rows: &[(u32, u32, String)]) -> Option<String> {
    let mut by_parent: std::collections::HashMap<u32, Vec<&(u32, u32, String)>> =
        std::collections::HashMap::new();
    for row in rows {
        by_parent.entry(row.1).or_default().push(row);
    }

    // Breadth-first from the shell: first non-shell command line wins,
    // preferring shallower processes (cargo/nvim over rustc grandchildren).
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(shell_pid);
    let mut seen = std::collections::HashSet::new();
    seen.insert(shell_pid);

    while let Some(pid) = queue.pop_front() {
        let Some(children) = by_parent.get(&pid) else {
            continue;
        };
        for (child_pid, _, args) in children {
            if !seen.insert(*child_pid) {
                continue;
            }
            queue.push_back(*child_pid);
            let token = first_command_token(args);
            if is_shell_or_helper(token) {
                continue;
            }
            let cleaned = clean_process_title(args);
            if !cleaned.is_empty() {
                return Some(cleaned);
            }
        }
    }
    None
}

fn first_command_token(args: &str) -> &str {
    let token = args.split_whitespace().next().unwrap_or(args);
    // Strip leading path: /bin/zsh → zsh, ./target/debug/foo → foo
    token.rsplit('/').next().unwrap_or(token)
}

/// Shells and wrappers that should not become the workspace title.
fn is_shell_or_helper(comm: &str) -> bool {
    let base = comm.trim_start_matches('-');
    matches!(
        base,
        "sh"
            | "bash"
            | "zsh"
            | "fish"
            | "dash"
            | "csh"
            | "tcsh"
            | "ksh"
            | "login"
            | "nu"
            | "pwsh"
            | "powershell"
            | "cmd"
            | "cmd.exe"
            | "env"
            | "nice"
            | "nohup"
            | "script"
            | "time"
            | "timeout"
            | "stdbuf"
            | "script_session"
            // macOS login helpers
            | "login.exe"
    )
}

/// Collapse whitespace and truncate for sidebar display.
pub fn clean_process_title(args: &str) -> String {
    let collapsed: String = args.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_chars(&collapsed, MAX_PROCESS_TITLE_CHARS)
}

fn truncate_chars(s: &str, max: usize) -> String {
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

/// Resolve the current working directory of process `pid`.
///
/// * Linux: read `/proc/<pid>/cwd` symlink.
/// * macOS: parse `lsof -a -d cwd -p <pid> -Fn` (no extra crates).
/// * Other: `None`.
fn cwd_of_process(pid: u32) -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
    }
    #[cfg(target_os = "macos")]
    {
        cwd_of_process_macos(pid)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        None
    }
}

/// macOS: ask `lsof` for the cwd file descriptor of `pid`.
#[cfg(target_os = "macos")]
fn cwd_of_process_macos(pid: u32) -> Option<PathBuf> {
    let output = std::process::Command::new("lsof")
        .args(["-a", "-d", "cwd", "-p", &pid.to_string(), "-Fn"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_lsof_cwd_output(&output.stdout)
}

/// Parse `lsof -Fn` stdout and return the first `n<path>` entry that is a dir.
///
/// Compiled on macOS (production caller) and under `cfg(test)` on all platforms
/// so unit tests stay green on Linux/Windows without dead-code warnings.
#[cfg(any(test, target_os = "macos"))]
fn parse_lsof_cwd_output(stdout: &[u8]) -> Option<PathBuf> {
    let text = String::from_utf8_lossy(stdout);
    for line in text.lines() {
        if let Some(path) = line.strip_prefix('n') {
            // lsof can emit paths like `/path (deleted)` — take the path part.
            let path = path.split(" (").next().unwrap_or(path);
            let p = PathBuf::from(path);
            if p.is_dir() {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_shell() {
        let mut backend = PtyBackend::spawn(80, 24).expect("Failed to spawn shell");
        assert!(backend.is_alive(), "Shell should be alive immediately after spawning");
        // Clean up
        backend.kill().ok();
    }

    #[test]
    fn test_parse_ps_pid_ppid_args() {
        let sample = "\
  100   1 /bin/zsh\n\
  200 100 cargo run -p rmux-app\n\
  201 200 rustc --crate-name foo\n\
";
        let rows = parse_ps_pid_ppid_args(sample);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].2, "cargo run -p rmux-app");
    }

    #[test]
    fn test_pick_foreground_title_prefers_shell_child() {
        let rows = vec![
            (100, 1, "/bin/zsh".into()),
            (200, 100, "cargo run -p rmux-app --release".into()),
            (201, 200, "rustc --crate-name foo".into()),
        ];
        let title = pick_foreground_title(100, &rows).expect("title");
        assert!(title.starts_with("cargo run"), "got {title}");
        // Idle shell → None
        assert!(pick_foreground_title(100, &[(100, 1, "zsh".into())]).is_none());
    }

    #[test]
    fn test_clean_process_title_truncates() {
        let long = "a".repeat(80);
        let cleaned = clean_process_title(&long);
        assert_eq!(cleaned.chars().count(), MAX_PROCESS_TITLE_CHARS);
        assert!(cleaned.ends_with('…'));
    }

    #[test]
    fn test_is_shell_or_helper() {
        assert!(is_shell_or_helper("zsh"));
        assert!(is_shell_or_helper("-zsh"));
        assert!(is_shell_or_helper("bash"));
        assert!(!is_shell_or_helper("cargo"));
        assert!(!is_shell_or_helper("nvim"));
    }

    #[test]
    fn test_spawn_with_cwd_uses_directory() {
        let tmp = std::env::temp_dir();
        let mut backend =
            PtyBackend::spawn_with_cwd(80, 24, Some(tmp.as_path())).expect("spawn with cwd");
        assert!(backend.is_alive());
        // Working directory query is best-effort; just ensure spawn succeeded.
        backend.kill().ok();
    }

    #[test]
    fn test_parse_lsof_cwd_output() {
        let home = dirs_or_tmp();
        let fake = format!("p12345\nfcwd\nn{}\n", home.display());
        let parsed = parse_lsof_cwd_output(fake.as_bytes());
        assert_eq!(parsed.as_ref(), Some(&home));
    }

    fn dirs_or_tmp() -> PathBuf {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .unwrap_or_else(std::env::temp_dir)
    }

    #[test]
    fn test_write_and_wait() {
        let mut backend = PtyBackend::spawn(80, 24).expect("Failed to spawn shell");
        // Write exit command
        backend.write(b"exit\n").ok();

        // Wait a bit for the process to exit
        std::thread::sleep(std::time::Duration::from_millis(500));

        let status = backend.try_wait();
        // The process may or may not have exited yet; either way is fine
        let _ = status;
    }

    #[test]
    fn test_resize() {
        let mut backend = PtyBackend::spawn(80, 24).expect("Failed to spawn shell");
        let result = backend.resize(120, 40);
        assert!(result.is_ok(), "Resize should succeed");
        backend.kill().ok();
    }

    #[test]
    fn test_login_args_for_common_shells() {
        assert_eq!(login_args_for_shell("/bin/zsh"), vec!["-l"]);
        assert_eq!(login_args_for_shell("/bin/bash"), vec!["-l"]);
        assert_eq!(login_args_for_shell("/usr/local/bin/fish"), vec!["-l"]);
        assert_eq!(login_args_for_shell("/opt/homebrew/bin/nu"), vec!["--login"]);
        assert_eq!(login_args_for_shell("cmd.exe"), Vec::<&str>::new());
    }

    #[test]
    fn test_path_looks_sparse_detects_gui_default() {
        assert!(path_looks_sparse("/usr/bin:/bin:/usr/sbin:/sbin"));
        assert!(path_looks_sparse(""));
        // Long path with homebrew is not sparse
        assert!(!path_looks_sparse(
            "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/Users/x/.cargo/bin"
        ));
    }

    #[test]
    fn test_capture_login_path_returns_nonempty() {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        // Should succeed on Unix CI / developer machines.
        if cfg!(unix) {
            let path = capture_login_path(&shell);
            assert!(
                path.as_ref().is_some_and(|p| !p.is_empty()),
                "login shell should print a non-empty PATH, got {path:?}"
            );
        }
    }
}
