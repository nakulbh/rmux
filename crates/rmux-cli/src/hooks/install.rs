//! Install / uninstall agent hook configuration for Claude Code and OpenCode.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};

use super::registry::{
    AgentId, OPENCODE_PLUGIN_MARKER, binary_on_path, claude_settings_path, ensure_parent_dir,
    opencode_config_dir, rmux_cli_command, shell_double_quote,
};

/// Which agents to install/uninstall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentChoice {
    /// All agents whose binary is on PATH.
    All,
    /// Only Claude Code.
    Claude,
    /// Only OpenCode.
    OpenCode,
}

impl AgentChoice {
    /// Agents this choice expands to (before PATH filtering).
    fn agents(self) -> Vec<AgentId> {
        match self {
            Self::All => AgentId::all().to_vec(),
            Self::Claude => vec![AgentId::Claude],
            Self::OpenCode => vec![AgentId::OpenCode],
        }
    }
}

/// Result line printed for one agent during setup/uninstall.
#[derive(Debug, Clone)]
pub struct InstallOutcome {
    pub agent: AgentId,
    pub status: InstallStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStatus {
    Installed,
    Uninstalled,
    Skipped,
    Error,
}

/// Install hooks for the chosen agents. Skips agents not on PATH when `require_binary`.
pub fn install_agents(choice: AgentChoice, require_binary: bool) -> Result<Vec<InstallOutcome>> {
    let cli = rmux_cli_command();
    let mut outcomes = Vec::new();

    for agent in choice.agents() {
        if require_binary && !binary_on_path(agent.binary_name()) {
            outcomes.push(InstallOutcome {
                agent,
                status: InstallStatus::Skipped,
                detail: format!("{} not found on PATH", agent.binary_name()),
            });
            continue;
        }
        let result = match agent {
            AgentId::Claude => install_claude(&cli),
            AgentId::OpenCode => install_opencode(&cli),
        };
        match result {
            Ok(detail) => {
                outcomes.push(InstallOutcome { agent, status: InstallStatus::Installed, detail })
            }
            Err(e) => outcomes.push(InstallOutcome {
                agent,
                status: InstallStatus::Error,
                detail: e.to_string(),
            }),
        }
    }

    Ok(outcomes)
}

/// Remove rmux-owned hooks for the chosen agents.
pub fn uninstall_agents(choice: AgentChoice) -> Result<Vec<InstallOutcome>> {
    let mut outcomes = Vec::new();
    for agent in choice.agents() {
        let result = match agent {
            AgentId::Claude => uninstall_claude(),
            AgentId::OpenCode => uninstall_opencode(),
        };
        match result {
            Ok(detail) => {
                outcomes.push(InstallOutcome { agent, status: InstallStatus::Uninstalled, detail })
            }
            Err(e) => outcomes.push(InstallOutcome {
                agent,
                status: InstallStatus::Error,
                detail: e.to_string(),
            }),
        }
    }
    Ok(outcomes)
}

// ── Claude Code ──────────────────────────────────────────────────────────────

/// Substring that identifies an rmux-owned Claude hook command.
const CLAUDE_CMD_MARKER: &str = "hooks claude";

fn claude_hook_command(cli: &str, event: &str) -> String {
    format!(
        "[ \"${{RMUX_CLAUDE_HOOKS_DISABLED:-}}\" != \"1\" ] && [ \"${{RMUX_HOOKS_DISABLED:-}}\" != \"1\" ] && {} hooks claude {} || true",
        shell_double_quote(cli),
        event
    )
}

fn claude_command_hook(cli: &str, event: &str, timeout: u64, async_flag: bool) -> Value {
    let mut hook = Map::new();
    hook.insert("type".into(), json!("command"));
    hook.insert("command".into(), json!(claude_hook_command(cli, event)));
    hook.insert("timeout".into(), json!(timeout));
    if async_flag {
        hook.insert("async".into(), json!(true));
    }
    json!({ "matcher": "", "hooks": [hook] })
}

fn claude_command_hook_matcher(
    cli: &str,
    event: &str,
    matcher: &str,
    timeout: u64,
    async_flag: bool,
) -> Value {
    let mut entry = claude_command_hook(cli, event, timeout, async_flag);
    if let Some(obj) = entry.as_object_mut() {
        obj.insert("matcher".into(), json!(matcher));
    }
    entry
}

/// Build the hooks object rmux merges into Claude settings.
pub fn build_claude_hooks_object(cli: &str) -> Value {
    json!({
        "SessionStart": [claude_command_hook(cli, "session-start", 10, false)],
        "UserPromptSubmit": [claude_command_hook(cli, "prompt-submit", 10, false)],
        "Stop": [claude_command_hook(cli, "stop", 10, false)],
        "Notification": [claude_command_hook(cli, "notification", 10, false)],
        "SessionEnd": [claude_command_hook(cli, "session-end", 1, false)],
        "PostToolUse": [claude_command_hook_matcher(
            cli,
            "push-notification",
            "PushNotification",
            10,
            true
        )],
    })
}

fn install_claude(cli: &str) -> Result<String> {
    let path =
        claude_settings_path().context("HOME not set; cannot locate ~/.claude/settings.json")?;
    install_claude_at(cli, &path)
}

/// Install Claude hooks into an explicit settings path (testable without mutating env).
pub fn install_claude_at(cli: &str, path: &Path) -> Result<String> {
    ensure_parent_dir(path)?;

    let mut root = read_json_object(path)?;
    let hooks = root.entry("hooks".to_owned()).or_insert_with(|| json!({}));
    let hooks_obj =
        hooks.as_object_mut().context("Claude settings.json: hooks is not an object")?;

    // Strip any previous rmux-owned entries, then add fresh ones.
    strip_rmux_claude_hooks(hooks_obj);

    let new_hooks = build_claude_hooks_object(cli);
    let new_map = new_hooks.as_object().expect("hooks object");
    for (event, entries) in new_map {
        let list = hooks_obj.entry(event.clone()).or_insert_with(|| json!([]));
        let arr = list.as_array_mut().context(format!("hooks.{event} is not an array"))?;
        if let Some(new_entries) = entries.as_array() {
            for entry in new_entries {
                arr.push(entry.clone());
            }
        }
    }

    // Prefer disabling Claude's own OSC notifications so we don't double-fire
    // (rmux OSC scanner is also disabled, but other terminals may still show them).
    if !root.contains_key("preferredNotifChannel") {
        root.insert("preferredNotifChannel".into(), json!("notifications_disabled"));
    }

    write_json_pretty(path, &Value::Object(root))?;
    Ok(format!("merged hooks into {}", path.display()))
}

fn uninstall_claude() -> Result<String> {
    let path = claude_settings_path().context("HOME not set")?;
    uninstall_claude_at(&path)
}

/// Uninstall Claude hooks from an explicit settings path.
pub fn uninstall_claude_at(path: &Path) -> Result<String> {
    if !path.exists() {
        return Ok("no settings.json".to_owned());
    }
    let mut root = read_json_object(path)?;
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return Ok("no hooks key".to_owned());
    };
    strip_rmux_claude_hooks(hooks);
    // Drop empty event arrays
    hooks.retain(|_, v| v.as_array().is_none_or(|a| !a.is_empty()));
    write_json_pretty(path, &Value::Object(root))?;
    Ok(format!("removed rmux hooks from {}", path.display()))
}

