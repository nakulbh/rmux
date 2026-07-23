//! Parse agent hook stdin JSON and emit notifications + sidebar status.

use std::io::{self, Read};
use std::path::Path;

use anyhow::Result;
use serde_json::{Value, json};

use super::registry::{AgentId, agent_hooks_disabled};
use crate::socket;

/// Claude Code hook event names handled by `rmux-cli hooks claude …`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeEvent {
    SessionStart,
    PromptSubmit,
    Stop,
    Notification,
    PushNotification,
    SessionEnd,
}

impl ClaudeEvent {
    /// Parse from CLI subcommand name.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "session-start" => Some(Self::SessionStart),
            "prompt-submit" => Some(Self::PromptSubmit),
            "stop" => Some(Self::Stop),
            "notification" => Some(Self::Notification),
            "push-notification" => Some(Self::PushNotification),
            "session-end" => Some(Self::SessionEnd),
            _ => None,
        }
    }
}

/// OpenCode plugin-bridged events handled by `rmux-cli hooks opencode …`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenCodeEvent {
    SessionStart,
    Stop,
    Notification,
    Status,
}

impl OpenCodeEvent {
    /// Parse from CLI subcommand name.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "session-start" => Some(Self::SessionStart),
            "stop" => Some(Self::Stop),
            "notification" => Some(Self::Notification),
            "status" => Some(Self::Status),
            _ => None,
        }
    }
}

/// Classification of a user-visible agent signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedSignal {
    /// Sidebar status text (`Running`, `Idle`, `Needs input`, `Error`).
    pub status: Option<&'static str>,
    /// Whether to create a notification.
    pub notify: bool,
    /// Notification subtitle.
    pub subtitle: Option<&'static str>,
    /// Notification body (already truncated).
    pub body: String,
}

/// Read all of stdin as UTF-8 (best-effort).
fn read_stdin() -> String {
    let mut buf = String::new();
    let _ = io::stdin().read_to_string(&mut buf);
    buf
}

/// Parse stdin as JSON object; empty / invalid → empty object.
fn parse_stdin_json(raw: &str) -> Value {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return json!({});
    }
    serde_json::from_str(trimmed).unwrap_or_else(|_| json!({}))
}

/// First non-empty string among `keys` in `obj` (and nested `notification`/`data`).
fn first_string(obj: &Value, keys: &[&str]) -> Option<String> {
    let candidates = [
        obj,
        obj.get("notification").unwrap_or(&Value::Null),
        obj.get("data").unwrap_or(&Value::Null),
        obj.get("properties").unwrap_or(&Value::Null),
        obj.get("info").unwrap_or(&Value::Null),
    ];
    for candidate in candidates {
        if let Some(map) = candidate.as_object() {
            for key in keys {
                if let Some(s) = map.get(*key).and_then(Value::as_str) {
                    let t = s.trim();
                    if !t.is_empty() {
                        return Some(t.to_owned());
                    }
                }
            }
        }
    }
    None
}

/// Collapse whitespace and cap length for notification bodies.
fn normalize_body(text: &str, max: usize) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max {
        collapsed
    } else {
        let truncated: String = collapsed.chars().take(max.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

/// Classify free-form signal + message into status / notify fields.
pub fn classify_signal(display_name: &str, signal: &str, message: &str) -> ClassifiedSignal {
    let lower = format!("{} {}", signal.to_lowercase(), message.to_lowercase());
    let body_src = message.trim();

    if lower.contains("permission")
        || lower.contains("approve")
        || lower.contains("approval")
        || lower.contains("permission_prompt")
    {
        let body = if body_src.is_empty() {
            "Approval needed".to_owned()
        } else {
            normalize_body(body_src, 180)
        };
        return ClassifiedSignal {
            status: Some("Needs input"),
            notify: true,
            subtitle: Some("Permission"),
            body,
        };
    }

    if lower.contains("error")
        || lower.contains("failed")
        || lower.contains("failure")
        || lower.contains("exception")
    {
        let body = if body_src.is_empty() {
            format!("{display_name} reported an error")
        } else {
            normalize_body(body_src, 180)
        };
        return ClassifiedSignal {
            status: Some("Error"),
            notify: true,
            subtitle: Some("Error"),
            body,
        };
    }

    // Waiting cues before completion: `idle_prompt` contains "idle" but means needs input.
    if contains_waiting_cue(&lower) {
        let body = if body_src.is_empty() {
            "Waiting for input".to_owned()
        } else {
            normalize_body(body_src, 180)
        };
        return ClassifiedSignal {
            status: Some("Needs input"),
            notify: true,
            subtitle: Some("Waiting"),
            body,
        };
    }

    if contains_completion_cue(&lower) {
        let body = if body_src.is_empty() {
            "Task completed".to_owned()
        } else {
            normalize_body(body_src, 180)
        };
        return ClassifiedSignal {
            status: Some("Idle"),
            notify: true,
            subtitle: Some("Completed"),
            body,
        };
    }

    if !body_src.is_empty() {
        return ClassifiedSignal {
            status: Some("Needs input"),
            notify: true,
            subtitle: Some("Attention"),
            body: normalize_body(body_src, 180),
        };
    }

    ClassifiedSignal {
        status: Some("Needs input"),
        notify: true,
        subtitle: Some("Attention"),
        body: format!("{display_name} needs your attention"),
    }
}

fn contains_completion_cue(lower: &str) -> bool {
    lower.contains("complete")
        || lower.contains("completed")
        || lower.contains("finished")
        || lower.contains("done")
        || lower.contains("session.idle")
        || lower.contains("stop")
        || lower.contains("turn complete")
        || lower.contains("agent_completed")
}

fn contains_waiting_cue(lower: &str) -> bool {
    lower.contains("waiting")
        || lower.contains("idle_prompt")
        || lower.contains("needs input")
        || lower.contains("needs_input")
        || lower.contains("agent_needs_input")
        || lower.contains("input required")
}

/// Routing context from env / flags.
#[derive(Debug, Clone, Default)]
pub struct RoutingContext {
    pub workspace_id: Option<u64>,
    pub pane_id: Option<u64>,
}

impl RoutingContext {
    /// Build from process environment (`RMUX_WORKSPACE_ID`, `RMUX_PANE_ID`).
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            workspace_id: std::env::var("RMUX_WORKSPACE_ID").ok().and_then(|s| s.parse().ok()),
            pane_id: std::env::var("RMUX_PANE_ID").ok().and_then(|s| s.parse().ok()),
        }
    }
}

