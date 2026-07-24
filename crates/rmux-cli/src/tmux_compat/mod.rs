//! Tmux compatibility layer for agent team integrations.
//!
//! Agents (Claude Code Teams, OMO, …) shell out to `tmux`. A PATH shim
//! redirects those calls here; we translate them into rmux socket API
//! methods so teammates open as native splits.

mod parse;
mod store;

use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::socket;
use parse::{TmuxCommand, keys_to_text, parse_tmux_args};
use store::TmuxCompatStore;

/// Run `__tmux-compat` with the given argv (including optional leading `tmux`).
///
/// Prints to stdout when commands produce listing output. Failures that would
/// crash an agent are soft-failed (exit via Result::Ok with warning on stderr)
/// for unknown/noop commands; real socket errors still return Err.
pub fn run(socket_path: &Path, args: &[String]) -> Result<()> {
    let mut store = TmuxCompatStore::load().unwrap_or_else(|_| {
        // Fallback empty in-memory if HOME missing
        TmuxCompatStore::load_from(Path::new("/tmp/rmux-tmux-compat-store.json"))
            .expect("fallback store")
    });
    let _ = store.ensure_current_from_env();

    let cmd = parse_tmux_args(args);
    match cmd {
        TmuxCommand::SplitWindow { horizontal, target, cwd, shell } => {
            run_split(
                socket_path,
                &mut store,
                horizontal,
                target.as_deref(),
                cwd.as_deref(),
                &shell,
            )?;
        }
        TmuxCommand::SendKeys { target, keys } => {
            run_send_keys(socket_path, &store, target.as_deref(), &keys)?;
        }
        TmuxCommand::SelectPane { target } => {
            run_select(socket_path, &mut store, target.as_deref())?;
        }
        TmuxCommand::ListPanes { format, all: _ } => {
            run_list_panes(&store, format.as_deref());
        }
        TmuxCommand::ListWindows { format } => {
            run_list_windows(socket_path, format.as_deref())?;
        }
        TmuxCommand::KillPane { target } => {
            run_kill(socket_path, &mut store, target.as_deref())?;
        }
        TmuxCommand::NewWindow { shell } => {
            run_new_window(socket_path, &mut store, &shell)?;
        }
        TmuxCommand::Noop { name } => {
            // Soft success so agents probing tmux options don't die.
            if name != "empty" {
                eprintln!("rmux __tmux-compat: ignoring unsupported tmux command: {name}");
            }
        }
    }

    let _ = store.save();
    Ok(())
}

fn run_split(
    socket_path: &Path,
    store: &mut TmuxCompatStore,
    horizontal: bool,
    target: Option<&str>,
    cwd: Option<&str>,
    shell: &[String],
) -> Result<()> {
    // Focus target before split so the split attaches to the right leaf.
    if let Some((ws, pane)) = store.resolve_target(target) {
        let _ = socket::call(socket_path, "surface.focus", json!({ "pane_id": pane }));
        let _ = ws; // workspace_id available if we need select later
    }

    let direction = if horizontal { "right" } else { "down" };
    let result = socket::call(socket_path, "surface.split", json!({ "direction": direction }))
        .context("surface.split failed")?;

    let new_pane =
        result.get("pane_id").and_then(Value::as_u64).context("surface.split missing pane_id")?;

    // Infer workspace from active/env or store active.
    let workspace_id = store
        .resolve_target(None)
        .map(|(ws, _)| ws)
        .or_else(|| std::env::var("RMUX_WORKSPACE_ID").ok().and_then(|s| s.parse().ok()))
        .unwrap_or(0);

    let fake = store.alloc_fake(workspace_id, new_pane);
    store.set_active(&fake);

    // Optional: cd then run shell command in the new pane.
    let mut text = String::new();
    if let Some(dir) = cwd.filter(|d| !d.is_empty()) {
        text.push_str("cd -- ");
        text.push_str(&shell_single_quote(dir));
        text.push_str(" && ");
    }
    if !shell.is_empty() {
        // Join shell tokens; wrap whole body for safety (cmux issue #6447 lesson).
        let body = shell.join(" ");
        text.push_str(&body);
        text.push('\r');
        let _ = socket::call(
            socket_path,
            "surface.send_text",
            json!({ "pane_id": new_pane, "text": text }),
        );
    } else if let Some(dir) = cwd.filter(|d| !d.is_empty()) {
        let cd = format!("cd -- {} \r", shell_single_quote(dir));
        let _ = socket::call(
            socket_path,
            "surface.send_text",
            json!({ "pane_id": new_pane, "text": cd }),
        );
    }

    // Print new pane id like tmux sometimes does (some agents parse).
    println!("{fake}");
    Ok(())
}

