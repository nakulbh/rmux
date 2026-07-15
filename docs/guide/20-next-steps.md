# 20. Next steps

Guide loop complete. You now know crates, terminal path, UI path, API path.

Use small changes first. One file. One test. One behavior.

## Where features land

App state and frame loop:

```text
crates/rmux-app/src/app.rs
```

Terminal widget behavior:

```text
crates/rmux-app/src/ui/terminal_pane.rs
```

Shortcut definitions:

```text
crates/rmux-app/src/shortcuts.rs
crates/rmux-app/src/shortcut_handler.rs
```

Workspace and splits:

```text
crates/rmux-app/src/workspace/model.rs
crates/rmux-app/src/workspace/splits.rs
crates/rmux-app/src/ui/workspace_view.rs
```

Socket API and CLI:

```text
crates/rmux-api/src/
crates/rmux-cli/src/
crates/rmux-app/src/api_dispatch.rs
```

Config, theme, chrome:

```text
crates/rmux-config/src/schema.rs
crates/rmux-app/src/ui/theme.rs
crates/rmux-app/src/ui/top_bar.rs
crates/rmux-app/src/ui/sidebar.rs
crates/rmux-app/src/ui/notification_panel.rs
```

## Starter task 1: add config field

Goal: add terminal cursor blink setting.

Edit: `crates/rmux-config/src/schema.rs`.

Steps:

1. Add `pub cursor_blink: bool` to `TerminalConfig`.
2. Add `default_cursor_blink() -> bool`.
3. Add field to `Default` impl.
4. Add test for default.

## Starter task 2: add shortcut

Goal: add shortcut to toggle dimension overlay.

Edit:

```text
crates/rmux-app/src/shortcuts.rs
crates/rmux-app/src/shortcut_handler.rs
crates/rmux-app/src/ui/terminal_pane.rs
```

Steps:

1. Add `ShortcutAction::ToggleDimensions`.
2. Register chord in `ShortcutRegistry::default()`.
3. Add `TerminalPane` public method.
4. Dispatch to active terminal.

## Starter task 3: add CLI command

Goal: `rmux-cli clear-notifications`.

Edit:

```text
crates/rmux-cli/src/commands.rs
crates/rmux-cli/src/main.rs
crates/rmux-app/src/api_dispatch.rs
```

Steps:

1. Add request builder returning `notification.clear`.
2. Add CLI subcommand.
3. Add dispatch method to clear manager.
4. Add tests for request builder.

## Starter task 4: add theme token usage

Goal: change active sidebar card accent or badge color.

Edit:

```text
crates/rmux-app/src/ui/theme.rs
crates/rmux-app/src/ui/sidebar.rs
```

Steps:

1. Add or reuse color token.
2. Replace hardcoded color if any.
3. Run app and inspect sidebar.

## Starter task 5: improve notification row

Goal: show pane id in notification card metadata.

Edit:

```text
crates/rmux-app/src/ui/notification_panel.rs
crates/rmux-app/src/notifications/mod.rs
```

Steps:

1. Read `Notification` fields.
2. Update row text in `render_row()`.
3. Keep read/unread styling unchanged.

## Starter task 6: add API capability

Goal: expose current theme in `system.capabilities` or new method.

Edit:

```text
crates/rmux-app/src/api_dispatch.rs
crates/rmux-api/src/protocol.rs
crates/rmux-cli/src/commands.rs
```

Steps:

1. Find existing `system.capabilities` handler.
2. Add theme field from app state.
3. Add CLI print path if needed.
4. Add dispatch test.

## Starter task 7: add find bar polish

Goal: show match count as `2/9`.

Edit: `crates/rmux-app/src/ui/terminal_pane.rs`.

Steps:

1. Find find bar rendering function.
2. Use `find_results.len()` and `find_index`.
3. Render small muted label.

## Safe workflow

Use this loop:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Docs-only check:

```bash
wc -l docs/guide/*.md
```

Need refresh? Start over.

→ **Next: [00 — Introduction](00-intro.md)**

← **Prev: [19 — Data Flow](19-data-flow.md)**
