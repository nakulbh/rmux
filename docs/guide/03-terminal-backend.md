# 03. Terminal backend

PTY backend talks to real shell.

Three jobs:

1. spawn shell
2. write keyboard bytes
3. read shell output

File: `crates/rmux-terminal/src/backend.rs`.

Top comment says purpose:

```rust
//! PTY backend for terminal process management.
//!
//! Manages the pseudo-terminal (PTY) lifecycle: spawning a shell,
//! reading output, writing input, and handling resize events.
//! Built on `portable-pty` for cross-platform PTY support.
```

PTY means pseudo-terminal.

Shell thinks it has terminal.

rmux sits between shell and GUI.

Backend owns process and I/O:

```rust
pub struct PtyBackend {
    /// The spawned child process.
    child: Box<dyn Child + Send + 'static>,
    /// The master PTY (for resize and I/O).
    master: Box<dyn MasterPty + Send>,
    /// Cloned reader for PTY output.
    reader: Option<Box<dyn Read + Send>>,
    /// Writer for PTY input.
    writer: Option<Box<dyn Write + Send>>,
    /// Cloned child killer for signaling.
    child_killer: Box<dyn ChildKiller + Send>,
    /// Whether the child process has exited.
    exited: bool,
}
```

Field map:

| Field | Job | Why needed |
|---|---|---|
| `child` | shell process | check exit status |
| `master` | PTY master side | resize terminal |
| `reader` | output stream | shell to rmux |
| `writer` | input stream | rmux to shell |
| `child_killer` | signal process | close pane cleanly |
| `exited` | cached state | cheap alive check |

Spawn picks shell:

```rust
let shell = std::env::var("SHELL").unwrap_or_else(|_| {
    #[cfg(unix)]
    {
        "/bin/sh".to_string()
    }
    #[cfg(not(unix))]
    {
        "cmd.exe".to_string()
    }
});
```

Why fallback?

Env var may be missing.

App still starts.

PTY size gets created:

```rust
let pty_size = PtySize { rows, cols, pixel_width: 0, pixel_height: 0 };

let pair = pty_system.openpty(pty_size).map_err(PtyError::OpenPty)?;
```

Rows and cols matter more than pixels.

Terminal apps draw by cells.

Spawn command sets terminal type:

```rust
let mut cmd = CommandBuilder::new(&shell);
cmd.env("TERM", "xterm-256color");
```

Why `TERM`?

Shell apps need capabilities.

`xterm-256color` means colors and escape sequences work.

Write path:

```rust
pub fn write(&mut self, data: &[u8]) -> PtyResult<()> {
    if let Some(ref mut writer) = self.writer {
        writer.write_all(data).map_err(PtyError::WriteError)?;
        writer.flush().map_err(PtyError::WriteError)?;
    }
    Ok(())
}
```

Why bytes, not strings?

Terminal input includes arrows, Ctrl keys, ESC sequences.

Not all input is text.

Read path:

```rust
pub fn try_read(&mut self, buf: &mut [u8]) -> Option<usize> {
    let reader = self.reader.as_mut()?;

    match reader.read(buf) {
        Ok(0) => None,
        Ok(n) => Some(n),
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => None,
        Err(_) => None,
    }
}
```

`None` means no bytes now.

UI loop keeps moving.

[Prev: Rust basics](02-rust-basics.md) | [Next: Terminal state](04-terminal-state.md)
