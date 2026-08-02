//! cmux CLI compatibility for agent notify plugins (OpenCode kdco-notify, etc.).
//!
//! Those plugins prefer:
//! ```text
//! cmux notify --title … [--subtitle …] --body …
//! cmux set-status <key> <text>
//! cmux clear-status <key>
//! ```
//! when `CMUX_WORKSPACE_ID` (or socket mode) is set and `cmux` is on `PATH`.
//! Without this, macOS falls back to the external `alerter` binary.
//!
//! Invoked as `rmux-cli __cmux-compat …` or when the binary argv0 is `cmux`
//! (install symlink / PATH shim).

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::json;

use crate::socket;

/// Run a cmux-compatible command (`args` without the program name).
///
/// # Errors
///
/// Returns an error when arguments are invalid or the socket call fails.
pub fn run(socket_path: &Path, args: &[String]) -> Result<()> {
    let mut it = args.iter().map(String::as_str);
    let Some(cmd) = it.next() else {
        bail!("cmux: missing command (expected notify, set-status, or clear-status)");
    };

    match cmd {
        "notify" => run_notify(socket_path, &args[1..])?,
        "set-status" => {
            // cmux: set-status <key> <text…>
            let key = it.next().unwrap_or("");
            let text_parts: Vec<&str> = it.collect();
            if text_parts.is_empty() {
                bail!("cmux set-status: expected <key> <text>");
            }
            let text = if key.is_empty() {
                text_parts.join(" ")
            } else {
                // Prefer the human-visible text; key is a session id for cmux.
                text_parts.join(" ")
            };
            let workspace_id = workspace_id_from_env();
            let params = json!({ "workspace_id": workspace_id, "status": text });
            let _ = socket::call(socket_path, "sidebar.set_status", params)?;
        }
        "clear-status" => {
            // cmux: clear-status <key>
            let _key = it.next();
            let workspace_id = workspace_id_from_env();
            let params = json!({ "workspace_id": workspace_id });
            let _ = socket::call(socket_path, "sidebar.clear_status", params)?;
        }
        // Soft-success for other cmux probes so plugins don't hard-fail.
        other => {
            eprintln!("rmux cmux-compat: ignoring unsupported command: {other}");
        }
    }
    Ok(())
}

fn run_notify(socket_path: &Path, args: &[String]) -> Result<()> {
    let mut title: Option<String> = None;
    let mut subtitle: Option<String> = None;
    let mut body: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--title" => {
                i += 1;
                title = args.get(i).cloned();
            }
            "--subtitle" => {
                i += 1;
                subtitle = args.get(i).cloned();
            }
            "--body" => {
                i += 1;
                body = args.get(i).cloned();
            }
            flag if flag.starts_with("--") => {
                // Skip unknown flags and their optional values.
                if args.get(i + 1).is_some_and(|v| !v.starts_with("--")) {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let title = title.context("cmux notify: --title is required")?;
    let workspace_id = workspace_id_from_env();
    let pane_id = env_u64("CMUX_SURFACE_ID").or_else(|| env_u64("RMUX_PANE_ID"));

    let params = json!({
        "title": title,
        "subtitle": subtitle,
        "body": body,
        "workspace_id": workspace_id,
        "pane_id": pane_id,
    });
    let _ = socket::call(socket_path, "notification.create", params)?;
    Ok(())
}

fn workspace_id_from_env() -> Option<u64> {
    env_u64("CMUX_WORKSPACE_ID").or_else(|| env_u64("RMUX_WORKSPACE_ID"))
}

fn env_u64(key: &str) -> Option<u64> {
    env::var(key).ok().and_then(|s| s.parse().ok())
}

/// Resolve the socket for cmux-compat: CLI flag path is passed in; env
/// fallbacks honor both `CMUX_SOCKET_PATH` and `RMUX_SOCKET_PATH`.
pub fn effective_socket_path(flag: Option<PathBuf>) -> PathBuf {
    if let Some(p) = flag {
        return p;
    }
    if let Ok(p) = env::var("CMUX_SOCKET_PATH") {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    socket::effective_socket_path(None)
}

/// Ensure `~/.local/bin/cmux` (or platform equivalent) is a shim to `rmux-cli
/// __cmux-compat`. Returns the directory containing the shim when successful.
///
/// Idempotent: rewrites the shim if the target path changed.
pub fn ensure_local_shim() -> Option<PathBuf> {
    let bin_dir = local_bin_dir()?;
    let rmux_cli = discover_rmux_cli()?;
    let shim = bin_dir.join("cmux");

    let body = format!(
        "#!/bin/sh\n# rmux cmux shim — auto-managed; do not edit\nexec {} __cmux-compat \"$@\"\n",
        shell_quote_path(&rmux_cli)
    );

    let needs_write = match std::fs::read_to_string(&shim) {
        Ok(existing) => existing != body,
        Err(_) => true,
    };

    if needs_write {
        std::fs::create_dir_all(&bin_dir).ok()?;
        std::fs::write(&shim, body).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&shim).ok()?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&shim, perms).ok()?;
        }
    }

    Some(bin_dir)
}

fn local_bin_dir() -> Option<PathBuf> {
    // Prefer XDG/local conventions that login shells usually put on PATH.
    if let Some(home) = env::var_os("HOME") {
        return Some(PathBuf::from(home).join(".local").join("bin"));
    }
    None
}

fn discover_rmux_cli() -> Option<PathBuf> {
    // 1. Same directory as the running binary (cargo run / install layout).
    if let Ok(exe) = env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let candidate = dir.join("rmux-cli");
        if candidate.is_file() {
            return Some(candidate);
        }
        // When this process *is* rmux-cli (shim target), use ourselves.
        if exe.file_name().and_then(|s| s.to_str()) == Some("rmux-cli") {
            return Some(exe);
        }
    }
    // 2. PATH lookup.
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join("rmux-cli");
            candidate.is_file().then_some(candidate)
        })
    })
}

fn shell_quote_path(path: &Path) -> String {
    let s = path.display().to_string();
    if s.chars().all(|c| c.is_ascii_alphanumeric() || "/._-+".contains(c)) {
        s
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quote_path_escapes_spaces() {
        assert_eq!(shell_quote_path(Path::new("/tmp/rmux-cli")), "/tmp/rmux-cli");
        let quoted = shell_quote_path(Path::new("/tmp/my cli"));
        assert!(quoted.starts_with('\''), "{quoted}");
        assert!(quoted.contains("my cli"), "{quoted}");
    }

    #[test]
    fn env_u64_parses() {
        // Pure parse helper path via missing keys.
        assert!(env_u64("RMUX_TEST_NO_SUCH_ENV_VAR_XYZ").is_none());
    }
}
