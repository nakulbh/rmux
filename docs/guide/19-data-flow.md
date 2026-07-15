# 19. Data flow

Full loop: key press to shell, shell output to pixels.

## Big picture

```text
User key -> egui Event -> TerminalPane::handle_keyboard_input
  -> InputMapper / map_key_to_terminal -> PtyBackend::write
  -> shell process -> PTY reader thread
  -> TerminalPane::process_pty_output
  -> OscScanner + TermState::feed_bytes
  -> VTE parser updates grid -> TerminalRenderer draws cells
```

## 1. User presses key

egui sends input events to focused terminal pane.

```rust
fn handle_keyboard_input(&mut self, ui: &mut egui::Ui) {
    let events: Vec<egui::Event> = ui.input(|i| i.events.clone());

    for event in &events {
        if let egui::Event::Key { key, pressed, modifiers, .. } = event {
            if !pressed {
                continue;
            }
```

App-level shortcuts get filtered. Terminal receives rest.

```rust
if modifiers.command && !modifiers.ctrl {
    continue;
}
```

## 2. Key becomes bytes

Special keys become escape bytes.

```rust
match key {
    Key::Enter => Some(vec![b'\r']),
    Key::Tab => Some(vec![b'\t']),
    Key::Backspace => Some(vec![0x7f]),
    Key::Escape => Some(vec![0x1b]),
    Key::Delete => Some(vec![0x1b, b'[', b'3', b'~']),
    Key::ArrowUp => Some(vec![0x1b, b'[', b'A']),
    Key::ArrowDown => Some(vec![0x1b, b'[', b'B']),
```

Printable text goes through `InputMapper`.

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

## 3. Bytes go to PTY

Write path:

```rust
let bytes = self.map_key_to_terminal(key, modifiers);
if let Some(data) = bytes {
    self.backend.write(&data).ok();
}
```

API or clear screen can send raw text too:

```rust
pub fn send_text(&mut self, text: &str) {
    if let Err(err) = self.backend.write(text.as_bytes()) {
        tracing::warn!(error = %err, "failed to write text to PTY");
    }
}
```

PTY connects to shell. Shell reads bytes as typed input.

## 4. Shell outputs bytes

TerminalPane spawned reader thread when shell started.

```rust
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

Reader thread sends byte chunks to UI thread through channel.

## 5. Frame drains output

Every app frame calls workspace processing.

```rust
let osc_notifications = self.workspace_manager.process_all_panes();
for (workspace_id, pane_id, notification) in osc_notifications {
    self.add_pane_notification(workspace_id, pane_id, notification);
}
```

Each pane drains channel:

```rust
pub fn process_pty_output(&mut self) {
    while let Ok(data) = self.pty_rx.try_recv() {
        self.pending_notifications.extend(self.osc_scanner.feed(&data));
        self.state.feed_bytes(&data);
    }
```

Two consumers see same bytes:

- `OscScanner` finds notification escape sequences.
- `TermState::feed_bytes` feeds VTE parser.

VTE parser updates grid: chars, colors, cursor, scrollback.

## 6. Render frame

App requests steady repaint:

```rust
ctx.request_repaint_after(std::time::Duration::from_millis(16));
```

Central panel renders pane tree:

```rust
fn render_workspace(&mut self, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let zoomed = self.workspace_manager.active().zoomed_pane;
        workspace_view::render_pane_tree(ui, &mut self.workspace_manager, zoomed);
    });
}
```

TerminalPane `show()` draws terminal grid through renderer.

```rust
/// Draws the terminal grid, handles keyboard input when focused,
/// and shows the cursor. When the find bar is active, it appears
/// at the bottom of the pane.
pub fn show(&mut self, ui: &mut egui::Ui) {
```

## 7. Notifications branch

OSC bytes become app notification:

```rust
let id = self.notifications.add(
    notification.title.clone(),
    notification.body.clone(),
    Some(pane_id),
    Some(workspace_id),
);
```

Then app publishes `notification` event for stream subscribers.

## 8. Shortcut branch

Global shortcut handler runs after UI render:

```rust
self.handle_keyboard_shortcuts(ctx);
```

Shortcut action mutates app state, for example split active pane right.

Core rule: UI thread owns state. Background tasks send messages. Frame loop drains messages.

← **Prev: [18 — Config](18-config.md)**

→ **Next: [20 — Next Steps](20-next-steps.md)**
