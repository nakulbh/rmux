# rmux

<img src="rmux_logo.jpg" alt="rmux logo" width="120" align="right">

> Cross-platform terminal multiplexer GUI written in Rust.

rmux is a memory-efficient terminal multiplexer with a native desktop interface. It supports workspaces, pane splits, browser panes, notifications, and a socket API for automation. Targets Linux, macOS, and Windows.

Inspired by [cmux](https://github.com/manaflow-ai/cmux).

## Quick Start

```sh
# Build
cargo build --workspace

# Run
cargo run -p rmux-app --bin rmux

# Tests + lint + fmt
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Prerequisites

[Rust toolchain](https://www.rust-lang.org/tools/install) (stable).

## Keyboard Shortcuts

macOS uses Cmd where Linux and Windows use Ctrl.

| Action | macOS | Linux/Windows |
|---|---|---|
| New Workspace | Cmd+N | Ctrl+N |
| Split Right | Cmd+D | Ctrl+D |
| Split Down | Cmd+Shift+D | Ctrl+Shift+D |
| Close Pane | Cmd+W | Ctrl+W |
| Toggle Sidebar | Cmd+B | Ctrl+B |
| Find | Cmd+F | Ctrl+F |
| Increase Font Size | Cmd+= | Ctrl+= |
| Decrease Font Size | Cmd+- | Ctrl+- |

See [`docs/KEY_BINDINGS.md`](docs/KEY_BINDINGS.md) for the full reference.

## Project Layout

| Path | Purpose |
|---|---|
| `crates/rmux-app` | Main egui application |
| `crates/rmux-terminal` | Terminal emulation (alacritty_terminal + portable-pty) |
| `crates/rmux-cli` | CLI client |
| `crates/rmux-api` | Socket server (JSON-RPC) |
| `crates/rmux-config` | Configuration schema |
| `docs/` | Design docs and roadmap |

See [`docs/PLAN.md`](docs/PLAN.md) for the phased roadmap and [`AGENTS.md`](AGENTS.md) for contribution guidelines.

## License

MIT
