# rmux

rmux is a cross-platform, memory-efficient terminal multiplexer GUI written in Rust. It combines an egui desktop interface with terminal emulation, PTY-backed panes, workspaces, split layouts, browser splits, notifications, and a command socket for automation.

The project targets Linux, macOS, and Windows, with a goal of staying lightweight enough for many active panes.

## Features

- Multi-workspace terminal sessions
- Horizontal and vertical pane splits
- Keyboard-first navigation
- Terminal find, copy, scrollback, and font controls
- Browser split support
- Notification panel and desktop notification plumbing
- Rust workspace split into app, terminal, API, CLI, and config crates

## Build Instructions

Install the Rust toolchain first, then build from the workspace root:

```sh
cargo build --workspace
```

Run the app:

```sh
cargo run -p rmux-app
```

Run the full development check:

```sh
just check
```

If `just` is not installed, run the commands directly:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Build documentation:

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

## Project Layout

| Path | Purpose |
|---|---|
| `crates/rmux-app` | Main egui application |
| `crates/rmux-terminal` | Terminal emulation and PTY integration |
| `crates/rmux-cli` | CLI client |
| `crates/rmux-api` | Socket API and protocol |
| `crates/rmux-config` | Configuration schema and loading |
| `docs` | Design, architecture, and project notes |
