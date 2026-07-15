# 18. Config

Config crate defines `rmux.json` shape. Schema first. Loading later.

Files:

- `crates/rmux-config/src/lib.rs`
- `crates/rmux-config/src/schema.rs`

## Crate job

Top comment:

```rust
#![forbid(unsafe_code)]
//! Configuration management for rmux.
//!
//! Loads and saves the rmux configuration from platform-appropriate
//! directories. Defines the config schema and provides import from
//! Ghostty config files.
```

Current module export:

```rust
pub mod schema;
```

So users import types from `rmux_config::schema`.

## Config file path

Schema docs say where config lives:

```rust
/// Top-level rmux configuration.
///
/// Loaded from the platform-specific config directory:
/// - Linux: `~/.config/rmux/rmux.json`
/// - macOS: `~/Library/Application Support/rmux/rmux.json`
/// - Windows: `%APPDATA%\rmux\rmux.json`
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// Terminal emulation settings.
    #[serde(default)]
    pub terminal: TerminalConfig,
}
```

Beginner note: `#[serde(default)]` means missing `terminal` can still deserialize.

## TerminalConfig

Terminal settings live in nested struct.

```rust
/// Terminal emulation configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TerminalConfig {
    /// Shell to spawn (defaults to `$SHELL` or `/bin/sh`).
    #[serde(default)]
    pub shell: Option<String>,

    /// Font family for terminal text (must be monospace).
    #[serde(default = "default_font_family")]
    pub font_family: String,

    /// Font size in points.
    #[serde(default = "default_font_size")]
    pub font_size: f32,

    /// Maximum scrollback lines per pane.
    #[serde(default = "default_max_scrollback")]
    pub max_scrollback_lines: usize,
}
```

`Option<String>` for shell means user may omit it. App can fallback to `$SHELL`.

## Defaults

Manual `Default` impl:

```rust
impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell: None,
            font_family: default_font_family(),
            font_size: default_font_size(),
            max_scrollback_lines: default_max_scrollback(),
        }
    }
}
```

Helper functions used by serde:

```rust
fn default_font_family() -> String {
    "monospace".to_owned()
}

fn default_font_size() -> f32 {
    14.0
}

fn default_max_scrollback() -> usize {
    10_000
}
```

Why helper functions? Serde attributes need function paths.

## Example config

This matches current schema:

```json
{
  "terminal": {
    "shell": "/bin/zsh",
    "font_family": "JetBrains Mono",
    "font_size": 14.0,
    "max_scrollback_lines": 10000
  }
}
```

Minimal config also valid:

```json
{
  "terminal": {}
}
```

Why? Field defaults fill missing values.

## Tests explain behavior

Defaults test:

```rust
#[test]
fn test_terminal_config_defaults() {
    let config = TerminalConfig::default();
    assert_eq!(config.font_family, "monospace");
    assert_eq!(config.font_size, 14.0);
    assert_eq!(config.max_scrollback_lines, 10_000);
    assert!(config.shell.is_none());
}
```

Deserialize empty terminal:

```rust
#[test]
fn test_config_deserialize_empty() {
    let json = r#"{"terminal":{}}"#;
    let config: Config = serde_json::from_str(json).expect("should deserialize");
    assert_eq!(config.terminal.font_family, "monospace");
}
```

## Where to add config next

Want new setting? Edit `schema.rs`.

Example path:

```text
Config
  terminal: TerminalConfig
    shell
    font_family
    font_size
    max_scrollback_lines
```

Need browser settings? Add `BrowserConfig` and field on `Config`.

Need notifications setting? Add `NotificationConfig` and field on `Config`.

Remember: add defaults, add serde attrs, add tests.

← **Prev: [17 — API Server](17-api-server.md)**

→ **Next: [19 — Data Flow](19-data-flow.md)**
