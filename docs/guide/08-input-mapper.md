# 08. Input mapper

Input mapper turns GUI key events into terminal bytes.

File: `crates/rmux-terminal/src/input.rs`.

Top comment:

```rust
//! Input mapping for terminal emulation.
//!
//! Maps egui keyboard events to terminal escape sequences
//! that can be written to the PTY. Supports modifier keys,
//! special keys, and bracket paste mode.
```

Why mapping?

Shell does not understand `egui::Key`.

Shell expects bytes.

Letters are bytes. Arrows are escape sequences. Ctrl keys are control bytes.

Struct:

```rust
pub struct InputMapper {
    /// Whether bracket paste mode is active.
    bracket_paste_mode: bool,
}
```

Why bracket paste mode?

Shell apps can ask terminal to wrap pasted text.

This helps editors distinguish paste from typing.

Constructor:

```rust
pub fn new() -> Self {
    Self { bracket_paste_mode: false }
}
```

Default off.

Apps enable it later using terminal escape modes.

Character mapping signature:

```rust
pub fn map_char(&self, c: char, ctrl: bool, alt: bool) -> Vec<u8> {
```

Why return `Vec<u8>`?

One key may become multiple bytes.

`Alt+a` becomes ESC plus `a`.

Enter and Tab:

```rust
match c {
    '\r' | '\n' => {
        // Enter key → carriage return
        vec![b'\r']
    }
    '\t' => {
        // Tab key
        vec![b'\t']
    }
```

Why Enter sends carriage return?

Terminal tradition.

Shell expects `\r` for Enter.

Backspace and Escape:

```rust
'\x08' | '\x7f' => {
    // Backspace
    vec![0x7f]
}
'\x1b' => {
    // Escape
    vec![0x1b]
}
```

Control letters:

```rust
if ctrl {
    // Map Ctrl+letter to the corresponding control character
    match c {
        'a'..='z' => vec![(c as u8) - b'a' + 1],
        'A'..='Z' => vec![(c as u8) - b'A' + 1],
        '[' => vec![0x1b],       // Ctrl+[ = ESC
        '\\' => vec![0x1c],      // Ctrl+\ = FS
        ']' => vec![0x1d],       // Ctrl+] = GS
        ' ' | '@' => vec![0x00], // Ctrl+Space/Ctrl+@ = NUL
```

Why math on letters?

ASCII control chars are 1 through 26 for Ctrl+A through Ctrl+Z.

`Ctrl+C` becomes byte `3`.

Alt prefix:

```rust
} else if alt {
    // Alt+char → ESC + char
    let mut buf = vec![0x1b];
    let mut char_buf = [0u8; 4];
```

Why ESC prefix?

Classic terminal protocol represents Alt as escape prefix.

Flow:

```text
egui keyboard event -> InputMapper -> Vec<u8> -> PtyBackend.write()
```

Beginner trap.

Do not send Rust `char` directly to shell.

Always convert to terminal bytes.

[Prev: OSC notifications](07-osc-notifications.md) | [Next: Workspace splits](09-workspace-splits.md)