/// Apply a classified signal to the running rmux instance.
///
/// Silently succeeds when the socket is unreachable (agent not inside rmux).
fn apply_signal(
    socket_path: &Path,
    agent: AgentId,
    signal: &ClassifiedSignal,
    routing: &RoutingContext,
) -> Result<()> {
    if let Some(status) = signal.status {
        let params = json!({
            "workspace_id": routing.workspace_id,
            "status": status,
        });
        // Best-effort: ignore connect failures.
        let _ = socket::call(socket_path, "sidebar.set_status", params);
    }

    if signal.notify {
        let params = json!({
            "title": agent.display_name(),
            "subtitle": signal.subtitle,
            "body": signal.body,
            "workspace_id": routing.workspace_id,
            "pane_id": routing.pane_id,
        });
        let _ = socket::call(socket_path, "notification.create", params);
    }

    Ok(())
}

fn clear_status(socket_path: &Path, routing: &RoutingContext) {
    let params = json!({ "workspace_id": routing.workspace_id });
    let _ = socket::call(socket_path, "sidebar.clear_status", params);
}

fn set_running(socket_path: &Path, routing: &RoutingContext) {
    let params = json!({
        "workspace_id": routing.workspace_id,
        "status": "Running",
    });
    let _ = socket::call(socket_path, "sidebar.set_status", params);
}

/// Handle a Claude Code hook event. Always returns Ok (fail-open for agents).
pub fn handle_claude_event(socket_path: &Path, event: ClaudeEvent) -> Result<()> {
    if agent_hooks_disabled(AgentId::Claude) {
        return Ok(());
    }

    let raw = read_stdin();
    let obj = parse_stdin_json(&raw);
    let routing = RoutingContext::from_env();
    let agent = AgentId::Claude;

    match event {
        ClaudeEvent::SessionStart | ClaudeEvent::PromptSubmit => {
            set_running(socket_path, &routing);
        }
        ClaudeEvent::SessionEnd => {
            clear_status(socket_path, &routing);
        }
        ClaudeEvent::Stop => {
            let message = first_string(
                &obj,
                &["last_assistant_message", "message", "body", "text", "summary"],
            )
            .unwrap_or_default();
            // Stop always means turn complete → Idle + notify.
            let signal = ClassifiedSignal {
                status: Some("Idle"),
                notify: true,
                subtitle: Some("Completed"),
                body: if message.is_empty() {
                    "Session complete".to_owned()
                } else {
                    normalize_body(&message, 180)
                },
            };
            let _ = apply_signal(socket_path, agent, &signal, &routing);
        }
        ClaudeEvent::Notification => {
            let notif_type =
                first_string(&obj, &["notification_type", "type", "matcher", "hook_event_name"])
                    .unwrap_or_else(|| "notification".to_owned());
            let message =
                first_string(&obj, &["message", "body", "text", "title"]).unwrap_or_default();
            let signal = classify_signal(agent.display_name(), &notif_type, &message);
            let _ = apply_signal(socket_path, agent, &signal, &routing);
        }
        ClaudeEvent::PushNotification => {
            let message = push_notification_message(&obj).unwrap_or_default();
            if message.is_empty() {
                return Ok(());
            }
            if !push_notification_should_bridge(&obj) {
                return Ok(());
            }
            let signal = ClassifiedSignal {
                status: None, // don't flip lifecycle for model-initiated push
                notify: true,
                subtitle: Some("Push"),
                body: normalize_body(&message, 240),
            };
            let _ = apply_signal(socket_path, agent, &signal, &routing);
        }
    }

    Ok(())
}

