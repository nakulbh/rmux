# 02. Rust basics in rmux

Rust in rmux uses small structs, enums, `Result`, and ownership.

No magic needed first.

Read types first. Then methods.

Main pattern:

```text
struct owns data
impl adds behavior
Result reports failure
Option reports missing value
```

Real error enum from `backend.rs`:

```rust
#[derive(Error, Debug)]
pub enum PtyError {
    /// Failed to open a new PTY device.
    #[error("Failed to open PTY: {0}")]
    OpenPty(#[from] anyhow::Error),

    /// Failed to spawn the child process.
    #[error("Failed to spawn child process: {0}")]
    SpawnProcess(#[source] anyhow::Error),

    /// Failed to write input to the PTY.
    #[error("Failed to write to PTY: {0}")]
    WriteError(#[source] std::io::Error),
}
```

Why enum here?

PTY can fail in different ways.

Open failed is not same as write failed.

Caller can print exact cause.

`Result` alias from source:

```rust
pub type PtyResult<T> = Result<T, PtyError>;
```

This shortens method signatures.

Instead of `Result<Self, PtyError>`, code writes `PtyResult<Self>`.

Struct from `state.rs`:

```rust
pub struct GridCell {
    /// The character displayed in this cell.
    pub c: char,
    /// Foreground color.
    pub fg: Color32,
    /// Background color.
    pub bg: Color32,
    /// Whether the text is bold.
    pub bold: bool,
    /// Whether the text is italic.
    pub italic: bool,
}
```

Why struct here?

Terminal screen is grid of cells.

Each cell needs char plus style.

Renderer can draw without asking parser again.

Ownership pattern from `backend.rs`:

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
}
```

`Box<dyn Trait>` means owned value behind trait interface.

Why? `portable-pty` hides OS-specific PTY types.

macOS, Linux, Windows differ. Trait keeps API same.

`Option` means value may be gone.

Example from source:

```rust
pub fn take_reader(&mut self) -> Option<Box<dyn Read + Send>> {
    self.reader.take()
}
```

`take()` moves reader out, leaves `None` behind.

Why? Background thread can own reader. Main backend no longer reads it.

Beginner rule.

When Rust says move, ask: who owns this now?

[Prev: Project layout](01-project-layout.md) | [Next: Terminal backend](03-terminal-backend.md)
