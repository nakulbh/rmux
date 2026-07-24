//! `rmux-cli claude-teams` — launch Claude Code with agent teams + tmux shim.

use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// Run Claude Code with experimental agent teams and a PATH `tmux` shim.
///
/// Remaining `claude_args` are forwarded to `claude` (after optional default
/// teammate-mode flag).
pub fn run(claude_args: &[String]) -> Result<()> {
    let home =
        env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")).context("HOME not set")?;
    let home = PathBuf::from(home);
    let bin_dir = home.join(".rmuxterm").join("claude-teams-bin");
    fs::create_dir_all(&bin_dir).with_context(|| format!("mkdir {}", bin_dir.display()))?;

    let rmux_cli = env::current_exe().context("current_exe for rmux-cli")?;
    write_tmux_shim(&bin_dir, &rmux_cli)?;

    let claude = which("claude")
        .context("claude not found on PATH — install Claude Code CLI before using claude-teams")?;

    let mut path = bin_dir.display().to_string();
    if let Ok(existing) = env::var("PATH") {
        path.push(':');
        path.push_str(&existing);
    }

    // Fake TMUX env: encode workspace/pane when running inside rmux.
    let workspace = env::var("RMUX_WORKSPACE_ID").unwrap_or_else(|_| "0".into());
    let pane = env::var("RMUX_PANE_ID").unwrap_or_else(|_| "0".into());
    let tmux_socket = format!("rmux,{workspace},{pane}");
    let tmux_pane = format!("%rmux{pane}");

    let mut cmd = Command::new(&claude);
    cmd.env("PATH", &path);
    cmd.env("TMUX", &tmux_socket);
    cmd.env("TMUX_PANE", &tmux_pane);
    cmd.env("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS", "1");
    // Keep rmux socket path for hooks + tmux-compat.
    if let Ok(sock) = env::var("RMUX_SOCKET_PATH") {
        cmd.env("RMUX_SOCKET_PATH", sock);
    }
    cmd.env("RMUX_TMUX_COMPAT", "1");

    // Claude accepts experimental teams via env; forward user argv unchanged.
    cmd.args(claude_args);
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    // exec semantics: replace process when possible
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        bail!("failed to exec claude: {err}");
    }
    #[cfg(not(unix))]
    {
        let status = cmd.status().context("spawn claude")?;
        if status.success() {
            Ok(())
        } else {
            bail!("claude exited with {status}");
        }
    }
}

fn write_tmux_shim(bin_dir: &Path, rmux_cli: &Path) -> Result<()> {
    let shim = bin_dir.join("tmux");
    let body = format!(
        "#!/bin/sh\n# rmux tmux shim — DO NOT EDIT\n# Redirects tmux calls to rmux-cli __tmux-compat\nexec {} __tmux-compat \"$@\"\n",
        shell_double_quote(&rmux_cli.display().to_string())
    );
    fs::write(&shim, body).with_context(|| format!("write {}", shim.display()))?;
    let mut perms = fs::metadata(&shim)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&shim, perms)?;
    Ok(())
}

fn shell_double_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn which(binary: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join(binary);
            if candidate.is_file() { Some(candidate) } else { None }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shim_script_invokes_tmux_compat() {
        let dir = std::env::temp_dir().join(format!("rmux-claude-teams-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let fake_cli = dir.join("rmux-cli");
        fs::write(&fake_cli, "#!/bin/sh\n").unwrap();
        write_tmux_shim(&dir, &fake_cli).unwrap();
        let text = fs::read_to_string(dir.join("tmux")).unwrap();
        assert!(text.contains("__tmux-compat"));
        assert!(text.contains("rmux-cli"));
        let _ = fs::remove_dir_all(&dir);
    }
}