fn push_notification_message(obj: &Value) -> Option<String> {
    if let Some(input) = obj.get("tool_input")
        && let Some(s) = first_string(input, &["message"])
    {
        return Some(s);
    }
    if let Some(resp) = obj.get("tool_response")
        && let Some(s) = first_string(resp, &["message"])
    {
        return Some(s);
    }
    first_string(obj, &["message", "body", "text"])
}

fn push_notification_should_bridge(obj: &Value) -> bool {
    let Some(resp) = obj.get("tool_response") else {
        return true;
    };
    !matches!(
        resp.get("disabledReason").and_then(Value::as_str),
        Some("user_present") | Some("config_off")
    )
}

/// Handle an OpenCode plugin-bridged event. Always returns Ok (fail-open).
pub fn handle_opencode_event(socket_path: &Path, event: OpenCodeEvent) -> Result<()> {
    if agent_hooks_disabled(AgentId::OpenCode) {
        return Ok(());
    }

    let raw = read_stdin();
    let obj = parse_stdin_json(&raw);
    let routing = RoutingContext::from_env();
    let agent = AgentId::OpenCode;

    // Plugin may wrap: { "type": "session.idle", "properties": {…} }
    let event_type =
        first_string(&obj, &["type", "event", "event_type", "name"]).unwrap_or_default();
    let message =
        first_string(&obj, &["message", "body", "text", "error", "summary"]).unwrap_or_default();

    match event {
        OpenCodeEvent::SessionStart => {
            set_running(socket_path, &routing);
        }
        OpenCodeEvent::Status => {
            let status_raw = first_string(&obj, &["status", "state"]).unwrap_or_default();
            let lower = status_raw.to_lowercase();
            if lower.contains("run") || lower.contains("busy") || lower.contains("think") {
                set_running(socket_path, &routing);
            } else if lower.contains("idle") || lower.contains("done") {
                let signal = ClassifiedSignal {
                    status: Some("Idle"),
                    notify: false,
                    subtitle: None,
                    body: String::new(),
                };
                let _ = apply_signal(socket_path, agent, &signal, &routing);
            }
        }
        OpenCodeEvent::Stop => {
            let signal = ClassifiedSignal {
                status: Some("Idle"),
                notify: true,
                subtitle: Some("Completed"),
                body: if message.is_empty() {
                    "Task complete".to_owned()
                } else {
                    normalize_body(&message, 180)
                },
            };
            let _ = apply_signal(socket_path, agent, &signal, &routing);
        }
        OpenCodeEvent::Notification => {
            let signal_src =
                if event_type.is_empty() { "notification".to_owned() } else { event_type };
            let signal = classify_signal(agent.display_name(), &signal_src, &message);
            let _ = apply_signal(socket_path, agent, &signal, &routing);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_permission() {
        let s = classify_signal("Claude Code", "permission_prompt", "Allow bash?");
        assert_eq!(s.status, Some("Needs input"));
        assert_eq!(s.subtitle, Some("Permission"));
        assert!(s.notify);
    }

    #[test]
    fn classify_idle_waiting() {
        let s = classify_signal("Claude Code", "idle_prompt", "");
        assert_eq!(s.status, Some("Needs input"));
        assert_eq!(s.subtitle, Some("Waiting"));
    }

    #[test]
    fn classify_completion() {
        let s = classify_signal("OpenCode", "session.idle", "All done");
        assert_eq!(s.status, Some("Idle"));
        assert_eq!(s.subtitle, Some("Completed"));
        assert_eq!(s.body, "All done");
    }

    #[test]
    fn classify_error() {
        let s = classify_signal("OpenCode", "session.error", "boom");
        assert_eq!(s.status, Some("Error"));
        assert_eq!(s.subtitle, Some("Error"));
    }

    #[test]
    fn normalize_truncates() {
        let long = "word ".repeat(100);
        let out = normalize_body(&long, 20);
        assert!(out.chars().count() <= 20);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn claude_event_parse() {
        assert_eq!(ClaudeEvent::parse("stop"), Some(ClaudeEvent::Stop));
        assert_eq!(ClaudeEvent::parse("push-notification"), Some(ClaudeEvent::PushNotification));
        assert_eq!(ClaudeEvent::parse("nope"), None);
    }

    #[test]
    fn push_bridge_skips_user_present() {
        let obj = json!({ "tool_response": { "disabledReason": "user_present", "message": "hi" } });
        assert!(!push_notification_should_bridge(&obj));
        let obj2 = json!({ "tool_response": { "message": "hi" } });
        assert!(push_notification_should_bridge(&obj2));
    }

    #[test]
    fn push_message_from_tool_input() {
        let obj = json!({ "tool_input": { "message": "Deploy finished" } });
        assert_eq!(push_notification_message(&obj).as_deref(), Some("Deploy finished"));
    }
}
