# rmux — Rust Coding Conventions

> Mandatory coding standards for all code in the rmux project.

---

## Table of Contents

- [Formatting](#formatting)
- [Naming](#naming)
- [Error Handling](#error-handling)
- [Concurrency](#concurrency)
- [Memory & Performance](#memory--performance)
- [Testing](#testing)
- [Documentation](#documentation)
- [Dependencies](#dependencies)
- [Anti-Patterns](#anti-patterns)
- [Code Review Checklist](#code-review-checklist)

---

## Formatting

Use `rustfmt` with the project's `rustfmt.toml`. No debates.

```bash
cargo fmt --all
```

**Key rules:**
- 4 spaces indentation (default)
- Max line width: 100 characters
- Trailing comma in multi-line constructs
- One import per line for `use` statements with 3+ items

---

## Naming

| Item | Convention | Example |
|---|---|---|
| Module files | `snake_case.rs` | `terminal_pane.rs` |
| Types (structs, enums) | `PascalCase` | `TerminalPane` |
| Traits | `PascalCase` (adjective or noun) | `Renderable`, `IntoElement` |
| Functions/methods | `snake_case` | `spawn_shell()` |
| Variables | `snake_case` | `cell_size` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_SCROLLBACK` |
| Statics | `SCREAMING_SNAKE_CASE` | `DEFAULT_CONFIG` |
| Enum variants | `PascalCase` | `SplitDirection::Horizontal` |
| Boolean getters | `is_*` / `has_*` | `is_alive()`, `has_focus()` |
| Constructor | `new()` | `TerminalPane::new()` |
| Fallible constructor | `try_new()` or `from_*()` | `Config::from_file()` |
| Conversion | `to_*` (owned), `as_*` (borrow) | `to_string()`, `as_str()` |

**Clippy lints to enforce:**
```rust
#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
)]
```

---

## Error Handling

### Library crates: `thiserror`

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TermError {
    #[error("PTY spawn failed: {0}")]
    PtySpawn(#[from] std::io::Error),

    #[error("Terminal resize failed: {cols}x{rows}")]
    Resize { cols: u16, rows: u16 },

    #[error("Invalid shell path: {path}")]
    InvalidShell { path: String },
}
```

### Application code: `anyhow`

```rust
use anyhow::{Context, Result};

fn load_config() -> Result<Config> {
    let path = config_path();
    let content = std::fs::read_to_string(&path)
        .context("Failed to read config file")?;
    let config: Config = serde_json::from_str(&content)
        .context("Failed to parse config JSON")?;
    Ok(config)
}
```

### Rules

1. **NEVER use `.unwrap()`** in production code
2. Use `.expect("reason")` only when the invariant is provably true and the reason explains why
3. Always propagate errors with `?` or `.context()`
4. Log errors with `tracing::error!` before returning them
5. Create specific error types for each library crate
6. Don't use `String` as an error type — use `thiserror` enums

### Error context pattern

```rust
// BAD: loses context
let data = std::fs::read(path)?;

// GOOD: adds context
let data = std::fs::read(path)
    .with_context(|| format!("Failed to read file: {}", path.display()))?;
```

---

## Concurrency

### Channel patterns

```rust
// Producer-consumer: mpsc channel
let (tx, mut rx) = tokio::sync::mpsc::channel::<PtyEvent>(256);

// Spawn reader task
let reader_tx = tx.clone();
tokio::spawn(async move {
    while let Ok(n) = reader.read(&mut buf).await {
        if reader_tx.send(PtyEvent::Data(buf[..n].to_vec())).await.is_err() {
            break; // receiver dropped
        }
    }
});

// Main loop receives events
while let Some(event) = rx.recv().await {
    match event {
        PtyEvent::Data(bytes) => term_state.feed_bytes(&bytes),
        PtyEvent::Exit(code) => handle_exit(code),
    }
}
```

### Shared state patterns

```rust
// Read-heavy, write-rare: Arc<RwLock<T>>
let state = Arc::new(RwLock::new(AppState::new()));

// Read access (non-blocking if no writer)
let guard = state.read().await;
let workspace = guard.active_workspace();

// Write access (exclusive)
let mut guard = state.write().await;
guard.create_workspace("new".into());
```

### Rules

1. **NEVER hold a lock across `.await`**
   ```rust
   // BAD: deadlock risk
   let mut guard = state.write().await;
   guard.update().await; // holding write lock

   // GOOD: drop lock before await
   let update = {
       let guard = state.read().await;
       guard.prepare_update()
   }; // lock dropped here
   do_async_update(update).await;
   ```

2. Use `tokio::sync::mpsc` for producer-consumer patterns
3. Use `Arc<RwLock<T>>` for read-heavy shared state
4. Use `Arc<Mutex<T>>` only for short critical sections
5. Use `tokio::select!` for concurrent event handling:
   ```rust
   tokio::select! {
       event = pty_rx.recv() => handle_pty(event),
       request = api_rx.recv() => handle_api(request),
       _ = shutdown.recv() => break,
   }
   ```

---

## Memory & Performance

### Allocation rules

- **No allocations in the render loop**
  - Pre-allocate buffers outside the loop
  - Reuse `Vec` with `.clear()` instead of creating new ones
  - Use `String::with_capacity()` when building strings

- **Use stack collections for small, known-size data**
  ```rust
  use smallvec::SmallVec;

  // Most terminal lines are < 200 chars
  type CellBuf = SmallVec<[Cell; 200]>;
  ```

- **Avoid cloning large data**
  ```rust
  // BAD: clones entire grid
  let snapshot = grid.clone();

  // GOOD: borrow or use Arc
  let snapshot = Arc::new(grid.snapshot()); // one allocation
  ```

### Render optimization

```rust
// Batch draws by background color
let mut current_bg = None;
let mut batch_start = 0;

for (i, cell) in row.iter().enumerate() {
    if cell.bg != current_bg {
        if let Some(bg) = current_bg {
            // Flush previous batch
            draw_bg_rect(ui, batch_start, i, bg);
        }
        current_bg = Some(cell.bg);
        batch_start = i;
    }
}
```

### Profiling

```bash
# Generate flamegraph
cargo flamegraph --bin rmux-app

# Benchmark with criterion
cargo bench
```

---

## Testing

### Unit tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_direction_is_vertical() {
        assert!(SplitDirection::Vertical.is_vertical());
        assert!(!SplitDirection::Horizontal.is_vertical());
    }

    #[test]
    fn test_workspace_pane_count() {
        let mut ws = Workspace::new("test".into());
        assert_eq!(ws.pane_count(), 1);

        let new_id = ws.split_right(ws.active_pane().id()).unwrap();
        assert_eq!(ws.pane_count(), 2);
    }

    #[tokio::test]
    async fn test_pty_spawn_and_exit() {
        let mut backend = PtyBackend::spawn("/bin/echo", 80, 24).unwrap();
        // echo exits immediately
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!backend.is_alive());
    }
}
```

### Integration tests

```rust
// tests/integration/workspace_api.rs
use rmux_api::SocketClient;

#[tokio::test]
async fn test_workspace_create_and_list() {
    let client = SocketClient::connect(test_socket_path()).await.unwrap();

    let result = client.call("workspace.create", json!({"name": "test"})).await.unwrap();
    let ws_id = result["id"].as_str().unwrap();

    let list = client.call("workspace.list", json!({})).await.unwrap();
    assert_eq!(list["workspaces"].as_array().unwrap().len(), 1);
}
```

### Test naming

```rust
// GOOD: describes behavior
#[test]
fn test_split_fails_when_pane_not_found() { ... }

// BAD: describes implementation
#[test]
fn test_split_returns_err() { ... }
```

### Test rules

- Test both happy paths AND error paths
- Test boundary conditions (empty input, max values, zero)
- Use `#[should_panic]` sparingly — prefer `Result` returns
- Use `insta` for snapshot tests of complex output
- Each test should be independent and not rely on global state

---

## Documentation

### Public API docs

```rust
/// A terminal pane that manages a PTY process and its rendering.
///
/// # Examples
///
/// ```
/// use rmux_terminal::{TerminalPane, PtyBackend};
///
/// let backend = PtyBackend::spawn("/bin/bash", 80, 24)?;
/// let pane = TerminalPane::new(backend);
/// assert_eq!(pane.cols(), 80);
/// ```
pub struct TerminalPane {
    // ...
}
```

### Rules

- Every `pub` function, struct, enum, and trait MUST have a doc comment
- Use `///` for doc comments, `//` for implementation comments
- Include `# Examples` sections for public APIs
- Include `# Panics` section if the function can panic
- Include `# Errors` section if the function returns `Result`
- Run `cargo doc --no-deps --workspace` to verify docs compile

### Inline comments

```rust
// GOOD: explains WHY, not WHAT
// We limit scrollback to prevent OOM when a process writes大量 output
// (e.g., `cat /dev/urandom`). The limit is configurable in rmux.json.
let max_lines = config.terminal.max_scrollback_lines;

// BAD: restates the code
// Set max_lines to the config value
let max_lines = config.terminal.max_scrollback_lines;
```

---

## Dependencies

### Pre-approved crates

See [AGENTS.md](../AGENTS.md) for the full list.

### Adding new crates

Before adding ANY new dependency:

1. **Can you implement it in < 100 lines?** If yes, do that instead.
2. **Check maintenance status:**
   - Last commit < 6 months ago
   - Download count > 1000/day on crates.io
   - No open security advisories
3. **Minimize feature flags:**
   ```toml
   # BAD: enables everything
   tokio = "1"

   # GOOD: only what we need
   tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "net"] }
   ```
4. **Run `cargo audit`** after adding
5. **Document why** in the commit message
6. **Update AGENTS.md** "Allowed" table

### Cargo.toml rules

```toml
[workspace.dependencies]
# Centralize versions in workspace root
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "net"] }

[package]
name = "rmux-terminal"
version = "0.1.0"
edition = "2024"
license = "MIT"

[dependencies]
# Use workspace versions
serde = { workspace = true }
tokio = { workspace = true }
```

---

## Anti-Patterns

### Code smells to avoid

```rust
// 1. God function — too many responsibilities
fn handle_everything(event: Event) { /* 200 lines */ }

// FIX: split into focused handlers
fn handle_key(event: KeyEvent) { ... }
fn handle_mouse(event: MouseEvent) { ... }
fn handle_resize(event: ResizeEvent) { ... }

// 2. Boolean parameters — unclear at call site
create_pane(true, false, 80);

// FIX: use builder or named fields
create_pane(CreatePaneOptions {
    focused: true,
    hibernated: false,
    cols: 80,
});

// 3. Stringly-typed data
fn set_status(workspace: &str, status: &str) { ... }

// FIX: use strong types
fn set_status(workspace: WorkspaceId, status: Status) { ... }

// 4. Unbounded collections
struct State {
    notifications: Vec<Notification>, // grows forever
}

// FIX: enforce limits
struct State {
    notifications: VecDeque<Notification>, // bounded
    max_notifications: usize,
}

// 5. Clone-happy code
let data = big_struct.clone();
process(data);

// FIX: borrow when possible
process(&big_struct);
```

### Rust-specific anti-patterns

```rust
// 1. Using .unwrap() — NEVER do this
let value = map.get("key").unwrap();

// 2. Using String for errors
fn do_thing() -> Result<(), String> { ... }

// 3. Holding Mutex across .await
let guard = mutex.lock().await;
some_async_fn(guard).await; // DEADLOCK

// 4. Using RefCell in async code
let cell = RefCell::new(value);
// Not Send, can't use across .await

// 5. Excessive Box<dyn Trait> — prefer generics
fn process(handler: Box<dyn Handler>) { ... }
// vs
fn process<H: Handler>(handler: H) { ... }
```

---

## Code Review Checklist

Before submitting a PR, verify:

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo doc --no-deps --workspace` compiles
- [ ] No `.unwrap()` in production code
- [ ] No `unsafe` unless approved and documented
- [ ] All `pub` items have doc comments
- [ ] Error types use `thiserror` (library) or `anyhow` (app)
- [ ] No locks held across `.await`
- [ ] No allocations in render loops
- [ ] Tests cover both happy and error paths
- [ ] New dependencies are documented and justified
- [ ] File is < 300 lines (split if larger)
- [ ] One concept per function (single responsibility)
