# rmux — Project Plan

> Cross-platform, memory-efficient terminal multiplexer GUI inspired by cmux, written in Rust.

## Table of Contents

- [Vision](#vision)
- [Research Summary](#research-summary)
- [Phase 0: Foundation](#phase-0-foundation)
- [Phase 1: Single Terminal Pane](#phase-1-single-terminal-pane)
- [Phase 2: Workspaces + Splits + Sidebar](#phase-2-workspaces--splits--sidebar)
- [Phase 3: Notifications + CLI/Socket API](#phase-3-notifications--clisocket-api)
- [Phase 4: Browser Pane](#phase-4-browser-pane)
- [Phase 5: SSH + Session Restore](#phase-5-ssh--session-restore)
- [Phase 6: Agent Hooks + Integration](#phase-6-agent-hooks--integration)
- [Memory Budget](#memory-budget)
- [Rust Best Practices](#rust-best-practices)

---

## Vision

cmux is a macOS-only terminal built on Swift/AppKit + libghostty. Users report high RAM usage
(2–5 GB with many panes) and it only runs on macOS. rmux rewrites the core as a
cross-platform Rust application targeting Linux, macOS, and Windows with a strict memory budget.

### Goals

- **Cross-platform**: Linux, macOS, Windows from a single codebase
- **Memory-efficient**: < 100 MB with 20 active terminal panes
- **Fast startup**: < 500 ms to first rendered frame
- **Feature parity with cmux core**: workspaces, splits, notifications, CLI/socket API
- **cmux CLI compatibility**: `rmux` CLI accepts the same commands as `cmux` where practical

### Non-Goals (for MVP)

- iOS companion app
- AppleScript support
- Canvas/freeform layout (stick to split-tree for now)
- tmux compatibility shims

---

## Research Summary

### What cmux is

| Component | cmux Implementation | rmux Equivalent |
|---|---|---|
| Terminal rendering | libghostty (Zig/C, Metal GPU) | `alacritty_terminal` + custom egui renderer |
| GUI shell | Swift AppKit/SwiftUI | `egui` + `eframe` |
| Browser pane | WKWebView | `wry` (OS webview) |
| PTY / process | libghostty tty module | `portable-pty` |
| SSH remote | Go daemon (`cmuxd-remote`) | Rust async SSH client |
| Notifications | OSC 9/99/777 + macOS UNUserNotificationCenter | `notify-rust` + OSC parser |
| CLI/socket | Swift CLI + Unix socket | Rust CLI binary + Unix socket |
| Session restore | `~/Library/Application Support/cmux/` JSON | `~/.config/rmux/session.json` |
| Agent hooks | Swift hook definitions per agent | Rust agent hook registry |
| Config | `~/.config/cmux/cmux.json` + Ghostty config | `~/.config/rmux/rmux.json` |

### Memory hotspots in cmux (to avoid in rmux)

| Problem | cmux Cause | rmux Solution |
|---|---|---|
| Per-surface GPU backing | Each Ghostty surface has its own IOSurface | Shared renderer, virtual surfaces |
| Unbounded SwiftUI view graph | O(panes) per-frame flush | Immediate-mode egui: no retained view graph |
| WKWebView process per pane | Separate WebKit renderer process | Single `wry` webview, lazy spawn |
| Thread explosion | 4 threads per Ghostty surface | Single async runtime, shared thread pool |
| Scrollback accumulation | No limits enforced | Configurable max scrollback per pane |

---

## Phase 0: Foundation

**Goal**: Skeleton project, window rendering, dependency setup.

### Tasks

- [x] **0.1** Replace GPUI with `eframe` + `egui` in `Cargo.toml`
- [x] **0.2** Add dependencies: `alacritty_terminal`, `portable-pty`, `tokio`, `serde`, `serde_json`
- [x] **0.3** Create module structure (see [ARCHITECTURE.md](./ARCHITECTURE.md))
- [x] **0.4** Build a single `egui` window with a placeholder grid
- [x] **0.5** Add `tracing` + `tracing-subscriber` for structured logging
- [x] **0.6** Add `clap` for CLI argument parsing
- [x] **0.7** Set up `cargo fmt`, `cargo clippy`, `cargo test` in CI
- [x] **0.8** Add `justfile` or `makefile` with common tasks

### Deliverables

- Window opens with a gray grid placeholder
- `rmux --version` works from CLI
- `cargo clippy` passes with zero warnings
- `cargo test` runs (even if empty)

### Milestone

**A window that renders something.**

---

## Phase 1: Single Terminal Pane

**Goal**: A working terminal emulator in a single pane.

### Tasks

- [x] **1.1** Implement `PtyBackend` using `portable-pty`:
  - Spawn shell (respect `$SHELL`, fallback to `/bin/sh`)
  - Read/write PTY
  - Handle resize events
- [x] **1.2** Implement `TermState` wrapping `alacritty_terminal::Term`:
  - Feed PTY bytes through VTE parser
  - Maintain grid, scrollback, selection
  - Configurable `max_scrollback_lines` (default: 10_000)
- [x] **1.3** Implement `TerminalRenderer`:
  - Convert `Term` grid cells → egui `PaintCmd` rects + glyphs
  - Render cursor, selection highlight, colors (16 + 256 + truecolor)
  - Font: monospace, configurable size
- [x] **1.4** Handle keyboard input:
  - Map egui `KeyEvent` → terminal escape sequences
  - Support Ctrl, Alt, Shift modifiers
  - Paste via bracket mode
- [x] **1.5** Handle mouse input:
  - Click to position cursor
  - Scroll wheel → terminal scrollback
  - Selection (click-drag)
- [x] **1.6** Handle terminal resize:
  - Window resize → recalculate cols/rows → PTY resize
  - Debounce resize events
- [x] **1.7** Add basic ANSI color theme support:
  - Default 16-color palette
  - Read from config later

### Deliverables

- A shell running inside rmux, accepting input and displaying output
- Scrolling works (both terminal scrollback and mouse wheel)
- Window resize recalculates terminal grid correctly

### Milestone

**A usable terminal emulator.**

---

## Phase 2: Workspaces + Splits + Sidebar

**Goal**: Multiple panes, split layouts, workspace tabs.

### Tasks

- [x] **2.1** Define `PaneNode` tree model:
  ```rust
  enum PaneNode {
      Leaf { pane: TerminalPane },
      Split { direction: SplitDirection, children: Vec<PaneNode>, sizes: Vec<f32> },
  }
  ```
- [x] **2.2** Implement `Workspace` model:
  - Each workspace owns a `PaneNode` tree
  - Track active pane, pane count, working directory
- [x] **2.3** Implement `WorkspaceManager`:
  - Create/delete/rename workspaces
  - Switch active workspace
  - Persist workspace list
- [x] **2.4** Build `SidebarView` in egui:
  - Vertical tab list showing workspace name + pane count
  - Git branch display (read from cwd `.git/HEAD`)
  - Notification badge (unread count)
  - Highlight active workspace
- [x] **2.5** Implement split commands:
  - Split right / split down
  - Focus pane by direction (arrow keys)
  - Close pane
  - Resize split (drag divider)
- [x] **2.6** Implement keyboard shortcuts:
  - `Cmd+N` / `Ctrl+N`: new workspace
  - `Cmd+D` / `Ctrl+D`: split right
  - `Cmd+Shift+D` / `Ctrl+Shift+D`: split down
  - `Cmd+1..9` / `Ctrl+1..9`: switch workspace
  - `Opt+Cmd+Arrow` / `Alt+Ctrl+Arrow`: focus pane
- [x] **2.7** Add pane memory guardrails:
  - Hibernate offscreen/hidden panes (stop rendering, keep PTY alive)
  - Configurable `max_active_panes` (warn when exceeded)

### Deliverables

- Multiple terminal panes visible via splits
- Sidebar with workspace tabs
- Keyboard shortcuts for split/focus/workspace operations
- Memory stays bounded when many panes exist

### Milestone

**A terminal multiplexer with workspaces and splits.**

---

## Phase 3: Notifications + CLI/Socket API

**Goal**: External control and agent notification support.

### Tasks

- [x] **3.1** Implement OSC parser for notifications:
  - OSC 9 (simple): `\e]9;message\a`
  - OSC 99 (rich): `\e]99;i=1;e=1;d=0;p=title:body\e\\`
  - OSC 777 (legacy): `\e]777;notify;Title;Body\a`
- [x] **3.2** Implement `NotificationManager`:
  - Store notifications per pane
  - Track read/unread state
  - Emit desktop notifications via `notify-rust`
  - Pane ring highlight on unread
  - Sidebar badge count
- [x] **3.3** Build `NotificationPanel` in egui:
  - List of pending notifications
  - Jump-to-pane on click
  - Mark read/clear actions
- [x] **3.4** Implement Unix socket server:
  - JSON-RPC line protocol (newline-delimited JSON)
  - Socket path: `/tmp/rmux.sock` (release) or `/tmp/rmux-debug.sock`
  - Configurable via `RMUX_SOCKET_PATH` env var
- [x] **3.5** Implement socket methods:
  - `system.ping`, `system.capabilities`, `system.identify`
  - `workspace.list`, `workspace.create`, `workspace.select`, `workspace.close`
  - `surface.list`, `surface.split`, `surface.focus`, `surface.send_text`, `surface.send_key`
  - `notification.create`, `notification.list`, `notification.clear`
  - `sidebar.set_status`, `sidebar.clear_status`, `sidebar.set_progress`
- [x] **3.6** Implement `rmux` CLI subcommands:
  - `rmux notify --title T --subtitle S --body B`
  - `rmux new-workspace`, `rmux list-workspaces --json`
  - `rmux new-split right`, `rmux send "echo hi\n"`
  - `rmux ping`, `rmux capabilities`
- [x] **3.7** Implement event streaming:
  - `events.stream` socket method for real-time event feed
  - Events: pane created/closed, notification, workspace changed

### Deliverables

- `rmux notify` triggers desktop notification + sidebar badge
- `rmux list-workspaces --json` returns workspace data
- `rmux send "ls\n"` types into active pane
- Agents can integrate via the socket API

### Milestone

**A scriptable terminal multiplexer.**

---

## Phase 4: Browser Pane

**Goal**: Embedded browser with automation API.

### Tasks

- [ ] **4.1** Integrate `wry` as a pane type:
  - Add `PaneNode::Browser { webview }` variant
  - Embed wry webview in egui via texture/render callback
- [ ] **4.2** Implement browser navigation:
  - `open`, `navigate`, `back`, `forward`, `reload`, `url`
  - Address bar in browser pane header
- [ ] **4.3** Implement browser automation API (subset of cmux's):
  - `click`, `type`, `fill`, `press`
  - `eval` (JavaScript evaluation)
  - `snapshot` (accessibility tree)
  - `screenshot`
- [ ] **4.4** Implement browser session persistence:
  - Save/restore URL and navigation history
  - Cookie persistence via wry
- [ ] **4.5** Add browser keyboard shortcuts:
  - `Cmd+Shift+L` / `Ctrl+Shift+L`: open browser split
  - `Cmd+L` / `Ctrl+L`: focus address bar
  - `Cmd+R` / `Ctrl+R`: reload

### Deliverables

- Browser pane opens and navigates to URLs
- Agents can control the browser via socket API
- Browser state persists across restarts

### Milestone

**A terminal with an embedded browser.**

---

## Phase 5: SSH + Session Restore

**Goal**: Remote workspaces and durable sessions.

### Tasks

- [ ] **5.1** Implement SSH workspace creation:
  - `rmux ssh user@host [-p port] [-i key]`
  - Use `thrussh` or `ssh2` crate for SSH client
  - Remote PTY over SSH channel
- [ ] **5.2** Implement remote file transfer:
  - Drag-and-drop image → SCP upload
  - `rmux scp local remote`
- [ ] **5.3** Implement session save:
  - Save to `~/.config/rmux/session.json`:
    - Window/workspace/pane layout
    - Working directories per pane
    - Scrollback (best effort, configurable max)
    - Browser URL history
  - Save on quit and periodically
- [ ] **5.4** Implement session restore:
  - `rmux restore-session` or `Cmd+Shift+O`
  - Recreate layout, respawn shells in saved directories
  - Restore browser URLs
- [ ] **5.5** Implement resume command bindings:
  - `rmux surface resume set --kind tmux --checkpoint work --shell "tmux attach -t work"`
  - Store approved resume prefixes in config
  - Auto-restore trusted bindings on reopen

### Deliverables

- SSH to remote machine creates a workspace
- Quitting and relaunching restores full layout
- Resume commands work for tmux and agent sessions

### Milestone

**A persistent, remote-capable terminal.**

---

## Phase 6: Agent Hooks + Integration

**Goal**: Drop-in compatibility with cmux agent hooks.

### Tasks

- [ ] **6.1** Define agent hook registry:
  - Each agent: binary name, resume command, session store file
  - Agents: Claude Code, Codex, OpenCode, Gemini, Amp, Cursor CLI, etc.
- [ ] **6.2** Implement `rmux hooks setup [--agent NAME]`:
  - Detect installed agents on PATH
  - Write hook configs to agent config directories
  - Session store: `~/.rmuxterm/<agent>-hook-sessions.json`
- [ ] **6.3** Implement hook event handling:
  - `SessionStart`, `SessionEnd`, `TurnComplete`, `WaitingForInput`
  - Map events to notifications + sidebar status
- [ ] **6.4** Implement agent session resume:
  - Read `~/.rmuxterm/<agent>-hook-sessions.json`
  - Match sessions to workspace/surface
  - Run agent's resume command in restored pane
- [ ] **6.5** Add cmux compatibility layer:
  - Accept `~/.cmuxterm/` session files (read-only migration)
  - Accept `cmux.json` config keys where equivalent

### Deliverables

- `rmux hooks setup` installs hooks for detected agents
- Agent sessions resume on relaunch
- Sidebar shows agent status per workspace

### Milestone

**A terminal that works with AI coding agents.**

---

## Memory Budget

| State | Target |
|---|---|
| Empty window, no panes | < 20 MB |
| 1 terminal pane, shell running | < 30 MB |
| 10 terminal panes, active | < 60 MB |
| 20 terminal panes, mixed active/hibernated | < 100 MB |
| 20 panes + 1 browser pane | < 150 MB |

---

## Rust Best Practices

These are **mandatory** for all code in this project. See [CONVENTIONS.md](./CONVENTIONS.md) for details.

### Error Handling

- Use `thiserror` for library error types
- Use `anyhow` in application code (binary crate)
- Never `.unwrap()` in production code — use `.expect("reason")` or propagate with `?`
- Log errors before swallowing them

### Concurrency

- Use `tokio` for async runtime
- Prefer `tokio::sync::mpsc` channels over `Arc<Mutex<T>>` for shared state
- Use `Arc<RwLock<T>>` only for read-heavy, write-rare shared state
- Never hold a lock across an `.await`

### Safety

- No `unsafe` code unless absolutely necessary and documented
- If `unsafe` is required, wrap it in a safe abstraction with tests
- Use `#[forbid(unsafe_code)]` on modules that should never use it

### Testing

- Unit tests in each module (`#[cfg(test)] mod tests`)
- Integration tests in `tests/` directory
- Test error paths, not just happy paths
- Use `insta` for snapshot tests where appropriate

### Naming & Structure

- Modules: `snake_case`
- Types: `PascalCase`
- Functions/methods: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- One concept per file; group related files in modules

### Dependencies

- Prefer well-maintained crates (> 1000 downloads/day, recent commits)
- Pin major versions, allow minor updates
- Audit new dependencies with `cargo audit`
- Minimize dependency count — evaluate if a feature can be implemented in < 100 lines before adding a crate

### Performance

- Use `#[inline]` sparingly, only for hot-path small functions
- Profile before optimizing — use `cargo flamegraph`
- Prefer iterators over manual loops
- Use `SmallVec` / `ArrayVec` for small, known-size collections
- Avoid allocations in render loops

### Documentation

- Every `pub` function/type must have a doc comment
- Use `///` for public items, `//` for implementation comments
- Include `# Examples` in doc comments for public APIs
- Run `cargo doc --no-deps` to verify docs compile

---

## Project Structure

```
rmux/
├── Cargo.toml              # Workspace manifest
├── AGENTS.md               # This file — agent instructions
├── README.md               # User-facing docs
├── justfile                # Task runner (build, test, lint, fmt)
├── rustfmt.toml            # Formatter config
├── clippy.toml             # Linter config
├── docs/
│   ├── PLAN.md             # This file
│   ├── ARCHITECTURE.md     # Architecture deep-dive
│   └── CONVENTIONS.md      # Rust conventions
├── crates/
│   ├── rmux-app/           # Main application binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── app.rs
│   │       ├── ui/
│   │       ├── workspace/
│   │       ├── notifications/
│   │       └── browser/
│   ├── rmux-terminal/      # Terminal emulation library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── backend.rs
│   │       ├── renderer.rs
│   │       └── scrollback.rs
│   ├── rmux-cli/           # CLI binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── commands.rs
│   │       └── socket.rs
│   ├── rmux-api/           # Socket API server library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── server.rs
│   │       ├── methods.rs
│   │       └── protocol.rs
│   └── rmux-config/        # Configuration management
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           └── schema.rs
└── tests/
    └── integration/
```

---

## Current Status

| Phase | Status | Progress |
|---|---|---|
| Phase 0: Foundation | 🟢 Complete | 100% |
| Phase 1: Terminal Pane | 🟢 Complete | 100% |
| Phase 2: Workspaces | 🟢 Complete | 100% |
| Phase 3: Notifications + API | 🟢 Complete | 100% |
| Phase 4: Browser Pane | ⬜ Ready to start | 0% |
| Phase 5: SSH + Sessions | ⬜ Blocked by Phase 4 | 0% |
| Phase 6: Agent Hooks | ⬜ Blocked by Phase 5 | 0% |
