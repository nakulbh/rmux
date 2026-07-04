//! OSC notification sequence scanner.
//!
//! Detects notification OSC sequences (OSC 9, OSC 99, OSC 777) in a raw
//! PTY byte stream. The scanner does not transform the stream — it only
//! observes bytes and reports completed notifications. It is stateful and
//! incremental: sequences may be split across multiple [`OscScanner::feed`]
//! calls, including a split `ESC \` (ST) terminator.

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;

/// Maximum number of payload bytes buffered for a single OSC sequence.
///
/// Payloads exceeding this limit are discarded (the sequence is still
/// consumed up to its terminator, but no notification is produced).
const MAX_PAYLOAD: usize = 4096;

/// Which OSC sequence form produced a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OscKind {
    /// OSC 9 (simple): `ESC ] 9 ; message TERM`.
    Simple9,
    /// OSC 99 (rich): `ESC ] 99 ; k=v;...;p=title:body TERM`.
    Rich99,
    /// OSC 777 (legacy): `ESC ] 777 ; notify ; Title ; Body TERM`.
    Legacy777,
}

/// A notification parsed from an OSC sequence in the PTY output stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OscNotification {
    /// Notification title.
    pub title: String,
    /// Optional notification body text.
    pub body: Option<String>,
    /// Which sequence form produced this notification.
    pub kind: OscKind,
}

/// Internal scanner state.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
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

/// Incremental, bounded scanner for notification OSC sequences.
///
/// Feed it the same raw bytes that go into the terminal emulator; it
/// detects OSC 9/99/777 notification sequences and ignores everything
/// else (including other OSC codes such as 0, 2, or 52). The payload
/// buffer is capped at 4096 bytes, so garbage or hostile input can never
/// grow memory unboundedly.
#[derive(Debug, Default)]
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

impl OscScanner {
    /// Create a new scanner in the ground state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a chunk of PTY bytes; returns any notifications completed in this chunk.
    ///
    /// Sequences may be split across calls — the scanner keeps its state
    /// between chunks, including a pending `ESC` that may turn out to be
    /// the first half of an `ESC \` (ST) terminator.
    ///
    /// # Examples
    ///
    /// ```
    /// use rmux_terminal::{OscKind, OscScanner};
    ///
    /// let mut scanner = OscScanner::new();
    /// let found = scanner.feed(b"\x1b]9;build finished\x07");
    /// assert_eq!(found.len(), 1);
    /// assert_eq!(found[0].title, "build finished");
    /// assert_eq!(found[0].body, None);
    /// assert_eq!(found[0].kind, OscKind::Simple9);
    /// ```
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<OscNotification> {
        let mut out = Vec::new();
        for &byte in bytes {
            self.step(byte, &mut out);
        }
        out
    }

    /// Advance the state machine by one byte.
    fn step(&mut self, byte: u8, out: &mut Vec<OscNotification>) {
        match self.state {
            State::Ground => {
                if byte == ESC {
                    self.state = State::Escape;
                }
            }
            State::Escape => match byte {
                b']' => {
                    self.state = State::Code;
                    self.code = 0;
                    self.code_valid = true;
                    self.buf.clear();
                    self.esc_pending = false;
                }
                // Another ESC: still waiting to see what follows it.
                ESC => {}
                _ => self.state = State::Ground,
            },
            State::Code | State::Payload | State::Ignore => self.step_in_sequence(byte, out),
        }
    }

    /// Handle a byte while inside an OSC sequence (code, payload, or ignore).
    fn step_in_sequence(&mut self, byte: u8, out: &mut Vec<OscNotification>) {
        if self.esc_pending {
            self.esc_pending = false;
            if byte == b'\\' {
                // ESC \ (ST) terminates the sequence.
                self.finish(out);
            } else {
                // Interrupted by an escape that is not ST: discard the
                // current sequence and resync on the new escape.
                self.reset();
                self.state = State::Escape;
                self.step(byte, out);
            }
            return;
        }
        match byte {
            ESC => self.esc_pending = true,
            BEL => self.finish(out),
            _ => self.consume_sequence_byte(byte),
        }
    }

