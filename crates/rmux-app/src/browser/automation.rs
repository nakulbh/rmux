//! Browser automation helpers — JS snippets for click / type / fill / press /
//! snapshot, inspired by cmux / agent-browser.
//!
//! All helpers return a JSON string from the page so the Rust side can parse
//! `{ ok, … }` uniformly.

/// Escape `s` as a JS string literal (double-quoted).
pub fn js_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

/// Wrap a user expression so the page always returns a JSON string result.
///
/// `script` is treated as a JS **expression** (e.g. `document.title`, `1+1`).
pub fn wrap_eval_expression(script: &str) -> String {
    format!(
        r#"(function(){{
  try {{
    var __v = ({script});
    return JSON.stringify({{ok:true,value:(__v===undefined)?null:__v}});
  }} catch(e) {{
    return JSON.stringify({{ok:false,error:String(e&&e.message||e)}});
  }}
}})()"#
    )
}

/// Click the first element matching `selector`.
pub fn script_click(selector: &str) -> String {
    let sel = js_string(selector);
    format!(
        r#"(function(){{
  var el = document.querySelector({sel});
  if (!el) return JSON.stringify({{ok:false,error:"element not found: "+{sel}}});
  el.click();
  return JSON.stringify({{ok:true}});
}})()"#
    )
}

/// Set the value of an input/textarea/contenteditable and fire input/change.
pub fn script_fill(selector: &str, value: &str) -> String {
    let sel = js_string(selector);
    let val = js_string(value);
    format!(
        r#"(function(){{
  var el = document.querySelector({sel});
  if (!el) return JSON.stringify({{ok:false,error:"element not found: "+{sel}}});
  el.focus();
  if (el.isContentEditable) {{
    el.textContent = {val};
  }} else {{
    var proto = el.tagName === 'SELECT' ? HTMLSelectElement.prototype
              : el.tagName === 'TEXTAREA' ? HTMLTextAreaElement.prototype
              : HTMLInputElement.prototype;
    var desc = Object.getOwnPropertyDescriptor(proto, 'value');
    if (desc && desc.set) desc.set.call(el, {val});
    else el.value = {val};
  }}
  el.dispatchEvent(new Event('input', {{bubbles:true}}));
  el.dispatchEvent(new Event('change', {{bubbles:true}}));
  return JSON.stringify({{ok:true}});
}})()"#
    )
}

/// Append text into the focused element (or `selector` if given) via input events.
pub fn script_type(text: &str, selector: Option<&str>) -> String {
    let val = js_string(text);
    let resolve = match selector {
        Some(sel) => {
            let s = js_string(sel);
            format!(
                r#"var el = document.querySelector({s});
  if (!el) return JSON.stringify({{ok:false,error:"element not found: "+{s}}});
  el.focus();"#
            )
        }
        None => r#"var el = document.activeElement;
  if (!el || el === document.body) return JSON.stringify({ok:false,error:"no focused element"});"#
            .to_string(),
    };
    format!(
        r#"(function(){{
  {resolve}
  var text = {val};
  for (var i = 0; i < text.length; i++) {{
    var ch = text[i];
    el.dispatchEvent(new KeyboardEvent('keydown', {{key:ch,bubbles:true}}));
    if (el.isContentEditable) {{
      el.textContent = (el.textContent || '') + ch;
    }} else if ('value' in el) {{
      el.value = (el.value || '') + ch;
    }}
    el.dispatchEvent(new Event('input', {{bubbles:true}}));
    el.dispatchEvent(new KeyboardEvent('keyup', {{key:ch,bubbles:true}}));
  }}
  return JSON.stringify({{ok:true}});
}})()"#
    )
}

