# rmux — Testing Strategy

Comprehensive testing approach covering every crate, feature, and layer.

---

## Current State

| Dimension | Status |
|---|---|
| Total tests | **112** (3 `#[tokio::test]` async, rest `#[test]` sync) |
| Test files | 15 inline `#[cfg(test)]` modules + 1 integration test (`rmux-cli`) |
| Testing crates | **None** — only built-in `#[test]` and `#[tokio::test]` |
| Test helpers | No shared utilities; ad-hoc private helpers per module |
| CI/CD | **None** — no `.github/workflows/` |
| Benchmarks | **None** |
| Snapshot testing | **None** |
| Property-based testing | **None** |
| Mocking framework | **None** — only manual trait-object fakes |

**Per-crate test distribution:**

| Crate | Tests | Focus |
|---|---|---|
| `rmux-terminal` | 44 | PTY spawn, VTE parsing, input mapping, OSC scanning, renderer |
| `rmux-app` | 38 | Workspace manager, pane tree, splits, notifications, UI panel |
| `rmux-cli` | 18 | CLI arg parsing, command building, socket round-trip |
| `rmux-api` | 10 | Socket server (with real `tokio` runtime), event streaming |
| `rmux-config` | 2 | Config deserialization, defaults |

---

## Recommended Testing Toolchain

Add to the **workspace-level** `[workspace.dependencies]` section for central version management:

```toml
# Testing (dev-only)
egui_kittest = "0.33"
insta = "1"
proptest = "1"
criterion = "0.5"
```

Add per-crate `[dev-dependencies]` as needed:

```toml
# rmux-app/Cargo.toml
[dev-dependencies]
egui_kittest = { workspace = true }
insta = { workspace = true, features = ["yaml"] }
proptest = { workspace = true }

# rmux-terminal/Cargo.toml
[dev-dependencies]
insta = { workspace = true, features = ["yaml"] }
proptest = { workspace = true }
criterion = { workspace = true, features = ["html_reports"] }

# Top-level Cargo.toml
[[bench]]
name = "terminal_benchmarks"
harness = false
```

**Quick decision guide:**

| What to test | Tool | Why |
|---|---|---|
| egui widget state | `egui_kittest` (Harness) | AccessKit queries + event simulation |
| egui visual output | `egui_kittest::snapshot` | Headless render → image compare |
| PTY spawn + read | `portable-pty` directly | Real PTY, no mocking needed |
| VTE parsing | `TermState::feed_bytes` | Pure in-memory, fast, deterministic |
| Socket API | `tokio::net::UnixStream` | Already done in existing tests |
| Terminal grid output | `insta::assert_debug_snapshot` | `GridSnapshot` implements Debug |
| Pane tree invariants | `proptest` | Property: `splits + 1 = leaf count` |
| Render/throughput perf | `criterion` | Statistical, baseline comparison |

---

## 1. Unit Testing Strategy

### 1.1 Terminal Emulation (`rmux-terminal`)

**Goals:** Test VTE parser correctness, input mapping, OSC scanning — all without a real PTY.

**Pattern — feed ANSI sequences and snapshot:**

```rust
#[test]
fn test_ansi_color_output() {
    let mut state = TermState::new(80, 24, 1000);
    state.feed_bytes(b"\x1b[31mRED\x1b[0m normal\r\n");
    let snap = state.snapshot();
    assert_eq!(snap.cells[0][0].c, 'R');
    insta::assert_debug_snapshot!(snap);
}
```

