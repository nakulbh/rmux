# rmux

[![CI](https://github.com/nakulbh/rmux/actions/workflows/ci.yml/badge.svg)](https://github.com/nakulbh/rmux/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](#license)

**rmux** is a cross-platform, memory-efficient terminal multiplexer GUI written in Rust. It pairs an `egui` desktop interface with terminal emulation, PTY-backed panes, workspaces, split layouts, browser splits, notifications, and a command socket for automation.

It's a from-scratch reimplementation of [cmux](https://github.com/manaflow-ai/cmux)'s core experience — cmux is macOS-only (Swift/AppKit + libghostty) and reports 2–5 GB RAM usage with many panes open. rmux targets Linux, macOS, and Windows from one codebase, with a strict memory budget.

## Goals

- **Cross-platform** — Linux, macOS, Windows from a single codebase
- **Memory-efficient** — under 100 MB with 20 active terminal panes
- **Fast startup** — under 500 ms to first rendered frame
- **Feature parity with cmux core** — workspaces, splits, notifications, CLI/socket API
- **CLI compatibility** — `rmux-cli` accepts equivalent commands where practical

## Features

- Multi-workspace terminal sessions
- Horizontal and vertical pane splits, with focus navigation and zoom
- Keyboard-first navigation (see [Keyboard Shortcuts](#keyboard-shortcuts))
- Terminal find, copy, scrollback, and font controls
- Browser split support (`wry`-backed webview panes)
- Notification panel and desktop notification plumbing (OSC 9/99/777)
- Unix socket API + CLI client for scripting/automation
- Workspace split into focused crates: app, terminal, API, CLI, config

## Status

Actively developed. Phases 0–3 (foundation, terminal pane, workspaces/splits, notifications + CLI/socket API) are complete. Phase 4 (browser pane) is in progress. See [`docs/PLAN.md`](docs/PLAN.md) for the full phased roadmap, including upcoming SSH + session restore and agent-hook integration work.

## Getting Started

### Prerequisites

Install the [Rust toolchain](https://www.rust-lang.org/tools/install) (stable, edition 2024 support required).

### Build

```sh
cargo build --workspace
```

### Run

```sh
cargo run -p rmux-app
```

### Verify

```sh
just check
```

If `just` isn't installed, run the equivalent commands directly:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### Build docs

```sh
cargo doc --no-deps --workspace
```

## Keyboard Shortcuts

macOS uses Cmd where Linux and Windows use Ctrl.

| Action | macOS | Linux/Windows |
|---|---|---|
| Quit | Cmd+Q | Ctrl+Q |
| Find | Cmd+F | Ctrl+F |
| Find Next | Cmd+G | Ctrl+G |
| Find Next, when find is visible | Enter | Enter |
| Find Previous, when find is visible | Cmd+Option+G | Ctrl+Alt+G |
| Close Find Bar | Escape | Escape |
| Use Selection for Find | Cmd+E | Ctrl+E |
| Clear Scrollback | Cmd+K | Ctrl+K |
| Clear Screen | Cmd+Shift+K | Ctrl+Shift+K |
| Toggle Sidebar | Cmd+B | Ctrl+B |
| Toggle Notifications | Cmd+I | Ctrl+I |
| New Workspace | Cmd+N | Ctrl+N |
| Split Right | Cmd+D | Ctrl+D |
| Split Down | Cmd+Shift+D | Ctrl+Shift+D |
| Close Pane | Cmd+W | Ctrl+W |
| Close Workspace | Cmd+Shift+W | Ctrl+Shift+W |
| Rename Workspace | Cmd+Shift+R | Ctrl+Shift+R |
| Toggle Zoom | Cmd+Shift+Enter | Ctrl+Shift+Enter |
| Equalize Splits | Cmd+Shift+= | Ctrl+Shift+= |
| Previous Workspace | Cmd+Shift+[ | Ctrl+Shift+[ |
| Next Workspace | Cmd+Shift+] | Ctrl+Shift+] |
| Switch to Workspace 1 through 9 | Cmd+1 through Cmd+9 | Ctrl+1 through Ctrl+9 |
| Focus Left | Cmd+Left Arrow | Ctrl+Left Arrow |
| Focus Up | Cmd+Up Arrow | Ctrl+Up Arrow |
| Focus Right | Cmd+Option+Right Arrow | Ctrl+Alt+Right Arrow |
| Focus Down | Cmd+Option+Down Arrow | Ctrl+Alt+Down Arrow |
| Open Browser Split | Cmd+Shift+L | Ctrl+Shift+L |
| Focus Browser URL Bar | Cmd+L | Ctrl+L |
| Reload Browser | Cmd+R | Ctrl+R |
| Increase Font Size | Cmd++ or Cmd+= | Ctrl++ or Ctrl+= |
| Decrease Font Size | Cmd+- | Ctrl+- |
| Reset Font Size | Cmd+0 | Ctrl+0 |
| Copy, when text is selected | Cmd+C | Ctrl+C |

See [`docs/KEY_BINDINGS.md`](docs/KEY_BINDINGS.md) for the full reference.

## Project Layout

| Path | Purpose |
|---|---|
| `crates/rmux-app` | Main `egui` application — window, event loop, orchestrates all subsystems |
| `crates/rmux-terminal` | Terminal emulation and PTY integration (`alacritty_terminal` + `portable-pty`) |
| `crates/rmux-cli` | CLI client — connects to the socket API |
| `crates/rmux-api` | Socket server — JSON-RPC protocol, method dispatch, event streaming |
| `crates/rmux-config` | Configuration schema and loading (`rmux.json`, Ghostty config import) |
| `docs` | Design, architecture, and project notes |

## Tech Stack

| Purpose | Crate |
|---|---|
| GUI | `eframe`, `egui` |
| Terminal emulation | `alacritty_terminal` |
| PTY | `portable-pty` |
| Async runtime | `tokio` |
| Browser pane | `wry` |
| Notifications | `notify-rust` |
| Serialization | `serde`, `serde_json` |
| Errors | `thiserror`, `anyhow` |
| Logging | `tracing` |

## Documentation

| File | Contents |
|---|---|
| [`docs/PLAN.md`](docs/PLAN.md) | Full phased roadmap and task list |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Detailed architecture and data flow |
| [`docs/API_REFERENCE.md`](docs/API_REFERENCE.md) | Socket API / CLI protocol reference |
| [`docs/CONVENTIONS.md`](docs/CONVENTIONS.md) | Rust coding conventions |
| [`docs/TESTING_STRATEGY.md`](docs/TESTING_STRATEGY.md) | Testing approach and CI pipeline |
| [`docs/KEY_BINDINGS.md`](docs/KEY_BINDINGS.md) | Full keyboard shortcut reference |
| [`AGENTS.md`](AGENTS.md) | Contribution/agent instructions, code style, and dependency policy |

## Contributing

See [`AGENTS.md`](AGENTS.md) for coding conventions, the verification checklist, and dependency policy. In short: run `just check` before committing, follow conventional commit messages, and keep one logical change per commit.

## License

MIT