    /// Consume a non-terminator byte of the current sequence.
    fn consume_sequence_byte(&mut self, byte: u8) {
        match self.state {
            State::Code => match byte {
                b'0'..=b'9' => {
                    self.code = self.code.saturating_mul(10).saturating_add(u32::from(byte - b'0'));
                }
                b';' => {
                    self.state = if self.code_valid && matches!(self.code, 9 | 99 | 777) {
                        State::Payload
                    } else {
                        State::Ignore
                    };
                }
                _ => self.code_valid = false,
            },
            State::Payload => {
                if self.buf.len() >= MAX_PAYLOAD {
                    // Oversized payload: discard and stop buffering, but
                    // keep consuming until the terminator.
                    self.buf.clear();
                    self.state = State::Ignore;
                } else {
                    self.buf.push(byte);
                }
            }
            // Uninteresting sequence: swallow bytes without buffering.
            State::Ignore | State::Ground | State::Escape => {}
        }
    }

    /// Sequence terminator reached: emit a notification if applicable, then reset.
    fn finish(&mut self, out: &mut Vec<OscNotification>) {
        if self.state == State::Payload
            && let Some(notification) = parse_notification(self.code, &self.buf)
        {
            out.push(notification);
        }
        self.reset();
    }

    /// Return to the ground state, dropping any buffered payload.
    fn reset(&mut self) {
        self.state = State::Ground;
        self.code = 0;
        self.code_valid = false;
        self.buf.clear();
        self.esc_pending = false;
    }
}

/// Parse a completed OSC payload into a notification, if the code is one we support.
fn parse_notification(code: u32, payload: &[u8]) -> Option<OscNotification> {
    let text = String::from_utf8_lossy(payload);
    match code {
        9 => Some(OscNotification { title: text.into_owned(), body: None, kind: OscKind::Simple9 }),
        99 => Some(parse_rich99(&text)),
        777 => parse_legacy777(&text),
        _ => None,
    }
}

/// Parse an OSC 99 payload: `;`-separated `k=v` segments where `p=` carries
/// `title:body`. If no `p=` segment exists, the whole payload is the title.
fn parse_rich99(payload: &str) -> OscNotification {
    for segment in payload.split(';') {
        if let Some(rest) = segment.strip_prefix("p=") {
            let (title, body) = match rest.split_once(':') {
                Some((title, body)) => (title.to_owned(), non_empty(body)),
                None => (rest.to_owned(), None),
            };
            return OscNotification { title, body, kind: OscKind::Rich99 };
        }
    }
    OscNotification { title: payload.to_owned(), body: None, kind: OscKind::Rich99 }
}

/// Parse an OSC 777 payload: `notify;Title;Body` (body optional). Any
/// subcommand other than `notify` produces no notification.
fn parse_legacy777(payload: &str) -> Option<OscNotification> {
    let mut parts = payload.splitn(3, ';');
    if parts.next()? != "notify" {
        return None;
    }
    let title = parts.next()?.to_owned();
    let body = parts.next().and_then(non_empty);
    Some(OscNotification { title, body, kind: OscKind::Legacy777 })
}

