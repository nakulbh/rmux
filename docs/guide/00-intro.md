# rmux Guide — Introduction

> Read this guide front-to-back like a book. Each chapter builds on the last.
> No Rust experience required to start — chapter 02 covers what you need.

## What is rmux?

rmux = terminal multiplexer GUI. Like tmux, but:
- **GUI** — rendered with egui (immediate-mode graphics, not a TUI)
- **Cross-platform** — macOS, Linux, Windows from one codebase
- **Memory target** — under 100 MB with 20 active panes
- **Inspired by cmux** — cmux is macOS-only, Swift/AppKit, uses 2–5 GB RAM

rmux runs real shell processes (zsh, bash, fish) inside a GPU-rendered window. You can split the window into panes, create multiple workspaces, get desktop notifications from shell output, and control everything via keyboard shortcuts or a socket API.

## How to use this guide

| Chapter | Topic |
|---|---|
| 00 | This intro |
| 01 | Project layout — 5 Rust crates |
| 02 | Rust concepts used in rmux |
| 03 | PTY backend — spawning the shell |
| 04 | Terminal state — the grid |
| 05 | Renderer — grid → pixels |
| 06 | Terminal themes |
| 07 | OSC notifications |
| 08 | Input mapper — keys → bytes |
| 09 | Pane split tree |
| 10 | Workspace model |
| 11 | App state + frame loop |
| 12 | UI theme / palette |
| 13 | Top bar, sidebar, notification panel |
| 14 | Terminal pane widget |
| 15 | Keyboard shortcuts |
| 16 | Desktop notifications |
| 17 | Socket API + CLI |
| 18 | Config file |
| 19 | Full data-flow walkthrough |
| 20 | Where to go next |

## How to run rmux

```bash
git clone <repo>
cd rmux
cargo run -p rmux-app
```

First run compiles everything (~2 min). Subsequent runs: ~0.5 s.

## Key shortcut reference

Full list in `docs/KEY_BINDINGS.md`. Most important:

| macOS | Linux/Win | Action |
|---|---|---|
| Cmd+D | Ctrl+D | Split right |
| Cmd+Shift+D | Ctrl+Shift+D | Split down |
| Cmd+W | Ctrl+W | Close pane |
| Cmd+N | Ctrl+N | New workspace |
| Cmd+I | Ctrl+I | Toggle notifications |
| Cmd+F | Ctrl+F | Find in terminal |

→ **Next: [01 — Project Layout](01-project-layout.md)**
