# 14. Terminal pane widget

TerminalPane = shell process plus terminal state plus egui widget.

File: `crates/rmux-app/src/ui/terminal_pane.rs`.

Top comment:

```rust
//! Terminal pane widget.
//!
//! Wraps a PTY backend, terminal state, and renderer into
//! a self-contained egui widget that can be placed in split layouts.
```

## Struct fields

Core fields:

```rust
pub struct TerminalPane {
    /// The PTY backend managing the shell process.
    backend: PtyBackend,
    /// The terminal emulator state (grid, scrollback, cursor).
    state: TermState,
    /// The terminal grid renderer.
    renderer: TerminalRenderer,
    /// Input mapper for keyboard events.
    input_mapper: InputMapper,
    /// Channel receiver for PTY output from background thread.
    pty_rx: mpsc::Receiver<Vec<u8>>,
```

## Spawn

`spawn()` creates PTY, grid, renderer, input mapper.

```rust
pub fn spawn(cols: u16, rows: u16, font_size: f32) -> Result<Self, PtyError> {
    let mut backend = PtyBackend::spawn(cols, rows)?;
    let state = TermState::new(cols, rows, 10_000);
    let renderer = TerminalRenderer::new(font_size);
    let input_mapper = InputMapper::new();
```

PTY output comes from background thread.

```rust
let (tx, rx) = mpsc::channel::<Vec<u8>>();

if let Some(reader) = backend.take_reader() {
    let mut reader = reader;
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF: process exited
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break; // receiver dropped
                    }
                }
```

PTY read can block. UI frame must not block.

## Process PTY output

Each frame drains bytes, scans OSC, feeds terminal state.

```rust
pub fn process_pty_output(&mut self) {
    while let Ok(data) = self.pty_rx.try_recv() {
        self.pending_notifications.extend(self.osc_scanner.feed(&data));
        self.state.feed_bytes(&data);
    }

    // Check if the PTY process has exited
    if !self.exited && self.backend.try_wait().is_some() {
        self.exited = true;
        self.name.push_str(" [exited]");
    }
}
```

`try_recv()` drains what exists. No waiting.

## show()

`show(&mut self, ui)` is widget entry point.

```rust
/// Render the terminal pane in the egui UI.
///
/// Draws the terminal grid, handles keyboard input when focused,
/// and shows the cursor. When the find bar is active, it appears
/// at the bottom of the pane.
pub fn show(&mut self, ui: &mut egui::Ui) {
    // Process any new PTY output
```

It drains output, allocates rectangle, draws grid, handles input.

## Keyboard input

`handle_keyboard_input()` reads egui events.

```rust
fn handle_keyboard_input(&mut self, ui: &mut egui::Ui) {
    let events: Vec<egui::Event> = ui.input(|i| i.events.clone());

    for event in &events {
        if let egui::Event::Key { key, pressed, modifiers, .. } = event {
            if !pressed {
                continue;
            }
```

Cmd-only macOS chords stay app-level.

```rust
if modifiers.command && !modifiers.ctrl {
    continue;
}
```

Linux and Windows reserve app Ctrl chords.

```rust
if !cfg!(target_os = "macos")
    && modifiers.ctrl
    && !modifiers.command
    && self.is_reserved_app_key(key)
{
    continue;
}
```

Text input handles normal typing, paste, IME.

```rust
if let egui::Event::Text(text) = event {
    for c in text.chars() {
        let bytes = self.input_mapper.map_char(c, false, false);
        if !bytes.is_empty() {
            self.backend.write(&bytes).ok();
        }
    }
}
```

## Find bar and image paste

Find bar is pane state. Shortcuts call active pane methods.

```rust
pub fn is_find_visible(&self) -> bool {
    self.find_visible
}
```

When find bar is open, Escape and Enter are shortcut keys, not shell input.

```rust
if self.find_visible
    && !modifiers.command
    && !modifiers.ctrl
    && !modifiers.alt
    && !modifiers.shift
    && matches!(key, egui::Key::Escape | egui::Key::Enter)
{
    continue;
}
```

Image paste reads clipboard image, writes temp PNG, injects path.

```rust
pub fn try_paste_image(&mut self) -> bool {
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(_) => return false,
    };
```

```rust
self.paste_counter += 1;
let mut path = std::env::temp_dir();
path.push(format!("rmux-paste-{}-{}.png", std::process::id(), self.paste_counter));

let path_str = path.to_string_lossy();
self.backend.write(format!("\"{path_str}\"").as_bytes()).ok();
```

← **Prev: [13 — UI Topbar Sidebar](13-ui-topbar-sidebar.md)**

→ **Next: [15 — Shortcuts](15-shortcuts.md)**
