//! Input mapping for terminal emulation.
//!
//! Maps egui keyboard events to terminal escape sequences
//! that can be written to the PTY. Supports modifier keys,
//! special keys, and bracket paste mode.

/// Maps keyboard input to terminal escape sequences.
///
/// Translates egui key events into the byte sequences
/// that the PTY shell expects to receive.
///
/// # Examples
///
/// ```
/// use rmux_terminal::InputMapper;
///
/// let mapper = InputMapper::new();
/// let bytes = mapper.map_char('a', false, false);
/// assert_eq!(bytes, vec![b'a']);
/// ```
pub struct InputMapper {
    /// Whether bracket paste mode is active.
    bracket_paste_mode: bool,
}

impl InputMapper {
    /// Create a new input mapper.
    pub fn new() -> Self {
        Self { bracket_paste_mode: false }
    }

    /// Map a character input to terminal bytes.
    ///
    /// Handles alphanumeric keys and common special characters.
    /// Returns the byte sequence to write to the PTY.
    ///
    /// # Arguments
    ///
    /// * `c` - The character typed by the user.
    /// * `ctrl` - Whether the Ctrl modifier is held.
    /// * `alt` - Whether the Alt (Option) modifier is held.
    pub fn map_char(&self, c: char, ctrl: bool, alt: bool) -> Vec<u8> {
        match c {
            '\r' | '\n' => {
                // Enter key → carriage return
                vec![b'\r']
            }
            '\t' => {
                // Tab key
                vec![b'\t']
            }
            '\x08' | '\x7f' => {
                // Backspace
                vec![0x7f]
            }
            '\x1b' => {
                // Escape
                vec![0x1b]
            }
            _ => {
                if ctrl {
                    // Map Ctrl+letter to the corresponding control character
                    match c {
                        'a'..='z' => vec![(c as u8) - b'a' + 1],
                        'A'..='Z' => vec![(c as u8) - b'A' + 1],
                        '[' => vec![0x1b],       // Ctrl+[ = ESC
                        '\\' => vec![0x1c],      // Ctrl+\ = FS
                        ']' => vec![0x1d],       // Ctrl+] = GS
                        ' ' | '@' => vec![0x00], // Ctrl+Space/Ctrl+@ = NUL
                        _ => {
                            // Unknown Ctrl combo, send raw
                            let mut buf = [0u8; 4];
                            let encoded = c.encode_utf8(&mut buf);
                            encoded.as_bytes().to_vec()
                        }
                    }
                } else if alt {
                    // Alt+char → ESC + char
                    let mut buf = vec![0x1b];
                    let mut char_buf = [0u8; 4];
                    let encoded = c.encode_utf8(&mut char_buf);
                    buf.extend_from_slice(encoded.as_bytes());
                    buf
                } else {
                    let mut buf = [0u8; 4];
                    let encoded = c.encode_utf8(&mut buf);
                    encoded.as_bytes().to_vec()
                }
            }
        }
    }

    /// Map a named key (function, arrow, home, etc.) to terminal bytes.
    ///
    /// Returns `None` if the key is not recognized.
    pub fn map_named_key(&self, key_name: &str, _ctrl: bool, _shift: bool) -> Option<Vec<u8>> {
        match key_name {
            "ArrowUp" => Some(vec![0x1b, b'[', b'A']),
            "ArrowDown" => Some(vec![0x1b, b'[', b'B']),
            "ArrowRight" => Some(vec![0x1b, b'[', b'C']),
            "ArrowLeft" => Some(vec![0x1b, b'[', b'D']),
            "Home" => Some(vec![0x1b, b'[', b'H']),
            "End" => Some(vec![0x1b, b'[', b'F']),
            "Delete" => Some(vec![0x1b, b'[', b'3', b'~']),
            "Insert" => Some(vec![0x1b, b'[', b'2', b'~']),
            "PageUp" => Some(vec![0x1b, b'[', b'5', b'~']),
            "PageDown" => Some(vec![0x1b, b'[', b'6', b'~']),
            "F1" => Some(vec![0x1b, b'O', b'P']),
            "F2" => Some(vec![0x1b, b'O', b'Q']),
            "F3" => Some(vec![0x1b, b'O', b'R']),
            "F4" => Some(vec![0x1b, b'O', b'S']),
            "F5" => Some(vec![0x1b, b'[', b'1', b'5', b'~']),
            "F6" => Some(vec![0x1b, b'[', b'1', b'7', b'~']),
            "F7" => Some(vec![0x1b, b'[', b'1', b'8', b'~']),
            "F8" => Some(vec![0x1b, b'[', b'1', b'9', b'~']),
            "F9" => Some(vec![0x1b, b'[', b'2', b'0', b'~']),
            "F10" => Some(vec![0x1b, b'[', b'2', b'1', b'~']),
            "F11" => Some(vec![0x1b, b'[', b'2', b'3', b'~']),
            "F12" => Some(vec![0x1b, b'[', b'2', b'4', b'~']),
            _ => None,
        }
    }