**Pattern — fuzz VTE parser (never panics):**

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_vte_parser_never_panics(
        bytes in prop::collection::vec(any::<u8>(), 0..4096)
    ) {
        let mut state = TermState::new(80, 24, 1000);
        state.feed_bytes(&bytes);       // must not panic
        let _ = state.snapshot();       // must not panic
    }
}
```

**Existing coverage (44 tests):** PTY spawn/write/resize/kill, input mapping (characters, modifiers, arrows, paste), OSC 9/99/777 parsing with edge cases (split chunks, oversized payloads, invalid UTF-8), renderer calculations, grid operations. **Gaps:** no color/attribute rendering verification, no cursor movement tests, no scrollback boundary tests.

### 1.2 Workspace & Pane Tree (`rmux-app/workspace`)

**Goals:** Verify pane tree invariants hold for any sequence of operations.

**Pattern — property-based on split operations:**

```rust
proptest! {
    #[test]
    fn test_leaf_count_equals_splits_plus_one(
        splits in prop::collection::vec(any::<bool>(), 0..20)
    ) {
        let mut mgr = WorkspaceManager::new();
        for &is_vertical in &splits {
            let dir = if is_vertical { SplitDirection::Vertical }
                      else { SplitDirection::Horizontal };
            mgr.split_active_right().expect("split failed");
        }
        // After N splits: pane_count = N + 1
        let total = mgr.total_pane_count();
        prop_assert_eq!(total as usize, splits.len() + 1);
    }

    #[test]
    fn test_pane_ids_are_unique(
        splits in prop::collection::vec(any::<bool>(), 0..15)
    ) {
        let mut mgr = WorkspaceManager::new();
        for &vertical in &splits {
            let dir = if vertical { SplitDirection::Vertical }
                      else { SplitDirection::Horizontal };
            mgr.split_active_right().expect("split failed");
        }
        let ids: Vec<_> = mgr.active().pane_ids();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        prop_assert_eq!(ids.len(), unique.len(), "duplicate pane IDs found");
    }

    #[test]
    fn test_close_all_panes_returns_to_one(
        mut splits in prop::collection::vec(any::<bool>(), 1..10)
    ) {
        let mut mgr = WorkspaceManager::new();
        for &vertical in &splits {
            let dir = if vertical { SplitDirection::Vertical }
                      else { SplitDirection::Horizontal };
            mgr.split_active_right().expect("split failed");
        }
        // Close all until last one
        while mgr.total_pane_count() > 1 {
            mgr.close_active_pane().expect("close failed");
        }
        prop_assert_eq!(mgr.total_pane_count(), 1);
    }
}
```

**Existing coverage (38 tests):** Workspace CRUD, pane tree splits/closes, focus navigation, zoom toggle, equalize splits, notification manager, guardrails. **Gaps:** no browser pane tests, no resize_split tests, no zoom+close edge cases.

### 1.3 Config (`rmux-config`)

**Existing coverage (2 tests):** Defaults and empty JSON. **Gaps:** round-trip serialize/deserialize, invalid JSON handling, platform-specific path resolution.

### 1.4 Notifications (`rmux-app/notifications`)

**Existing coverage (6 tests):** ID assignment, unread counts, mark-read, mark-all-read, clear, cap at 200. Well-covered using `RecordingNotifier` test double. **Gaps:** none significant.

### 1.5 CLI (`rmux-cli`)

**Existing coverage (18 tests):** Arg parsing, command construction, escape sequence interpretation, socket round-trip (Unix only). Strong coverage of the wire protocol layer.

---

## 2. GUI Testing with `egui_kittest`

### 2.1 Setup

```toml
# crates/rmux-app/Cargo.toml
[dev-dependencies]
egui_kittest = { workspace = true }
```

### 2.2 Pattern — test a UI component in isolation

```rust
use egui_kittest::{Harness, kittest::Queryable};

struct TestState {
    sidebar_visible: bool,
    notification_visible: bool,
}