/// Convert a possibly-empty string slice into `Option<String>`.
fn non_empty(text: &str) -> Option<String> {
    if text.is_empty() { None } else { Some(text.to_owned()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(bytes: &[u8]) -> Vec<OscNotification> {
        OscScanner::new().feed(bytes)
    }

    #[test]
    fn test_osc9_bel_terminated() {
        let found = scan(b"\x1b]9;hello world\x07");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "hello world");
        assert_eq!(found[0].body, None);
        assert_eq!(found[0].kind, OscKind::Simple9);
    }

    #[test]
    fn test_osc9_st_terminated() {
        let found = scan(b"\x1b]9;hello world\x1b\\");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "hello world");
        assert_eq!(found[0].kind, OscKind::Simple9);
    }

    #[test]
    fn test_osc99_bel_terminated_with_title_and_body() {
        let found = scan(b"\x1b]99;i=1;e=1;d=0;p=Build:All tests passed\x07");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Build");
        assert_eq!(found[0].body.as_deref(), Some("All tests passed"));
        assert_eq!(found[0].kind, OscKind::Rich99);
    }

    #[test]
    fn test_osc99_st_terminated() {
        let found = scan(b"\x1b]99;p=Title:Body\x1b\\");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Title");
        assert_eq!(found[0].body.as_deref(), Some("Body"));
    }

    #[test]
    fn test_osc99_p_segment_without_body() {
        let found = scan(b"\x1b]99;i=1;p=OnlyTitle\x07");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "OnlyTitle");
        assert_eq!(found[0].body, None);
    }

    #[test]
    fn test_osc99_without_p_segment_uses_whole_payload_as_title() {
        let found = scan(b"\x1b]99;something happened\x07");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "something happened");
        assert_eq!(found[0].body, None);
        assert_eq!(found[0].kind, OscKind::Rich99);
    }

    #[test]
    fn test_osc777_notify_bel_terminated() {
        let found = scan(b"\x1b]777;notify;Title;Body text\x07");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Title");
        assert_eq!(found[0].body.as_deref(), Some("Body text"));
        assert_eq!(found[0].kind, OscKind::Legacy777);
    }

    #[test]
    fn test_osc777_notify_st_terminated_body_optional() {
        let found = scan(b"\x1b]777;notify;Just a title\x1b\\");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Just a title");
        assert_eq!(found[0].body, None);
    }

    #[test]
    fn test_osc777_non_notify_subcommand_ignored() {
        assert!(scan(b"\x1b]777;other;Title;Body\x07").is_empty());
    }

    #[test]
    fn test_other_osc_codes_produce_nothing() {
        assert!(scan(b"\x1b]0;window title\x07").is_empty());
        assert!(scan(b"\x1b]2;window title\x1b\\").is_empty());
        assert!(scan(b"\x1b]52;c;aGVsbG8=\x07").is_empty());
    }

    #[test]
    fn test_sequence_split_across_two_feeds() {
        let mut scanner = OscScanner::new();
        assert!(scanner.feed(b"\x1b]9;par").is_empty());
        let found = scanner.feed(b"tial message\x07");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "partial message");
    }

    #[test]
    fn test_sequence_split_across_three_feeds() {
        let mut scanner = OscScanner::new();
        assert!(scanner.feed(b"\x1b]77").is_empty());
        assert!(scanner.feed(b"7;notify;Ti").is_empty());
        let found = scanner.feed(b"tle;Body\x07");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Title");
        assert_eq!(found[0].body.as_deref(), Some("Body"));
    }

    #[test]
    fn test_st_terminator_split_across_chunk_boundary() {
        let mut scanner = OscScanner::new();
        assert!(scanner.feed(b"\x1b]9;split st\x1b").is_empty());
        let found = scanner.feed(b"\\");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "split st");
    }

    #[test]
    fn test_oversized_payload_discarded_without_panic() {
        let mut scanner = OscScanner::new();
        let mut bytes = b"\x1b]9;".to_vec();
        bytes.extend(std::iter::repeat_n(b'x', MAX_PAYLOAD + 100));
        bytes.push(BEL);
        assert!(scanner.feed(&bytes).is_empty());

        // Scanner recovers and parses the next sequence normally.
        let found = scanner.feed(b"\x1b]9;after\x07");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "after");
    }

    #[test]
    fn test_interrupting_escape_discards_and_resyncs() {
        // The first sequence is cut off by a new OSC start; only the
        // second completes.
        let found = scan(b"\x1b]9;discarded\x1b]9;kept\x07");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "kept");
    }

    #[test]
    fn test_interleaved_text_and_notification() {
        let found = scan(b"normal output\r\n\x1b]9;note\x07more output\r\n");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "note");
    }

    #[test]
    fn test_two_notifications_in_one_chunk() {
        let found = scan(b"\x1b]9;first\x07mid\x1b]777;notify;second\x1b\\");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].title, "first");
        assert_eq!(found[0].kind, OscKind::Simple9);
        assert_eq!(found[1].title, "second");
        assert_eq!(found[1].kind, OscKind::Legacy777);
    }

    #[test]
    fn test_garbage_bytes_do_not_panic_or_emit() {
        let garbage: Vec<u8> = (0..=255).cycle().take(10_000).collect();
        let mut scanner = OscScanner::new();
        // Whatever the garbage contains, the scanner must stay bounded
        // and never panic.
        let _ = scanner.feed(&garbage);
        assert!(scanner.buf.len() <= MAX_PAYLOAD);
    }

    #[test]
    fn test_invalid_utf8_payload_is_lossy_decoded() {
        let found = scan(b"\x1b]9;bad\xff\xfeutf8\x07");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "bad\u{fffd}\u{fffd}utf8");
    }
}