fn strip_rmux_claude_hooks(hooks: &mut Map<String, Value>) {
    for (_event, value) in hooks.iter_mut() {
        let Some(arr) = value.as_array_mut() else {
            continue;
        };
        arr.retain(|entry| !entry_contains_rmux_claude_cmd(entry));
    }
}

fn entry_contains_rmux_claude_cmd(entry: &Value) -> bool {
    let Some(hooks) = entry.get("hooks").and_then(Value::as_array) else {
        return false;
    };
    hooks.iter().any(|h| {
        h.get("command").and_then(Value::as_str).is_some_and(|c| c.contains(CLAUDE_CMD_MARKER))
    })
}

// ── OpenCode ─────────────────────────────────────────────────────────────────

const PLUGIN_REL: &str = "plugins/rmux-notify.js";
const PLUGIN_REG: &str = "./plugins/rmux-notify.js";

fn install_opencode(cli: &str) -> Result<String> {
    let config_dir =
        opencode_config_dir().context("HOME not set; cannot locate OpenCode config")?;
    install_opencode_at(cli, &config_dir)
}

/// Install OpenCode plugin into an explicit config directory.
pub fn install_opencode_at(cli: &str, config_dir: &Path) -> Result<String> {
    let plugin_path = config_dir.join(PLUGIN_REL);
    ensure_parent_dir(&plugin_path)?;

    let plugin_src = opencode_plugin_source(cli);
    fs::write(&plugin_path, plugin_src)
        .with_context(|| format!("write {}", plugin_path.display()))?;

    let config_path = config_dir.join("opencode.json");
    let mut root = if config_path.exists() { read_json_object(&config_path)? } else { Map::new() };

    let plugins = root.entry("plugin".to_owned()).or_insert_with(|| json!([]));
    let arr = plugins.as_array_mut().context("opencode.json: plugin is not an array")?;

    // Remove stale registrations of our plugin (by path or bare name).
    arr.retain(|entry| !is_rmux_opencode_plugin_entry(entry));
    arr.push(json!(PLUGIN_REG));

    write_json_pretty(&config_path, &Value::Object(root))?;
    Ok(format!("wrote {} and registered in {}", plugin_path.display(), config_path.display()))
}