#[test]
fn test_sidebar_toggle() {
    let mut state = TestState { sidebar_visible: true, notification_visible: false };
    let mut harness = Harness::new_ui_state(|ui, state| {
        if state.sidebar_visible {
            ui.label("SIDEBAR");
        }
        if state.notification_visible {
            ui.label("NOTIFICATIONS");
        }
    }, state);

    assert!(harness.get_by_label("SIDEBAR").is_some());
    assert!(harness.get_by_label("NOTIFICATIONS").is_none());
}
```

### 2.3 Pattern — simulate keyboard shortcuts

```rust
#[test]
fn test_cmd_n_creates_workspace() {
    use egui::Modifiers;

    let mut state = TestAppState::new();
    let mut harness = Harness::new_ui_state(|ui, state| {
        state.render(ui);
    }, state);

    // Simulate Cmd+N
    let modifiers = if cfg!(target_os = "macos") {
        Modifiers::COMMAND
    } else {
        Modifiers::CTRL
    };
    harness.key_down(egui::Key::N);

    assert_eq!(harness.state().workspace_count, 2);
}
```

### 2.4 Pattern — snapshot terminal rendering

```rust
// Requires: egui_kittest = { features = ["wgpu", "snapshot"] }
#[test]
fn test_terminal_rendering_snapshot() {
    let mut terminal = TerminalPane::spawn(80, 24, 14.0).unwrap();
    terminal.send_text("echo hello\n");

    let mut harness = Harness::new_ui(|ui| {
        terminal.show(ui);
    });
    harness.fit_contents(300.0, 200.0);
    harness.snapshot("terminal_hello");
    // UPDATE_SNAPSHOTS=true cargo test  -- to update
}
```

---

## 3. Integration Testing

### 3.1 Socket API End-to-End

**Existing pattern is excellent** — use as reference:

```rust
// tests/api_integration.rs
#[cfg(unix)]
#[tokio::test]
async fn test_full_workflow() {
    let (request_tx, request_rx) = mpsc::channel(16);
    let (event_tx, _event_rx) = broadcast::channel(16);

    let socket_path = temp_socket_path();
    let server = ApiServer::bind(&socket_path, request_tx, event_tx).unwrap();
    let mut client = TestClient::connect(&socket_path).await;

    // Create workspace
    let result = client.call(1, "workspace.create", json!({"name": "test"})).await;
    assert!(result["id"].as_u64().is_some());

    // Create split
    let result = client.call(2, "surface.split", json!({"direction": "Right"})).await;
    assert!(result["pane_id"].as_u64().is_some());

    // Send text
    let result = client.call(3, "surface.send_text", json!({"text": "ls\n"})).await;
    assert_eq!(result["ok"], json!(true));

    server.shutdown();
}
```

### 3.2 PTY + Workspace Integration

```rust
#[test]
fn test_full_terminal_workflow() {
    let mut manager = WorkspaceManager::new();
    let pane_id = manager.active().active_pane;

    // Spawn terminal
    let terminal = TerminalPane::spawn(80, 24, 14.0).unwrap();
    manager.active_mut().set_terminal(pane_id, terminal);

    // Send command
    if let Some(term) = manager.active_mut().active_terminal() {
        term.send_text("echo test\n");
        std::thread::sleep(std::time::Duration::from_millis(200));
        term.process_pty_output();
    }

    // Verify output
    let ws = manager.active();
    let terminal_ref = ws.root.find_leaf(pane_id).unwrap();
    assert!(terminal_ref.is_some());
}
```

### 3.3 Browser Pane Integration

```rust
#[test]
fn test_browser_pane_creation_and_navigation() {
    let mut manager = WorkspaceManager::new();
    let pane_id = manager.active().active_pane;

    // Replace terminal with browser
    let browser = BrowserPane::new();
    manager.active_mut().set_browser(pane_id, browser);

    // Navigate
    if let Some(b) = manager.active_mut().root.find_browser_mut(pane_id) {
        assert_eq!(b.url(), "about:blank");
        let _ = b.navigate("example.com");
        assert_eq!(b.url(), "https://example.com");

        // History
        let _ = b.navigate("other.com");
        let _ = b.go_back();
        assert_eq!(b.url(), "https://example.com");
        let _ = b.go_forward();
        assert_eq!(b.url(), "https://other.com");
    }
}

#[test]
fn test_browser_pane_split_and_close() {
    let mut manager = WorkspaceManager::new();
    let pane_id = manager.active().active_pane;

    let browser = BrowserPane::new();
    manager.active_mut().set_browser(pane_id, browser);

    let _new_id = manager.split_active_right().unwrap();
    manager.active_mut().set_terminal(
        manager.active().active_pane,
        TerminalPane::spawn(80, 24, 14.0).unwrap(),
    );

    assert_eq!(manager.total_pane_count(), 2);
    manager.close_active_pane().unwrap();
    assert_eq!(manager.total_pane_count(), 1);
}
```

---

## 4. Snapshot Testing with `insta`

### 4.1 Terminal Grid Snapshots

```rust
#[test]
fn test_shell_prompt_snapshot() {
    let mut state = TermState::new(80, 24, 1000);
    state.feed_bytes(b"$ echo hello\r\n");
    insta::assert_debug_snapshot!(state.snapshot());
}

#[test]
fn test_ansi_colored_output() {
    let mut state = TermState::new(80, 24, 1000);
    state.feed_bytes(b"\x1b[32mGREEN\x1b[0m normal\r\n");
    insta::assert_debug_snapshot!(state.snapshot());
}
```

### 4.2 API Response Snapshots

```rust
#[test]
fn test_workspace_list_snapshot() {
    let mut manager = WorkspaceManager::new();
    manager.create_workspace("Frontend");
    manager.create_workspace("Backend");

    let workspaces: Vec<_> = manager.workspaces()
        .iter()
        .map(|w| (&w.name, w.pane_count()))
        .collect();
    insta::assert_debug_snapshot!(workspaces);
}
```

### 4.3 Configuration

```rust
// Use sort_maps for deterministic JSON output
insta::with_settings!({sort_maps => true}, {
    insta::assert_debug_snapshot!(api_response);
});
```

### 4.4 Workflow

```bash
# Run tests (never updates snapshots in CI)
cargo test