    /// Wrap text in bracket paste escape sequences.
    ///
    /// This tells the terminal that the following text is a paste,
    /// which enables the shell to handle it differently (e.g., prevent
    /// executing commands on paste).
    pub fn wrap_paste(&self, text: &str) -> Vec<u8> {
        if self.bracket_paste_mode {
            let mut result = vec![0x1b, b'[', b'2', b'0', b'0', b'~'];
            result.extend_from_slice(text.as_bytes());
            result.extend_from_slice(&[0x1b, b'[', b'2', b'0', b'1', b'~']);
            result
        } else {
            text.as_bytes().to_vec()
        }
    }

    /// Set bracket paste mode state.
    pub fn set_bracket_paste_mode(&mut self, enabled: bool) {
        self.bracket_paste_mode = enabled;
    }
}

impl Default for InputMapper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_char_basic() {
        let mapper = InputMapper::new();

        assert_eq!(mapper.map_char('a', false, false), vec![b'a']);
        assert_eq!(mapper.map_char('A', false, false), vec![b'A']);
        assert_eq!(mapper.map_char('1', false, false), vec![b'1']);
    }

    #[test]
    fn test_map_char_enter() {
        let mapper = InputMapper::new();
        assert_eq!(mapper.map_char('\r', false, false), vec![b'\r']);
    }

    #[test]
    fn test_map_char_tab() {
        let mapper = InputMapper::new();
        assert_eq!(mapper.map_char('\t', false, false), vec![b'\t']);
    }

    #[test]
    fn test_map_char_backspace() {
        let mapper = InputMapper::new();
        assert_eq!(mapper.map_char('\x08', false, false), vec![0x7f]);
        assert_eq!(mapper.map_char('\x7f', false, false), vec![0x7f]);
    }

    #[test]
    fn test_map_char_ctrl_c() {
        let mapper = InputMapper::new();
        // Ctrl+C should send 0x03
        assert_eq!(mapper.map_char('c', true, false), vec![0x03]);
    }

    #[test]
    fn test_map_char_ctrl_d() {
        let mapper = InputMapper::new();
        assert_eq!(mapper.map_char('d', true, false), vec![0x04]);
    }

    #[test]
    fn test_map_char_alt() {
        let mapper = InputMapper::new();
        // Alt+X → ESC + x
        assert_eq!(mapper.map_char('x', false, true), vec![0x1b, b'x']);
    }

    #[test]
    fn test_map_named_key_arrows() {
        let mapper = InputMapper::new();
        assert_eq!(mapper.map_named_key("ArrowUp", false, false), Some(vec![0x1b, b'[', b'A']));
        assert_eq!(mapper.map_named_key("ArrowDown", false, false), Some(vec![0x1b, b'[', b'B']));
    }

    #[test]
    fn test_map_named_key_unknown() {
        let mapper = InputMapper::new();
        assert_eq!(mapper.map_named_key("UnknownKey", false, false), None);
    }

    #[test]
    fn test_wrap_paste() {
        let mapper = InputMapper::new();
        let result = mapper.wrap_paste("hello");
        assert_eq!(result, b"hello".to_vec());
    }

    #[test]
    fn test_wrap_paste_bracket_mode() {
        let mut mapper = InputMapper::new();
        mapper.set_bracket_paste_mode(true);
        let result = mapper.wrap_paste("hello");
        let expected: Vec<u8> = {
            let mut v = vec![0x1b, b'[', b'2', b'0', b'0', b'~'];
            v.extend_from_slice(b"hello");
            v.extend_from_slice(&[0x1b, b'[', b'2', b'0', b'1', b'~']);
            v
        };
        assert_eq!(result, expected);
    }
}