/// Dispatch a key press on the active element (or `selector`).
///
/// `key` is a KeyboardEvent `key` value (`Enter`, `Tab`, `Escape`, `a`, …).
pub fn script_press(key: &str, selector: Option<&str>) -> String {
    let key_js = js_string(key);
    let resolve = match selector {
        Some(sel) => {
            let s = js_string(sel);
            format!(
                r#"var el = document.querySelector({s});
  if (!el) return JSON.stringify({{ok:false,error:"element not found: "+{s}}});
  el.focus();"#
            )
        }
        None => r#"var el = document.activeElement || document.body;"#.to_string(),
    };
    format!(
        r#"(function(){{
  {resolve}
  var key = {key_js};
  var opts = {{key:key,code:key,bubbles:true,cancelable:true}};
  el.dispatchEvent(new KeyboardEvent('keydown', opts));
  el.dispatchEvent(new KeyboardEvent('keypress', opts));
  if (key === 'Enter' && el.form) {{
    /* let form handle submit if applicable */
  }}
  el.dispatchEvent(new KeyboardEvent('keyup', opts));
  return JSON.stringify({{ok:true}});
}})()"#
    )
}

/// Build a shallow accessibility / DOM snapshot as JSON.
pub fn script_snapshot(max_depth: u32, max_children: u32) -> String {
    format!(
        r#"(function(){{
  var MAX_DEPTH = {max_depth};
  var MAX_CHILDREN = {max_children};
  function walk(el, depth) {{
    if (!el || depth > MAX_DEPTH) return null;
    if (el.nodeType !== 1) return null;
    var tag = (el.tagName || '').toLowerCase();
    if (tag === 'script' || tag === 'style' || tag === 'noscript') return null;
    var role = el.getAttribute('role') || tag;
    var name = el.getAttribute('aria-label')
      || el.getAttribute('alt')
      || el.getAttribute('placeholder')
      || el.getAttribute('title')
      || '';
    if (!name && (tag === 'a' || tag === 'button' || tag === 'label' || tag === 'h1' || tag === 'h2' || tag === 'h3')) {{
      name = (el.innerText || el.textContent || '').trim().slice(0, 80);
    }}
    var node = {{ role: role, name: name }};
    if (el.id) node.ref = '#' + el.id;
    else if (el.getAttribute('data-testid')) node.ref = '[data-testid=' + JSON.stringify(el.getAttribute('data-testid')) + ']';
    var kids = [];
    var children = el.children || [];
    for (var i = 0; i < children.length && kids.length < MAX_CHILDREN; i++) {{
      var c = walk(children[i], depth + 1);
      if (c) kids.push(c);
    }}
    if (kids.length) node.children = kids;
    return node;
  }}
  try {{
    var tree = walk(document.body, 0);
    return JSON.stringify({{ok:true,url:location.href,title:document.title,tree:tree}});
  }} catch(e) {{
    return JSON.stringify({{ok:false,error:String(e&&e.message||e)}});
  }}
}})()"#
    )
}

/// Parse a page-returned JSON string into a serde Value, mapping failures.
pub fn parse_page_json(raw: &str) -> Result<serde_json::Value, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "null" || trimmed == "undefined" {
        return Err("empty evaluation result".into());
    }
    // wry may return either a JSON value or a JSON string containing JSON
    // (double-encoded). Unwrap one string layer when present.
    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(serde_json::Value::String(s)) => serde_json::from_str(&s)
            .map_err(|e| format!("invalid JSON result (after unquote): {e}")),
        Ok(v) => Ok(v),
        Err(e) => Err(format!("invalid JSON result: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_string_escapes_quotes() {
        assert_eq!(js_string(r#"a"b"#), r#""a\"b""#);
    }

    #[test]
    fn wrap_eval_contains_script() {
        let s = wrap_eval_expression("1+1");
        assert!(s.contains("1+1"));
        assert!(s.contains("JSON.stringify"));
    }

    #[test]
    fn click_script_embeds_selector() {
        let s = script_click("#go");
        assert!(s.contains("#go"));
        assert!(s.contains("querySelector"));
    }

    #[test]
    fn fill_script_embeds_value() {
        let s = script_fill("input[name=q]", "hello");
        assert!(s.contains("hello"));
        assert!(s.contains("input[name=q]"));
    }

    #[test]
    fn parse_page_json_object() {
        let v = parse_page_json(r#"{"ok":true,"value":2}"#).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["value"], 2);
    }

    #[test]
    fn parse_page_json_double_encoded() {
        let v = parse_page_json(r#""{\"ok\":true}""#).unwrap();
        assert_eq!(v["ok"], true);
    }
}