# Review pending changes locally
cargo insta review

# Accept all changes
cargo insta accept

# In CI: INSTA_UPDATE=never is the default — snapshots must exist
```

---

## 5. Performance Benchmarking

### 5.1 Benchmarks (`benches/terminal_benchmarks.rs`)

```rust
use criterion::{criterion_group, criterion_main, Criterion, Throughput, BenchmarkId};

fn pty_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("pty_throughput");
    for size in [1024, 4096, 16384, 65536] {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let mut state = TermState::new(120, 40, 10_000);
            let data = vec![b'a'; size];
            b.iter(|| state.feed_bytes(&data));
        });
    }
    group.finish();
}

fn render_snapshot(c: &mut Criterion) {
    let mut state = TermState::new(120, 40, 10_000);
    for _ in 0..40 {
        state.feed_bytes(b"Lorem ipsum dolor sit amet.\r\n");
    }
    c.bench_function("snapshot_120x40", |b| b.iter(|| state.snapshot()));
}

fn pane_tree_operations(c: &mut Criterion) {
    fn make_tree(size: usize) -> WorkspaceManager {
        let mut mgr = WorkspaceManager::new();
        for _ in 1..size {
            mgr.split_active_right().unwrap();
        }
        mgr
    }

    let mut group = c.benchmark_group("pane_tree");
    for panes in [5, 10, 20, 50] {
        group.bench_with_input(BenchmarkId::from_parameter(panes), &panes, |b, &p| {
            b.iter_batched(
                || make_tree(p),
                |mut mgr| {
                    mgr.active_mut().root.pane_count();
                    mgr.active_mut().root.pane_ids();
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, pty_throughput, render_snapshot, pane_tree_operations);
criterion_main!(benches);
```

### 5.2 What to benchmark

| Benchmark | Metric | Target |
|---|---|---|
| `TermState::feed_bytes()` throughput | MB/s | > 50 MB/s |
| `TermState::snapshot()` | μs | < 1 ms for 120×40 |
| Pane tree `pane_count()` / `pane_ids()` | μs | < 100 μs for 50 panes |
| `TerminalRenderer::draw()` | ms | < 2 ms per frame |
| `OscScanner::feed()` | μs | < 10 μs for typical payload |

### 5.3 Baseline comparison

```bash
cargo bench -- --save-baseline v0.1
# ... after changes ...
cargo bench -- --baseline v0.1
```

---

## 6. CI Pipeline

### 6.1 GitHub Actions Workflow

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  test-linux:
    name: Test (Linux)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - name: System deps (egui + PTY)
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
            libxkbcommon-dev libssl-dev libgtk-3-dev libglib2.0-dev \
            libatk1.0-dev libcairo2-dev
      - uses: Swatinem/rust-cache@v2
      - name: Format
        run: cargo fmt --all -- --check
      - name: Clippy
        run: cargo clippy --workspace --all-targets -- -D warnings
      - name: Test
        run: cargo test --workspace
      - name: Doc
        run: cargo doc --no-deps --workspace

  test-macos:
    name: Test (macOS)
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --workspace

  test-windows:
    name: Test (Windows)
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --workspace

  security:
    name: Security Audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo install cargo-audit
      - run: cargo audit --deny warnings

  bench:
    name: Benchmarks
    runs-on: ubuntu-latest
    needs: test-linux  # only run if tests pass
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: System deps
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
            libxkbcommon-dev libssl-dev libgtk-3-dev
      - uses: Swatinem/rust-cache@v2
      - run: cargo bench -- --output-format bencher | tee bench.txt
      - uses: benchmark-action/github-action-benchmark@v1
        with:
          tool: cargo
          output-file-path: bench.txt
          github-token: ${{ secrets.GITHUB_TOKEN }}
          auto-push: true
```

### 6.2 Snapshot testing in CI

```bash
# Snapshots must be committed — CI never auto-updates
# If a snapshot is missing, the test fails
INSTA_UPDATE=never cargo test  # default behavior

# For wgpu snapshot tests, use Mesa software rasterizer:
LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe cargo test
```

---

## 7. Testing with OpenCode

### 7.1 How to trigger tests via opencode

```
# Run all tests
cargo test --workspace

# Run a specific crate
cargo test -p rmux-terminal

# Run a specific test
cargo test test_split_tree_preserves_leaf_count

# Run with snapshot updates
UPDATE_SNAPSHOTS=true cargo test

# Run benchmarks
cargo bench

# Profile a test
cargo test --profile=release test_pty_throughput
```

### 7.2 Test-first workflow with opencode

When implementing a feature in a new branch:

1. **Plan** — Read `docs/PLAN.md` for the task specification
2. **Write tests first** — Create `#[cfg(test)]` module with the expected behavior
3. **Run tests (expect failure)** — `cargo test -p rmux-app test_new_feature`
4. **Implement** — Write the feature code
5. **Run tests (expect pass)** — Verify all tests pass
6. **Run verification** — `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo doc --no-deps --workspace`
7. **Commit and push** — Open a PR

### 7.3 What opencode CAN test

| Capability | How |
|---|---|
| Unit tests | `cargo test` — fast feedback, runs anywhere |
| Integration tests | `cargo test --test integration` — PTY, socket, workspace |
| Build verification | `cargo check`, `cargo clippy`, `cargo fmt` |
| Benchmarks | `cargo bench` — throughput and latency |
| Snapshot review | `cargo insta review` — interactive diff review |
| CI status | GitHub Actions — automatic on push/PR |

### 7.4 What opencode CANNOT test (needs human or display)

| Limitation | Reason |
|---|---|
| Visual appearance of egui UI | Needs actual display or `egui_kittest` with wgpu snapshot |
| Desktop notifications appearing | Needs OS notification center |
| Browser pane rendering with wry | Needs real OS webview (WKWebView/WebView2) |
| macOS-specific keyboard shortcuts | Needs macOS CI runner or local machine |
| Real-time rendering latency | Needs monitor with vsync |

### 7.5 Pattern — opencode writes tests, human verifies visuals

When opencode implements a UI change:

1. OpenCode writes unit tests for logic (state changes, pane tree ops)
2. OpenCode writes `insta` snapshot tests for terminal grid output
3. Human runs `cargo run` to verify visual appearance
4. Human takes screenshot, updates snapshots: `UPDATE_SNAPSHOTS=true cargo test`
5. Committed snapshots become regression tests

---

## 8. Test Coverage Gaps

| Area | Current | Target | Priority |
|---|---|---|---|
| VTE color/attribute rendering | 0 tests | 5+ snapshot tests | High |
| Cursor movement (CSI sequences) | 0 tests | 8+ tests | High |
| Scrollback boundary behavior | 0 tests | 4+ tests | Medium |
| Browser pane (new) | 0 tests | 10+ tests | High |
| Pane tree resize_split | 0 tests | 5+ tests | Medium |
| Zoom + close edge cases | 0 tests | 3+ tests | Low |
| Font size propagation | 0 tests | 3+ tests | Medium |
| API dispatch error paths | Partial | 5+ tests | Medium |
| Workspace close edge cases | Partial | 3+ tests | Low |
| Property-based pane tree ops | 0 tests | 5+ proptest | Medium |
| Grid snapshot regression | 0 tests | 10+ insta snapshots | High |

---

## 9. Quick Reference

```bash
# Run everything
just test            # cargo test --workspace
just lint            # cargo clippy --workspace --all-targets -- -D warnings
just check           # fmt + lint + test

# Run specific tests
cargo test -p rmux-terminal test_osc
cargo test --test socket_roundtrip

# Update snapshots
UPDATE_SNAPSHOTS=true cargo test

# Run benchmarks
cargo bench

# Run security audit
cargo audit
```

---

## 10. Test File Organization (Recommended)

```
crates/rmux-terminal/
├── benches/
│   └── terminal_benchmarks.rs    # criterion benchmarks
└── src/
    ├── snapshots/                # insta snapshot files
    │   ├── osc_test__test_osc99_rich__
    │   └── state_test__test_ansi_colors__
    └── ...

crates/rmux-app/
├── tests/
│   ├── integration/
│   │   ├── mod.rs
│   │   ├── workspace_tests.rs
│   │   ├── browser_tests.rs
│   │   └── api_tests.rs
│   └── snapshots/               # shared snapshots
└── src/
    └── ...

crates/rmux-api/
└── tests/
    └── integration/
        └── socket_tests.rs

crates/rmux-cli/
└── tests/
    ├── integration/
    │   └── cli_tests.rs
    └── snapshots/

.github/
└── workflows/
    └── ci.yml                    # CI pipeline
```
