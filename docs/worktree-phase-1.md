# Phase 1 Worktree Checklist

## rmux-terminal crate

- [x] 1.1 Implement `PtyBackend` (`crates/rmux-terminal/src/backend.rs`)
  - [x] Spawn shell (respect `$SHELL`, fallback to `/bin/sh`)
  - [x] Write input bytes to PTY
  - [x] Try-read non-blocking from PTY
  - [x] Resize PTY
  - [x] Check if child process is alive
  - [x] Take reader for background thread
  - [x] Background reader thread with mpsc channel

- [x] 1.2 Implement `TermState` (`crates/rmux-terminal/src/state.rs`)
  - [x] Wrap `alacritty_terminal::Term<VoidListener>`
  - [x] `new()` with dimensions and scrollback limit
  - [x] `feed_bytes()` through `vte::ansi::Processor`
  - [x] `snapshot()` → GridSnapshot
  - [x] `resize()`
  - [x] `cursor_pos()`

- [x] 1.3 Implement `TerminalRenderer` (`crates/rmux-terminal/src/renderer.rs`)
  - [x] Convert GridSnapshot to egui paint commands
  - [x] Color mapping (NamedColor, Rgb, Indexed → egui::Color32)
  - [x] Cursor rendering with blink
  - [x] Font: monospace, configurable size

- [x] 1.4 Implement `InputMapper` (`crates/rmux-terminal/src/input.rs`)
  - [x] Map alpha keys to ASCII
  - [x] Map special keys to escape sequences (Enter, Tab, Esc, Bksp, Arrows)
  - [x] Map Ctrl-modified keys
  - [x] Bracket paste support

- [x] 1.5 Update `lib.rs` re-exports

## rmux-app integration

- [x] 1.6 Create `crates/rmux-app/src/ui/terminal_pane.rs`
  - [x] TerminalPane struct with PtyBackend + TermState + TerminalRenderer
  - [x] spawn() constructor
  - [x] show() render method
  - [x] process_pty_output() 
  - [x] resize() method
  - [x] Keyboard input handling

- [x] 1.7 Update `crates/rmux-app/src/app.rs`
  - [x] Create TerminalPane on startup
  - [x] Process PTY output in render loop
  - [x] Handle keyboard input
  - [x] Handle resize

## Final verification

- [x] cargo check --workspace
- [x] cargo fmt --all -- --check
- [x] cargo clippy --workspace --all-targets -- -D warnings
- [x] cargo test --workspace (22 unit tests + 3 doctests pass)
- [x] cargo doc --no-deps --workspace
- [x] grep -r "unsafe" crates/ --include="*.rs" (only forbid(unsafe_code) declarations)
- [ ] git commit and push
