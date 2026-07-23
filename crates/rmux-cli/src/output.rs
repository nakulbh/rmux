//! Shared stdout formatting for CLI commands.
//!
//! Every command routes user-visible results through this module so
//! `--json` and human tables stay consistent across domains.

use anyhow::{Context, Result};
use serde_json::Value;

use crate::util::{extract_id, field_string};

/// Options that control how command results are printed.
#[derive(Debug, Clone, Copy, Default)]
pub struct OutputOpts {
    /// When true, print raw/pretty JSON instead of human tables.
    pub json: bool,
}

/// Pretty-print a JSON value to stdout.
///
/// # Errors
///
/// Returns an error if the value cannot be serialized.
pub fn print_pretty_json(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value).context("serialize JSON")?);
    Ok(())
}

/// Print a result either as pretty JSON (`opts.json`) or via a human formatter.
///
/// # Errors
///
/// Returns an error if JSON serialization fails.
pub fn print_result(
    value: &Value,
    opts: OutputOpts,
    human: impl FnOnce(&Value) -> String,
) -> Result<()> {
    if opts.json {
        print_pretty_json(value)
    } else {
        print!("{}", human(value));
        Ok(())
    }
}

/// Print an extracted id (or the full JSON when `--json` is set).
///
/// # Errors
///
/// Returns an error if JSON serialization fails.
pub fn print_id(result: &Value, opts: OutputOpts) -> Result<()> {
    if opts.json {
        print_pretty_json(result)
    } else {
        println!("{}", extract_id(result));
        Ok(())
    }
}

/// Print nothing on success unless `--json` is set (then print the result).
///
/// # Errors
///
/// Returns an error if JSON serialization fails.
pub fn print_silent_or_json(result: &Value, opts: OutputOpts) -> Result<()> {
    if opts.json { print_pretty_json(result) } else { Ok(()) }
}

/// Render the `workspace.list` result as a table with an active marker.
///
/// Accepts either a bare array or an object with a `workspaces` array.
/// Prefers `pane_count` (wire format) and falls back to a `panes` array length.
#[must_use]
pub fn format_workspace_table(result: &Value) -> String {
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
        let panes = pane_count_display(workspace);
        let marker = if workspace.get("active").and_then(Value::as_bool).unwrap_or(false) {
            '*'
        } else {
            ' '
        };
        out.push_str(&format!("{marker} {id:<16} {name:<16} {panes}\n"));
    }
    out
}

/// Render the `surface.list` result as a table.
#[must_use]
pub fn format_surface_table(result: &Value) -> String {
    let empty = Vec::new();
    let surfaces = result
        .as_array()
        .or_else(|| result.get("surfaces").and_then(Value::as_array))
        .unwrap_or(&empty);
    if surfaces.is_empty() {
        return "no surfaces\n".to_owned();
    }
    let mut out = format!("  {:<12} {:<14} {}\n", "PANE", "WORKSPACE", "ACTIVE");
    for surface in surfaces {
        let pane = field_string(surface, "pane_id");
        let workspace = field_string(surface, "workspace_id");
        let active = surface.get("active").and_then(Value::as_bool).unwrap_or(false);
        let marker = if active { '*' } else { ' ' };
        out.push_str(&format!("{marker} {pane:<12} {workspace:<14} {active}\n"));
    }
    out
}

/// Render the `notification.list` result as a table.
#[must_use]
pub fn format_notification_table(result: &Value) -> String {
    let empty = Vec::new();
    let notifications = result
        .as_array()
        .or_else(|| result.get("notifications").and_then(Value::as_array))
        .unwrap_or(&empty);
    if notifications.is_empty() {
        return "no notifications\n".to_owned();
    }
    let mut out = format!("  {:<8} {:<24} {}\n", "ID", "TITLE", "BODY");
    for n in notifications {
        let id = field_string(n, "id");
        let title = field_string(n, "title");
        let body = field_string(n, "body");
        let marker = if n.get("read").and_then(Value::as_bool) == Some(false) { '*' } else { ' ' };
        out.push_str(&format!("{marker} {id:<8} {title:<24} {body}\n"));
    }
    out
}

fn pane_count_display(workspace: &Value) -> String {
    if let Some(count) = workspace.get("pane_count") {
        return match count {
            Value::Number(n) => n.to_string(),
            other => other.to_string(),
        };
    }
    match workspace.get("panes") {
        Some(Value::Array(items)) => items.len().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(other) => other.to_string(),
        None => "-".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn workspace_table_uses_pane_count() {
        let result = json!({ "workspaces": [
            { "id": 1, "name": "main", "pane_count": 3, "active": true },
            { "id": 2, "name": "logs", "pane_count": 1, "active": false },
        ]});
        let table = format_workspace_table(&result);
        assert!(table.contains("* 1"));
        assert!(table.contains("  2"));
        assert!(table.lines().nth(1).is_some_and(|l| l.contains("3")));
        assert_eq!(format_workspace_table(&json!([])), "no workspaces\n");
    }

    #[test]
    fn surface_table_marks_active() {
        let result = json!({ "surfaces": [
            { "pane_id": 10, "workspace_id": 1, "active": true },
            { "pane_id": 11, "workspace_id": 1, "active": false },
        ]});
        let table = format_surface_table(&result);
        assert!(table.contains("* 10"));
        assert!(table.contains("  11"));
    }
}