fn run_send_keys(
    socket_path: &Path,
    store: &TmuxCompatStore,
    target: Option<&str>,
    keys: &[String],
) -> Result<()> {
    let text = keys_to_text(keys);
    if text.is_empty() {
        return Ok(());
    }
    let pane_id = store.resolve_target(target).map(|(_, p)| p);
    let mut params = json!({ "text": text });
    if let Some(pid) = pane_id {
        params["pane_id"] = json!(pid);
    }
    socket::call(socket_path, "surface.send_text", params).context("surface.send_text")?;
    Ok(())
}

fn run_select(socket_path: &Path, store: &mut TmuxCompatStore, target: Option<&str>) -> Result<()> {
    let Some((_, pane_id)) = store.resolve_target(target) else {
        return Ok(());
    };
    if let Some(fake) = store.fake_for_pane(pane_id) {
        store.set_active(&fake);
    }
    socket::call(socket_path, "surface.focus", json!({ "pane_id": pane_id }))
        .context("surface.focus")?;
    Ok(())
}

fn run_list_panes(store: &TmuxCompatStore, format: Option<&str>) {
    let fmt = format.unwrap_or("#{pane_id}");
    for (fake, _ws, pane) in store.all_panes() {
        let active = if fake == store.active() { "1" } else { "0" };
        let line = fmt
            .replace("#{pane_id}", &fake)
            .replace("#{pane_index}", fake.trim_start_matches('%'))
            .replace("#{pane_active}", active)
            .replace("#{pane_current_command}", "shell")
            .replace("#{pane_title}", &format!("rmux-{pane}"));
        println!("{line}");
    }
}

fn run_list_windows(socket_path: &Path, format: Option<&str>) -> Result<()> {
    let fmt = format.unwrap_or("#{window_index}:#{window_name}");
    let result = socket::call(socket_path, "workspace.list", json!({})).unwrap_or(json!({}));
    let empty = Vec::new();
    let workspaces = result
        .as_array()
        .or_else(|| result.get("workspaces").and_then(Value::as_array))
        .unwrap_or(&empty);
    for (i, ws) in workspaces.iter().enumerate() {
        let name = ws.get("name").and_then(Value::as_str).unwrap_or("workspace");
        let id = ws.get("id").map(|v| v.to_string()).unwrap_or_else(|| i.to_string());
        let line = fmt
            .replace("#{window_index}", &i.to_string())
            .replace("#{window_name}", name)
            .replace("#{window_id}", &id);
        println!("{line}");
    }
    Ok(())
}

fn run_kill(socket_path: &Path, store: &mut TmuxCompatStore, target: Option<&str>) -> Result<()> {
    let Some((_, pane_id)) = store.resolve_target(target) else {
        return Ok(());
    };
    let _ = socket::call(socket_path, "surface.close", json!({ "pane_id": pane_id }));
    if let Some(fake) = store.fake_for_pane(pane_id) {
        store.remove_fake(&fake);
    }
    Ok(())
}

fn run_new_window(socket_path: &Path, store: &mut TmuxCompatStore, shell: &[String]) -> Result<()> {
    let result = socket::call(socket_path, "workspace.create", json!({ "name": null }))
        .context("workspace.create")?;
    let workspace_id = result
        .get("id")
        .or_else(|| result.get("workspace_id"))
        .and_then(Value::as_u64)
        .unwrap_or(0);

    // List surfaces to find a pane in the new workspace (best-effort).
    let list = socket::call(socket_path, "surface.list", json!({})).unwrap_or(json!({}));
    let surfaces = list.get("surfaces").and_then(Value::as_array).cloned().unwrap_or_default();
    let pane_id = surfaces
        .iter()
        .rev()
        .find(|s| s.get("workspace_id").and_then(Value::as_u64) == Some(workspace_id))
        .and_then(|s| s.get("pane_id").and_then(Value::as_u64))
        .or_else(|| surfaces.last().and_then(|s| s.get("pane_id").and_then(Value::as_u64)))
        .unwrap_or(0);

    let fake = store.alloc_fake(workspace_id, pane_id);
    store.set_active(&fake);

    if !shell.is_empty() {
        let text = format!("{}\r", shell.join(" "));
        let _ = socket::call(
            socket_path,
            "surface.send_text",
            json!({ "pane_id": pane_id, "text": text }),
        );
    }
    println!("{fake}");
    Ok(())
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::parse::{TmuxCommand, parse_tmux_args};

    #[test]
    fn dispatcher_maps_h_to_right_concept() {
        let cmd = parse_tmux_args(&["split-window".into(), "-h".into()]);
        assert!(matches!(cmd, TmuxCommand::SplitWindow { horizontal: true, .. }));
    }
}