fn uninstall_opencode() -> Result<String> {
    let config_dir = opencode_config_dir().context("HOME not set")?;
    uninstall_opencode_at(&config_dir)
}

/// Uninstall OpenCode plugin from an explicit config directory.
pub fn uninstall_opencode_at(config_dir: &Path) -> Result<String> {
    let plugin_path = config_dir.join(PLUGIN_REL);
    let mut removed = Vec::new();

    if plugin_path.exists() {
        fs::remove_file(&plugin_path)
            .with_context(|| format!("remove {}", plugin_path.display()))?;
        removed.push(plugin_path.display().to_string());
    }

    let config_path = config_dir.join("opencode.json");
    if config_path.exists() {
        let mut root = read_json_object(&config_path)?;
        if let Some(arr) = root.get_mut("plugin").and_then(Value::as_array_mut) {
            arr.retain(|entry| !is_rmux_opencode_plugin_entry(entry));
        }
        write_json_pretty(&config_path, &Value::Object(root))?;
        removed.push(format!("unregistered from {}", config_path.display()));
    }

    if removed.is_empty() { Ok("nothing to remove".to_owned()) } else { Ok(removed.join("; ")) }
}

fn is_rmux_opencode_plugin_entry(entry: &Value) -> bool {
    match entry {
        Value::String(s) => {
            s.contains("rmux-notify") || s == "rmux-session" || s.ends_with("rmux-notify.js")
        }
        Value::Array(a) => {
            a.first().and_then(Value::as_str).is_some_and(|s| s.contains("rmux-notify"))
        }
        _ => false,
    }
}

/// Generate the OpenCode plugin JS source.
pub fn opencode_plugin_source(cli: &str) -> String {
    // Escape for embedding inside a JS single-quoted string.
    let cli_js = cli.replace('\\', "\\\\").replace('\'', "\\'");
    format!(
        r#"// {OPENCODE_PLUGIN_MARKER}
// Bridges OpenCode plugin events to rmux notifications.
// Installed by `rmux-cli hooks setup`. DO NOT EDIT MANUALLY.

import {{ spawn }} from "node:child_process";

const RMUX_CLI = process.env.RMUX_OPENCODE_CLI || '{cli_js}';
const DISABLED =
  process.env.RMUX_OPENCODE_HOOKS_DISABLED === "1" ||
  process.env.RMUX_HOOKS_DISABLED === "1";

function runHook(event, payload) {{
  if (DISABLED) return;
  try {{
    const child = spawn(RMUX_CLI, ["hooks", "opencode", event], {{
      stdio: ["pipe", "ignore", "ignore"],
      env: process.env,
    }});
    child.on("error", () => {{}});
    try {{
      child.stdin.write(JSON.stringify(payload ?? {{}}));
      child.stdin.end();
    }} catch (_) {{}}
  }} catch (_) {{}}
}}

export const RmuxNotify = async () => {{
  return {{
    event: async ({{ event }}) => {{
      if (!event || typeof event !== "object") return;
      const type = event.type || "";
      const payload = {{
        type,
        ...(event.properties && typeof event.properties === "object"
          ? event.properties
          : {{}}),
      }};
      if (type === "session.created" || type === "session.updated") {{
        runHook("session-start", payload);
      }} else if (type === "session.idle") {{
        runHook("stop", payload);
      }} else if (type === "session.error") {{
        runHook("notification", {{ ...payload, message: event.properties?.error || "Session error" }});
      }} else if (type === "permission.asked") {{
        runHook("notification", {{
          ...payload,
          notification_type: "permission_prompt",
          message: event.properties?.permission || "Permission required",
        }});
      }} else if (type === "session.status") {{
        runHook("status", {{
          ...payload,
          status: event.properties?.status || event.properties?.type || "",
        }});
      }} else if (type === "tool.execute.before") {{
        runHook("status", {{ status: "running" }});
      }} else if (type === "tool.execute.after") {{
        // keep Running until session.idle
      }}
    }},
  }};
}};

export default RmuxNotify;
"#
    )
}

