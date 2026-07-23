//! One function per CLI subcommand: each builds the JSON-RPC method and
//! params, performs the socket roundtrip via [`crate::socket::call`], and
//! formats the result for stdout. Request building lives in separate
//! helpers so it can be unit tested without a socket.

use std::path::Path;

use anyhow::Result;
use serde_json::{Value, json};

use crate::socket;

/// `rmux-cli ping` — call `system.ping` and print `pong`.
///
/// # Errors
///
/// Returns an error if the socket call fails.
pub fn ping(socket_path: &Path) -> Result<()> {
    let (method, params) = ping_request();
    socket::call(socket_path, method, params)?;
    println!("pong");
    Ok(())
}

/// `rmux-cli capabilities` — call `system.capabilities` and pretty-print
/// the result JSON.
///
/// # Errors
///
/// Returns an error if the socket call fails or the result cannot be
/// re-serialized.
pub fn capabilities(socket_path: &Path) -> Result<()> {
    let (method, params) = capabilities_request();
    let result = socket::call(socket_path, method, params)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

/// `rmux-cli notify` — call `notification.create` and print the created
/// notification id.
///
/// # Errors
///
/// Returns an error if the socket call fails.
pub fn notify(
    socket_path: &Path,
    title: &str,
    subtitle: Option<&str>,
    body: Option<&str>,
) -> Result<()> {
    let (method, params) = notify_request(title, subtitle, body);
    let result = socket::call(socket_path, method, params)?;
    println!("{}", extract_id(&result));
    Ok(())
}

/// `rmux-cli new-workspace` — call `workspace.create` and print the
/// created workspace id.
///
/// # Errors
///
/// Returns an error if the socket call fails.
pub fn new_workspace(socket_path: &Path, name: Option<&str>) -> Result<()> {
    let (method, params) = new_workspace_request(name);
    let result = socket::call(socket_path, method, params)?;
    println!("{}", extract_id(&result));
    Ok(())
}

/// `rmux-cli list-workspaces` — call `workspace.list` and print either a
/// human-readable table or the raw JSON result.
///
/// # Errors
///
/// Returns an error if the socket call fails.
pub fn list_workspaces(socket_path: &Path, json_output: bool) -> Result<()> {
    let (method, params) = list_workspaces_request();
    let result = socket::call(socket_path, method, params)?;
    if json_output {
        println!("{result}");
    } else {
        print!("{}", format_workspace_table(&result));
    }
    Ok(())
}

/// `rmux-cli new-split` — call `surface.split` and print the new pane id.
///
/// # Errors
///
/// Returns an error if the socket call fails.
pub fn new_split(socket_path: &Path, direction: &str) -> Result<()> {
    let (method, params) = new_split_request(direction);
    let result = socket::call(socket_path, method, params)?;
    println!("{}", extract_id(&result));
    Ok(())
}

/// `rmux-cli send` — interpret backslash escapes in `text`, then call
/// `surface.send_text`. Silent on success.
///
/// # Errors
///
/// Returns an error if the socket call fails.
pub fn send(socket_path: &Path, text: &str) -> Result<()> {
    let (method, params) = send_request(text);
    socket::call(socket_path, method, params)?;
    Ok(())
}

fn ping_request() -> (&'static str, Value) {
    ("system.ping", json!({}))
}

fn capabilities_request() -> (&'static str, Value) {
    ("system.capabilities", json!({}))
}

fn notify_request(
    title: &str,
    subtitle: Option<&str>,
    body: Option<&str>,
) -> (&'static str, Value) {
    ("notification.create", json!({ "title": title, "subtitle": subtitle, "body": body }))
}

fn new_workspace_request(name: Option<&str>) -> (&'static str, Value) {
    ("workspace.create", json!({ "name": name }))
}

fn list_workspaces_request() -> (&'static str, Value) {
    ("workspace.list", json!({}))
}

fn new_split_request(direction: &str) -> (&'static str, Value) {
    ("surface.split", json!({ "direction": direction }))
}

fn send_request(text: &str) -> (&'static str, Value) {
    ("surface.send_text", json!({ "text": interpret_escapes(text) }))
}

/// `rmux-cli browser-open` — open a browser pane (optional URL).
pub fn browser_open(socket_path: &Path, url: Option<&str>) -> Result<()> {
    let (method, params) = browser_open_request(url);
    let result = socket::call(socket_path, method, params)?;
    println!("{}", extract_id(&result));
    Ok(())
}

/// `rmux-cli browser-nav` — navigate the active (or given) browser pane.
pub fn browser_nav(socket_path: &Path, url: &str, pane_id: Option<u64>) -> Result<()> {
    let (method, params) = browser_nav_request(url, pane_id);
    let result = socket::call(socket_path, method, params)?;
    if let Some(u) = result.get("url").and_then(Value::as_str) {
        println!("{u}");
    } else {
        println!("{result}");
    }
    Ok(())
}

/// `rmux-cli browser-url` — print current URL / title JSON.
pub fn browser_url(socket_path: &Path, pane_id: Option<u64>) -> Result<()> {
    let (method, params) = browser_url_request(pane_id);
    let result = socket::call(socket_path, method, params)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

/// `rmux-cli browser-eval` — evaluate a JS expression in the page.
pub fn browser_eval(socket_path: &Path, script: &str, pane_id: Option<u64>) -> Result<()> {
    let (method, params) = browser_eval_request(script, pane_id);
    let result = socket::call(socket_path, method, params)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

/// `rmux-cli browser-click` — click a CSS selector.
pub fn browser_click(socket_path: &Path, selector: &str, pane_id: Option<u64>) -> Result<()> {
    let (method, params) = browser_click_request(selector, pane_id);
    socket::call(socket_path, method, params)?;
    Ok(())
}

/// `rmux-cli browser-fill` — fill an input matching a CSS selector.
pub fn browser_fill(
    socket_path: &Path,
    selector: &str,
    value: &str,
    pane_id: Option<u64>,
) -> Result<()> {
    let (method, params) = browser_fill_request(selector, value, pane_id);
    socket::call(socket_path, method, params)?;
    Ok(())
}

/// `rmux-cli browser-snapshot` — dump a DOM/a11y snapshot as JSON.
pub fn browser_snapshot(socket_path: &Path, pane_id: Option<u64>) -> Result<()> {
    let (method, params) = browser_snapshot_request(pane_id);
    let result = socket::call(socket_path, method, params)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn browser_open_request(url: Option<&str>) -> (&'static str, Value) {
    ("browser.open", json!({ "url": url }))
}

fn browser_nav_request(url: &str, pane_id: Option<u64>) -> (&'static str, Value) {
    ("browser.navigate", json!({ "url": url, "pane_id": pane_id }))
}

fn browser_url_request(pane_id: Option<u64>) -> (&'static str, Value) {
    ("browser.url", json!({ "pane_id": pane_id }))
}

fn browser_eval_request(script: &str, pane_id: Option<u64>) -> (&'static str, Value) {
    ("browser.eval", json!({ "script": script, "pane_id": pane_id }))
}

fn browser_click_request(selector: &str, pane_id: Option<u64>) -> (&'static str, Value) {
    ("browser.click", json!({ "selector": selector, "pane_id": pane_id }))
}

fn browser_fill_request(
    selector: &str,
    value: &str,
    pane_id: Option<u64>,
) -> (&'static str, Value) {
    ("browser.fill", json!({ "selector": selector, "value": value, "pane_id": pane_id }))
}

fn browser_snapshot_request(pane_id: Option<u64>) -> (&'static str, Value) {
    ("browser.snapshot", json!({ "pane_id": pane_id }))
}

/// Interpret literal backslash escapes in CLI text arguments.
///
/// Supported: `\n` (newline), `\r` (carriage return), `\t` (tab),
/// `\e` (escape, 0x1B) and `\\` (backslash). Unknown escapes and a
/// trailing lone backslash pass through unchanged.
fn interpret_escapes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('e') => out.push('\u{1b}'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Pull a human-friendly id out of a result value.
///
/// Servers may return the id as a bare string or under one of several
/// conventional keys; fall back to the raw JSON if none match.
fn extract_id(result: &Value) -> String {
    if let Value::String(s) = result {
        return s.clone();
    }
    for key in ["id", "workspace_id", "pane_id", "notification_id"] {
        match result.get(key) {
            Some(Value::String(s)) => return s.clone(),
            Some(v) if !v.is_null() => return v.to_string(),
            _ => {}
        }
    }
    result.to_string()
}

/// Render the `workspace.list` result as a table with an active marker.
///
/// Accepts either a bare array or an object with a `workspaces` array.
fn format_workspace_table(result: &Value) -> String {
    let empty = Vec::new();
    let workspaces = result
        .as_array()
        .or_else(|| result.get("workspaces").and_then(Value::as_array))
        .unwrap_or(&empty);
    if workspaces.is_empty() {
        return "no workspaces\n".to_owned();
    }
    let mut out = format!("  {:<16} {:<16} {}\n", "ID", "NAME", "PANES");
    for workspace in workspaces {
        let id = field_string(workspace, "id");
        let name = field_string(workspace, "name");
        let panes = match workspace.get("panes") {
            Some(Value::Array(items)) => items.len().to_string(),
            Some(other) => other.to_string(),
            None => "-".to_owned(),
        };
        let marker = if workspace.get("active").and_then(Value::as_bool).unwrap_or(false) {
            '*'
        } else {
            ' '
        };
        out.push_str(&format!("{marker} {id:<16} {name:<16} {panes}\n"));
    }
    out
}

/// Read a field as a display string, falling back to `-` when absent.
fn field_string(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => "-".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameterless_requests_use_expected_methods() {
        assert_eq!(ping_request(), ("system.ping", json!({})));
        assert_eq!(capabilities_request(), ("system.capabilities", json!({})));
        assert_eq!(list_workspaces_request(), ("workspace.list", json!({})));
    }

    #[test]
    fn notify_request_includes_all_fields() {
        let (method, params) = notify_request("Build", Some("rmux"), Some("done"));
        assert_eq!(method, "notification.create");
        assert_eq!(params, json!({ "title": "Build", "subtitle": "rmux", "body": "done" }));
    }

    #[test]
    fn notify_request_nulls_missing_fields() {
        let (_, params) = notify_request("Build", None, None);
        assert_eq!(params, json!({ "title": "Build", "subtitle": null, "body": null }));
    }

    #[test]
    fn new_workspace_request_passes_optional_name() {
        assert_eq!(new_workspace_request(Some("dev")).1, json!({ "name": "dev" }));
        assert_eq!(new_workspace_request(None), ("workspace.create", json!({ "name": null })));
    }

    #[test]
    fn browser_open_request_shape() {
        let (method, params) = browser_open_request(Some("https://example.com"));
        assert_eq!(method, "browser.open");
        assert_eq!(params, json!({ "url": "https://example.com" }));
    }

    #[test]
    fn browser_eval_request_shape() {
        let (method, params) = browser_eval_request("1+1", Some(3));
        assert_eq!(method, "browser.eval");
        assert_eq!(params, json!({ "script": "1+1", "pane_id": 3 }));
    }

    #[test]
    fn browser_click_request_shape() {
        let (method, params) = browser_click_request("#submit", None);
        assert_eq!(method, "browser.click");
        assert_eq!(params, json!({ "selector": "#submit", "pane_id": null }));
    }

    #[test]
    fn new_split_request_passes_direction() {
        assert_eq!(new_split_request("right"), ("surface.split", json!({ "direction": "right" })));
        assert_eq!(new_split_request("down").1, json!({ "direction": "down" }));
    }

    #[test]
    fn send_request_interprets_escapes() {
        assert_eq!(send_request("ls\\n"), ("surface.send_text", json!({ "text": "ls\n" })));
    }

    #[test]
    fn escapes_cover_known_and_unknown_sequences() {
        assert_eq!(interpret_escapes("a\\nb\\rc\\td\\ee\\\\f"), "a\nb\rc\td\u{1b}e\\f");
        // Unknown escapes and a trailing lone backslash pass through unchanged.
        assert_eq!(interpret_escapes("a\\qb"), "a\\qb");
        assert_eq!(interpret_escapes("abc\\"), "abc\\");
    }

    #[test]
    fn extract_id_handles_common_shapes() {
        assert_eq!(extract_id(&json!("ws-1")), "ws-1");
        assert_eq!(extract_id(&json!({ "id": "pane-2" })), "pane-2");
        assert_eq!(extract_id(&json!({ "workspace_id": 7 })), "7");
        assert_eq!(extract_id(&json!({ "other": true })), r#"{"other":true}"#);
    }

    #[test]
    fn workspace_table_marks_active_and_counts_panes() {
        let result = json!({ "workspaces": [
            { "id": "ws-1", "name": "main", "panes": 3, "active": true },
            { "id": "ws-2", "name": "logs", "panes": ["p1", "p2"], "active": false },
        ]});
        let table = format_workspace_table(&result);
        assert!(table.contains("* ws-1"));
        assert!(table.contains("  ws-2"));
        assert!(table.lines().nth(1).is_some_and(|l| l.ends_with('3')));
        assert!(table.lines().nth(2).is_some_and(|l| l.ends_with('2')));
        assert_eq!(format_workspace_table(&json!([])), "no workspaces\n");
    }
}
