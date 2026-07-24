//! Shared helpers used by multiple CLI command modules.

use serde_json::Value;

/// Interpret literal backslash escapes in CLI text arguments.
///
/// Supported: `\n` (newline), `\r` (carriage return), `\t` (tab),
/// `\e` (escape, 0x1B) and `\\` (backslash). Unknown escapes and a
/// trailing lone backslash pass through unchanged.
#[must_use]
pub fn interpret_escapes(input: &str) -> String {
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
#[must_use]
pub fn extract_id(result: &Value) -> String {
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

/// Read a field as a display string, falling back to `-` when absent.
#[must_use]
pub fn field_string(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => "-".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn escapes_cover_known_and_unknown_sequences() {
        assert_eq!(interpret_escapes("a\\nb\\rc\\td\\ee\\\\f"), "a\nb\rc\td\u{1b}e\\f");
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
}
