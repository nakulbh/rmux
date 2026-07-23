# AGENTS.md — rmux Project Instructions

> **rmux** — Cross-platform, memory-efficient terminal multiplexer GUI, written in Rust.
> Inspired by [cmux](https://github.com/manaflow-ai/cmux). Targets Linux, macOS, Windows.

---

## Quick Reference

| Item | Value |
|---|---|
| Language | Rust (edition 2024) |
| GUI framework | `egui` + `eframe` |
| Terminal emulator | `alacritty_terminal` |
| PTY | `portable-pty` |
| Async runtime | `tokio` |
| Browser pane | `wry` |
| Notifications | `notify-rust` |
| Min RAM target | < 100 MB with 20 panes |
| Platforms | Linux, macOS, Windows |

---

## Project Plan

**The plan is the source of truth.** Read it before doing anything:

→ [`docs/PLAN.md`](docs/PLAN.md)

### How to work on tasks

1. **Read the current phase** in `docs/PLAN.md`
2. **Pick the next unmarked task** (marked with `[ ]`)
3. **Implement it** following the conventions below
4. **Write tests** for the task
5. **Run verification** (see "Verification Checklist" below)
6. **Mark the task done** in `docs/PLAN.md` (change `[ ]` to `[x]`)
7. **Commit and push** after the implementation is complete

### Phase progression

Do NOT skip phases. Each phase depends on the previous one. Within a phase, tasks are ordered — do them sequentially unless they are clearly independent.

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    rmux-app (binary)                     │
│  ┌─────────┐  ┌──────────────┐  ┌───────────────────┐   │
│  │   UI    │  │  Workspace   │  │  Notifications    │   │
│  │ (egui)  │  │  Manager     │  │  Manager          │   │
│  └────┬────┘  └──────┬───────┘  └─────────┬─────────┘   │
│       │              │                     │             │
│  ┌────▼──────────────▼─────────────────────▼─────────┐   │
│  │              Application State (app.rs)           │   │
│  └────┬──────────────┬─────────────────────┬─────────┘   │
│       │              │                     │             │
│  ┌────▼────┐  ┌──────▼───────┐  ┌─────────▼─────────┐   │
│  │ rmux-   │  │  rmux-api    │  │  rmux-config      │   │
│  │terminal │  │ (socket srv) │  │ (config mgmt)     │   │
│  └─────────┘  └──────────────┘  └───────────────────┘   │
└─────────────────────────────────────────────────────────┘
         ▲
         │
┌────────▼────────┐
│   rmux-cli      │
│ (separate bin)  │
└─────────────────┘
```

### Crate responsibilities

| Crate | Type | Purpose |
|---|---|---|
| `rmux-app` | binary | Main application. Owns the egui window, event loop, and orchestrates all subsystems. |
| `rmux-terminal` | library | Terminal emulation. Wraps `alacritty_terminal` + `portable-pty`. Owns PTY lifecycle, grid state, scrollback. |
| `rmux-cli` | binary | CLI tool (`rmux-cli` command). Connects to socket, sends commands, prints results. |
| `rmux-api` | library | Socket server. JSON-RPC protocol, method dispatch, event streaming. |
| `rmux-config` | library | Configuration loading/saving. `rmux.json` schema, Ghostty config import. |

### Data flow

```
User keyboard → egui event → TerminalPane → PtyBackend.write()
                                                  │
PTY output → PtyBackend.read() → TermState.feed() → Grid updated
                                                  │
egui render → TerminalRenderer.draw(grid) → Screen pixels
                                                  │
OSC sequence → NotificationManager → Sidebar badge + desktop notification
```

---

## Rust Best Practices (Mandatory)

### Error Handling

```rust
// Library crates: use thiserror
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TermError {
    #[error("PTY spawn failed: {0}")]
    PtySpawn(#[from] std::io::Error),
    #[error("Resize failed: cols={cols}, rows={rows}")]
    Resize { cols: u16, rows: u16 },
}

// Application code: use anyhow
use anyhow::{Context, Result};

fn load_config() -> Result<Config> {
    let path = config_path();
    let content = std::fs::read_to_string(&path)
        .context("Failed to read config file")?;
    serde_json::from_str(&content)
        .context("Failed to parse config JSON")
}
```

**Rules:**
- NEVER use `.unwrap()` in production code
- Use `.expect("specific reason")` only when the invariant is provably true
- Always use `?` or `.context()` to propagate errors
- Log errors with `tracing::error!` before returning them

### Concurrency

```rust
// Prefer channels over shared mutexes
let (tx, mut rx) = tokio::sync::mpsc::channel::<PtyEvent>(256);

// Spawn PTY reader on separate task
tokio::spawn(async move {
    while let Some(event) = rx.recv().await {
        // handle event
    }
});

// NEVER hold a lock across .await
let data = lock.read().await;  // OK: short hold
process(data).await;           // lock is already dropped

// BAD:
let mut data = lock.write().await;
data.update().await;  // holding write lock across await — DEADLOCK RISK
```

**Rules:**
- Use `tokio::sync::mpsc` for producer-consumer patterns
- Use `Arc<RwLock<T>>` only for read-heavy shared state
- NEVER hold a `Mutex` or `RwLock` across `.await`
- Use `tokio::select!` for concurrent event handling

### Module Organization

```rust
// src/ui/mod.rs — public interface
mod sidebar;
mod terminal_pane;
mod workspace_view;

pub use sidebar::SidebarView;
pub use terminal_pane::TerminalPane;
pub use workspace_view::WorkspaceView;

// src/ui/terminal_pane.rs — implementation
pub struct TerminalPane {
    // fields
}

impl TerminalPane {
    /// Create a new terminal pane with the given PTY backend.
    pub fn new(backend: PtyBackend) -> Self { ... }

    /// Render the terminal grid into egui paint commands.
    pub fn show(&mut self, ui: &mut egui::Ui) { ... }
}
```

**Rules:**
- One concept per file
- `mod.rs` only re-exports and doc comments
- Keep files under 300 lines — split if larger
- Group related types in the same module

### Naming Conventions

| Item | Convention | Example |
|---|---|---|
| Module | `snake_case` | `terminal_pane.rs` |
| Type | `PascalCase` | `TerminalPane` |
| Function/method | `snake_case` | `spawn_shell()` |
| Constant | `SCREAMING_SNAKE` | `MAX_SCROLLBACK` |
| Static | `SCREAMING_SNAKE` | `DEFAULT_CONFIG` |
| Enum variant | `PascalCase` | `SplitDirection::Horizontal` |
| Trait | `PascalCase` or verb | `Render`, `IntoElement` |

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_direction_horizontal() {
        let dir = SplitDirection::Horizontal;
        assert_eq!(dir.is_vertical(), false);
    }

    #[tokio::test]
    async fn test_pty_spawn_shell() {
        let backend = PtyBackend::spawn("/bin/sh", 80, 24).unwrap();
        assert!(backend.is_alive());
    }
}
```

**Rules:**
- Unit tests in each module: `#[cfg(test)] mod tests`
- Integration tests in `tests/` directory
- Test both happy paths AND error paths
- Test names describe behavior: `test_spawn_fails_with_invalid_shell`
- Run: `cargo test`, `cargo test --workspace`

### Performance Rules

- NO allocations in the render loop
- Use iterators over manual loops
- Use `SmallVec` for known-small collections (< 16 items)
- Benchmark hot paths with `criterion`
- Profile with `cargo flamegraph` before optimizing

### Safety Rules

- `#[forbid(unsafe_code)]` on all crates unless explicitly approved
- If `unsafe` is required: document the invariant, wrap in safe API, add tests
- No raw pointer manipulation in application code

---

## Verification Checklist

Before marking a task complete, run ALL of these:

```bash
# 1. Format check
cargo fmt --all -- --check

# 2. Lint check (zero warnings)
cargo clippy --workspace --all-targets -- -D warnings

# 3. Tests pass
cargo test --workspace

# 4. Docs compile
cargo doc --no-deps --workspace

# 5. No unsafe (unless approved)
grep -r "unsafe" crates/ --include="*.rs" | grep -v "#\[forbid(unsafe_code)\]"
# Should return nothing
```

### justfile targets (to be created)

```just
fmt:
    cargo fmt --all

lint:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo test --workspace

check: fmt lint test
    echo "All checks passed"

doc:
    cargo doc --no-deps --workspace --open
```

---

## Dependency Rules

### Allowed (pre-approved for this project)

| Purpose | Crate | Notes |
|---|---|---|
| GUI | `eframe`, `egui` | Core GUI framework |
| Terminal | `alacritty_terminal` | Terminal emulation model |
| PTY | `portable-pty` | Cross-platform PTY |
| Async | `tokio` (rt, macros, sync, net, fs) | Async runtime |
| Serialization | `serde`, `serde_json` | Config + API protocol |
| CLI | `clap` (derive) | Argument parsing |
| Errors | `thiserror`, `anyhow` | Error handling |
| Logging | `tracing`, `tracing-subscriber` | Structured logging |
| Notifications | `notify-rust` | Desktop notifications |
| Browser | `wry` | OS webview embedding |
| Config dirs | `dirs` | XDG/AppData paths |
| Images | `image` (png/jpeg/webp/gif) | App icon + workspace wallpaper |
| File dialog | `rfd` | Settings wallpaper picker |
| Fuzzy match | `nucleo` | Command palette |
| SSH | `thrussh` or `ssh2` | Remote sessions (Phase 5) |
| Testing | `insta`, `criterion` | Snapshot + benchmark |

### Adding new dependencies

Before adding ANY crate:

1. Check if the feature can be implemented in < 100 lines without it
2. Check the crate's maintenance status (recent commits, download count)
3. Run `cargo audit` after adding it
4. Add it to this file under "Allowed"
5. Minimize feature flags — only enable what you need

---

## File Organization

```
crates/
├── rmux-app/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs              # Entry point, arg parsing, app init
│       ├── app.rs               # Global application state
│       ├── ui/
│       │   ├── mod.rs           # Re-exports
│       │   ├── root.rs          # Root egui layout
│       │   ├── sidebar.rs       # Vertical tabs
│       │   ├── terminal_pane.rs # Terminal widget
│       │   ├── workspace_view.rs# Split container
│       │   └── notification_panel.rs
│       ├── workspace/
│       │   ├── mod.rs
│       │   ├── model.rs         # Workspace struct
│       │   ├── splits.rs        # Pane tree
│       │   └── session.rs       # Save/restore
│       ├── notifications/
│       │   ├── mod.rs
│       │   ├── osc_parser.rs    # OSC 9/99/777
│       │   └── manager.rs       # State + desktop notify
│       └── browser/
│           └── webview.rs       # wry wrapper
├── rmux-terminal/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── backend.rs           # portable-pty wrapper
│       ├── state.rs             # alacritty_terminal::Term wrapper
│       ├── renderer.rs          # Grid → egui paint commands
│       └── input.rs             # Keyboard/mouse → escape sequences
├── rmux-cli/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── commands.rs          # Subcommand definitions
│       └── socket.rs            # Unix socket client
├── rmux-api/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── server.rs            # Socket server
│       ├── methods.rs           # Method handlers
│       └── protocol.rs          # JSON-RPC line protocol
└── rmux-config/
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        └── schema.rs           # rmux.json types
```

---

## Git Conventions

- Branch naming: `feat/phase-N-description` (e.g., `feat/phase-1-pty-backend`)
- Commit messages: conventional commits format
  - `feat: add PTY backend using portable-pty`
  - `fix: handle resize race condition`
  - `test: add unit tests for pane tree`
  - `docs: update PLAN.md Phase 1 tasks`
- One logical change per commit
- Run verification checklist before committing

---

## Common Patterns

### The PTY → Terminal → Render pipeline

```rust
// 1. Spawn PTY
let pty = portable_pty::native_pty_system()
    .openpty(PtySize { rows: 24, cols: 80, .. })?;
let child = pty.slave.spawn_command(Command::new(shell))?;

// 2. Read PTY output → feed to terminal
let mut reader = pty.master.try_clone_reader()?;
tokio::spawn(async move {
    let mut buf = [0u8; 4096];
    loop {
        let n = reader.read(&mut buf).await?;
        term_state.feed_bytes(&buf[..n]);
    }
});

// 3. Render terminal grid
fn show_terminal(ui: &mut egui::Ui, term: &TermState) {
    let grid = term.grid();
    for row in 0..grid.rows() {
        for col in 0..grid.cols() {
            let cell = grid.cell(row, col);
            // draw cell as egui rect + glyph
        }
    }
}
```

### The socket API pattern

```rust
// Server side
async fn handle_connection(stream: UnixStream) {
    let (reader, writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let request: JsonRpcRequest = serde_json::from_str(&line)?;
        let response = dispatch_method(request).await;
        writer.write_all(serde_json::to_vec(&response)?).await?;
    }
}

// Client side (rmux-cli)
async fn send_command(method: &str, params: Value) -> Result<Value> {
    let stream = UnixStream::connect(socket_path()).await?;
    let request = json!({"id": 1, "method": method, "params": params});
    stream.write_all(serde_json::to_vec(&request)?).await?;
    // read response
}
```

---

## Reference Files

| File | What it contains |
|---|---|
| `docs/PLAN.md` | Full phased plan with all tasks |
| `docs/ARCHITECTURE.md` | Detailed architecture and data flow |
| `docs/CONVENTIONS.md` | Extended Rust conventions |
| `Cargo.toml` (workspace) | Dependencies and workspace config |
| `justfile` | Build/test/lint commands |