// ── JSON helpers ─────────────────────────────────────────────────────────────

fn read_json_object(path: &Path) -> Result<Map<String, Value>> {
    if !path.exists() {
        return Ok(Map::new());
    }
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    if text.trim().is_empty() {
        return Ok(Map::new());
    }
    let value: Value =
        serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    match value {
        Value::Object(map) => Ok(map),
        _ => bail!("{}: root must be a JSON object", path.display()),
    }
}

fn write_json_pretty(path: &Path, value: &Value) -> Result<()> {
    let text = serde_json::to_string_pretty(value).context("serialize JSON")?;
    // Atomic-ish write via temp sibling.
    let tmp = PathBuf::from(format!("{}.tmp", path.display()));
    fs::write(&tmp, format!("{text}\n")).with_context(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("rename onto {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_home() -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("rmux-hooks-test-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn claude_hooks_object_contains_stop_and_notification() {
        let hooks = build_claude_hooks_object("/usr/local/bin/rmux-cli");
        let stop = hooks.get("Stop").and_then(Value::as_array).unwrap();
        assert!(!stop.is_empty());
        let cmd = stop[0]["hooks"][0]["command"].as_str().unwrap();
        assert!(cmd.contains("hooks claude stop"));
        assert!(cmd.contains("/usr/local/bin/rmux-cli"));
    }

    #[test]
    fn claude_install_merge_preserves_user_hooks() {
        let home = temp_home();
        let settings = home.join(".claude").join("settings.json");
        fs::create_dir_all(settings.parent().unwrap()).unwrap();
        fs::write(
            &settings,
            r#"{
  "hooks": {
    "Stop": [
      { "matcher": "", "hooks": [{ "type": "command", "command": "echo user" }] }
    ]
  }
}
"#,
        )
        .unwrap();

        install_claude_at("/tmp/rmux-cli", &settings).unwrap();

        let text = fs::read_to_string(&settings).unwrap();
        assert!(text.contains("echo user"));
        assert!(text.contains("hooks claude stop"));

        uninstall_claude_at(&settings).unwrap();
        let text = fs::read_to_string(&settings).unwrap();
        assert!(text.contains("echo user"));
        assert!(!text.contains("hooks claude"));

        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn opencode_plugin_contains_marker_and_events() {
        let src = opencode_plugin_source("/opt/rmux-cli");
        assert!(src.contains(OPENCODE_PLUGIN_MARKER));
        assert!(src.contains("session.idle"));
        assert!(src.contains("permission.asked"));
        assert!(src.contains("/opt/rmux-cli"));
    }

    #[test]
    fn opencode_install_registers_plugin() {
        let home = temp_home();
        let config_dir = home.join(".config").join("opencode");
        install_opencode_at("/tmp/rmux-cli", &config_dir).unwrap();
        let plugin = config_dir.join("plugins/rmux-notify.js");
        assert!(plugin.exists());
        let cfg: Value =
            serde_json::from_str(&fs::read_to_string(config_dir.join("opencode.json")).unwrap())
                .unwrap();
        let plugins = cfg["plugin"].as_array().unwrap();
        assert!(plugins.iter().any(|p| p.as_str() == Some("./plugins/rmux-notify.js")));

        uninstall_opencode_at(&config_dir).unwrap();
        assert!(!plugin.exists());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn strip_detects_rmux_command() {
        let entry = json!({
            "matcher": "",
            "hooks": [{ "type": "command", "command": "\"/bin/rmux-cli\" hooks claude stop || true" }]
        });
        assert!(entry_contains_rmux_claude_cmd(&entry));
        let user = json!({
            "matcher": "",
            "hooks": [{ "type": "command", "command": "echo hi" }]
        });
        assert!(!entry_contains_rmux_claude_cmd(&user));
    }
}
