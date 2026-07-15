# 07. OSC notifications

OSC scanner watches terminal output for notifications.

It does not edit output.

It observes bytes, reports completed messages.

File: `crates/rmux-terminal/src/osc.rs`.

Top comment:

```rust
//! OSC notification sequence scanner.
//!
//! Detects notification OSC sequences (OSC 9, OSC 99, OSC 777) in a raw
//! PTY byte stream. The scanner does not transform the stream, it only
//! observes bytes and reports completed notifications. It is stateful and
//! incremental: sequences may be split across multiple [`OscScanner::feed`]
//! calls, including a split `ESC \` (ST) terminator.
```

Why incremental?

PTY output arrives in chunks.

One OSC sequence may be split across reads.

Scanner must remember partial state.

Control bytes:

```rust
const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;
```

Terminal protocols are bytes.

`ESC ]` starts OSC.

`BEL` or `ESC \` can end it.

Safety cap:

```rust
/// Maximum number of payload bytes buffered for a single OSC sequence.
///
/// Payloads exceeding this limit are discarded (the sequence is still
/// consumed up to its terminator, but no notification is produced).
const MAX_PAYLOAD: usize = 4096;
```

Why cap?

Hostile or buggy process could send endless payload.

Cap prevents unbounded memory growth.

Supported kinds:

```rust
pub enum OscKind {
    /// OSC 9 (simple): `ESC ] 9 ; message TERM`.
    Simple9,
    /// OSC 99 (rich): `ESC ] 99 ; k=v;...;p=title:body TERM`.
    Rich99,
    /// OSC 777 (legacy): `ESC ] 777 ; notify ; Title ; Body TERM`.
    Legacy777,
}
```

Different terminals and tools use different notification forms.

rmux accepts three.

Parsed result:

```rust
pub struct OscNotification {
    /// Notification title.
    pub title: String,
    /// Optional notification body text.
    pub body: Option<String>,
    /// Which sequence form produced this notification.
    pub kind: OscKind,
}
```

Why `Option<String>` for body?

Some notifications are just title.

No empty string guessing needed.

State machine:

```rust
enum State {
    /// Scanning ordinary output for an ESC byte.
    #[default]
    Ground,
    /// Saw ESC; deciding whether an OSC sequence starts.
    Escape,
    /// Collecting the numeric OSC code (before the first `;`).
    Code,
    /// Buffering the payload of an interesting OSC code (9/99/777).
    Payload,
    /// Consuming an uninteresting or oversized OSC sequence until its terminator.
    Ignore,
}
```

Why state machine?

Bytes arrive one by one.

Scanner asks: ordinary text, ESC seen, code, payload, or ignore?

Scanner fields:

```rust
pub struct OscScanner {
    /// Current state machine position.
    state: State,
    /// Numeric OSC code accumulated so far.
    code: u32,
    /// Whether the code consisted solely of ASCII digits.
    code_valid: bool,
    /// Payload bytes collected for an interesting sequence.
    buf: Vec<u8>,
    /// Whether the previous byte was an ESC inside a sequence (possible ST start).
    esc_pending: bool,
}
```

Why track `esc_pending`?

`ESC \` terminator takes two bytes.

They may arrive split across chunks.

Flow:

```text
PTY bytes -> OscScanner.feed -> Vec<OscNotification> -> app notification panel
same PTY bytes -> TermState.feed_bytes -> terminal screen
```

Scanner observes. Terminal still receives bytes.

[Prev: Terminal theme](06-terminal-theme.md) | [Next: Input mapper](08-input-mapper.md)
