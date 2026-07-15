# 01. Project layout

rmux is Rust workspace.

One repo. Many crates. Each crate owns one job.

Source root:

```text
crates/rmux-app
crates/rmux-terminal
crates/rmux-cli
crates/rmux-api
crates/rmux-config
```

Why split crates?

Small crates make beginner reading easier.

App code can change without touching terminal parser.

CLI can talk to socket without linking GUI code.

Config schema can stay boring and testable.

Workspace file says this:

```toml
[workspace]
members = ["crates/*"]
default-members = ["crates/rmux-app"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"
```

Important parts:

| Line | Meaning | Why beginner cares |
|---|---|---|
| `members = ["crates/*"]` | every crate under `crates` belongs | one `cargo test --workspace` tests all |
| `default-members = ["crates/rmux-app"]` | app is default target | `cargo run` tends toward GUI app |
| `resolver = "2"` | modern feature resolver | fewer surprise dependency features |
| `edition = "2024"` | Rust edition | syntax rules and lints match current Rust |

Shared dependencies live in root `Cargo.toml`:

```toml
[workspace.dependencies]
# GUI framework
eframe = "0.31"
egui = "0.31"

# Terminal emulation
alacritty_terminal = "0.26"
portable-pty = "0.8"
```

Why shared deps?

All crates use same version.

No crate quietly pulls older `egui`.

No duplicate terminal versions.

Tech map:

| Job | Crate |
|---|---|
| GUI window | `eframe`, `egui` |
| Terminal parsing | `alacritty_terminal` |
| Shell process | `portable-pty` |
| Async tasks | `tokio` |
| JSON protocol | `serde`, `serde_json` |
| Errors | `thiserror`, `anyhow` |

Beginner reading order:

1. `crates/rmux-terminal/src/backend.rs`
2. `crates/rmux-terminal/src/state.rs`
3. `crates/rmux-terminal/src/renderer.rs`
4. `crates/rmux-terminal/src/input.rs`
5. `crates/rmux-app/src/workspace/model.rs`
6. `crates/rmux-app/src/workspace/splits.rs`

Data path:

```text
keyboard input -> InputMapper -> PtyBackend.write()
PTY output -> PtyBackend.try_read() -> TermState.feed_bytes()
TermState snapshot -> TerminalRenderer.draw()
workspace -> PaneNode tree -> UI layout
```

Rule of thumb.

`rmux-terminal` knows terminals. `rmux-app` knows windows and workspaces.

[Prev: Intro](00-intro.md) | [Next: Rust basics](02-rust-basics.md)
