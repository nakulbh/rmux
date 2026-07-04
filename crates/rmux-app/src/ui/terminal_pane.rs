//! Terminal pane widget.
//!
//! Wraps a PTY backend, terminal state, and renderer into
//! a self-contained egui widget that can be placed in split layouts.

use anyhow::Result;
use rmux_terminal::{
    InputMapper, OscNotification, OscScanner, PtyBackend, PtyError, TermState, TerminalRenderer,
};
use std::sync::mpsc;

/// The default font size for terminal text.
const DEFAULT_FONT_SIZE: f32 = 14.0;

/// A terminal pane that manages a PTY process and its rendering.
///
/// Each pane spawns a shell process, manages the PTY I/O via a
/// background reader thread, and renders the terminal grid using
/// the `alacritty_terminal`-based renderer.
///
/// # Examples
///
/// ```no_run
/// use rmux_app::ui::TerminalPane;
///
/// let mut pane = TerminalPane::spawn(80, 24).unwrap();
/// ```
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
    /// Scanner that detects notification OSC sequences in the PTY output.
    osc_scanner: OscScanner,
    /// Notifications parsed from the output, waiting to be collected.
    pending_notifications: Vec<OscNotification>,
    /// Whether this pane currently has keyboard focus.
    has_focus: bool,
    /// Whether to show the blinking cursor.
    show_cursor: bool,
    /// Display name (typically the shell name).
    name: String,
    /// Current column count.
    cols: u16,
    /// Current row count.
    rows: u16,
    /// Whether the underlying process has exited.
    exited: bool,
}

impl TerminalPane {
    /// Spawn a new terminal pane with a shell process.
    ///
    /// Creates a PTY, spawns the user's shell, starts a background
    /// reader thread for PTY output, and initializes the terminal
    /// emulator state and renderer.
    ///
    /// # Arguments
    ///
    /// * `cols` - Initial number of columns.
    /// * `rows` - Initial number of rows.
    ///
    /// # Errors
    ///
    /// Returns an error if the PTY could not be created or the shell
    /// could not be spawned.
    pub fn spawn(cols: u16, rows: u16) -> Result<Self, PtyError> {
        let mut backend = PtyBackend::spawn(cols, rows)?;
        let state = TermState::new(cols, rows, 10_000);
        let renderer = TerminalRenderer::new(DEFAULT_FONT_SIZE);
        let input_mapper = InputMapper::new();

        // Channel for PTY output from background thread
        let (tx, rx) = mpsc::channel::<Vec<u8>>();

        // Spawn background thread for reading PTY output
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
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            // No data available, sleep briefly to avoid busy loop
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        // Determine the display name
        let name = std::env::var("SHELL")
            .unwrap_or_else(|_| "/bin/sh".to_string())
            .split('/')
            .next_back()
            .unwrap_or("sh")
            .to_string();

        Ok(Self {
            backend,
            state,
            renderer,
            input_mapper,
            pty_rx: rx,
            osc_scanner: OscScanner::new(),
            pending_notifications: Vec::new(),
            has_focus: false,
            show_cursor: true,
            name,
            cols,
            rows,
            exited: false,
        })
    }

    /// Process any new PTY output from the background reader thread.
    ///
    /// Drains the channel and feeds bytes into the terminal state.
    /// Should be called once per frame before rendering.
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

    /// Take all notifications parsed from the PTY output since the last call.
    ///
    /// Returns them in arrival order and leaves the internal queue empty.
    pub fn take_notifications(&mut self) -> Vec<OscNotification> {
        std::mem::take(&mut self.pending_notifications)
    }

    /// Render the terminal pane in the egui UI.
    ///
    /// Draws the terminal grid, handles keyboard input when focused,
    /// and shows the cursor.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Process any new PTY output
        self.process_pty_output();

        // Determine available space and calculate terminal dimensions
        let available = ui.available_size();
        let (new_cols, new_rows) = self
            .renderer
            .cols_rows_for_rect(egui::Rect::from_min_size(egui::Pos2::ZERO, available));

        // Resize terminal if dimensions changed
        if new_cols != self.cols || new_rows != self.rows {
            self.cols = new_cols;
            self.rows = new_rows;
            self.state.resize(new_cols, new_rows);
            self.backend.resize(new_cols, new_rows).ok();
        }

        // Allocate the full available space for the terminal
        let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());

        // Track focus
        if response.clicked() {
            self.has_focus = true;
        }
        if response.clicked_elsewhere() {
            self.has_focus = false;
        }

        // Handle scroll wheel for terminal scrollback
        if response.hovered() {
            let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
            if scroll_delta.y != 0.0 {
                let lines = (-scroll_delta.y / self.renderer.cell_size().y) as i32;
                if lines != 0 {
                    self.state.scroll(lines);
                }
            }
        }

        // Handle keyboard input when focused
        if self.has_focus {
            self.handle_keyboard_input(ui);
        }

        // Toggle cursor blink
        self.show_cursor = (ui.input(|i| i.time) as u64 % 1000) < 500;

        // Take a snapshot of the terminal grid and render it
        let snapshot = self.state.snapshot();
        self.renderer.draw(ui, rect, &snapshot, self.show_cursor);

        // Show pane border when focused
        if self.has_focus {
            let painter = ui.painter();
            painter.rect_stroke(
                rect,
                egui::CornerRadius::ZERO,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 149, 237)),
                egui::StrokeKind::Middle,
            );
        }

        // Show exit status
        if self.exited {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("Process exited (code: {:?})", self.backend.try_wait()),
                egui::FontId::monospace(18.0),
                egui::Color32::from_rgb(255, 100, 100),
            );
        }
    }

    /// Handle keyboard input events when this pane is focused.
    fn handle_keyboard_input(&mut self, ui: &mut egui::Ui) {
        let events: Vec<egui::Event> = ui.input(|i| i.events.clone());

        for event in &events {
            if let egui::Event::Key { key, pressed, modifiers, .. } = event {
                if !pressed {
                    continue;
                }

                // Cmd-only chords (macOS) are reserved for app-level shortcuts
                // (split, close, new workspace, etc.) — never forward to the shell.
                // Physical Ctrl is still forwarded so Ctrl+C/Ctrl+D keep working.
                if modifiers.command && !modifiers.ctrl {
                    continue;
                }

                // Skip plain printable characters — Event::Text handles them.
                // Only handle special keys (Enter, Tab, arrows, F-keys, etc.)
                // and modified keys (Ctrl+A, Alt+char).
                let name = key.name();
                if name.len() == 1 && !modifiers.ctrl && !modifiers.alt {
                    continue;
                }

                let bytes = self.map_key_to_terminal(key, modifiers);
                if let Some(data) = bytes {
                    self.backend.write(&data).ok();
                }
            }

            // Handle text input (for actual character typing, paste, IME)
            if let egui::Event::Text(text) = event {
                for c in text.chars() {
                    let bytes = self.input_mapper.map_char(c, false, false);
                    if !bytes.is_empty() {
                        self.backend.write(&bytes).ok();
                    }
                }
            }
        }
    }

    /// Map an egui key event to terminal bytes.
    fn map_key_to_terminal(&self, key: &egui::Key, modifiers: &egui::Modifiers) -> Option<Vec<u8>> {
        use egui::Key;

        let ctrl = modifiers.ctrl;
        let _shift = modifiers.shift;
        let alt = modifiers.alt;

        match key {
            Key::Enter => Some(vec![b'\r']),
            Key::Tab => Some(vec![b'\t']),
            Key::Backspace => Some(vec![0x7f]),
            Key::Escape => Some(vec![0x1b]),
            Key::Delete => Some(vec![0x1b, b'[', b'3', b'~']),
            Key::Insert => Some(vec![0x1b, b'[', b'2', b'~']),
            Key::Home => Some(vec![0x1b, b'[', b'H']),
            Key::End => Some(vec![0x1b, b'[', b'F']),
            Key::PageUp => Some(vec![0x1b, b'[', b'5', b'~']),
            Key::PageDown => Some(vec![0x1b, b'[', b'6', b'~']),
            Key::ArrowUp => Some(vec![0x1b, b'[', b'A']),
            Key::ArrowDown => Some(vec![0x1b, b'[', b'B']),
            Key::ArrowRight => Some(vec![0x1b, b'[', b'C']),
            Key::ArrowLeft => Some(vec![0x1b, b'[', b'D']),
            Key::F1 => Some(vec![0x1b, b'O', b'P']),
            Key::F2 => Some(vec![0x1b, b'O', b'Q']),
            Key::F3 => Some(vec![0x1b, b'O', b'R']),
            Key::F4 => Some(vec![0x1b, b'O', b'S']),
            Key::F5 => Some(vec![0x1b, b'[', b'1', b'5', b'~']),
            Key::F6 => Some(vec![0x1b, b'[', b'1', b'7', b'~']),
            Key::F7 => Some(vec![0x1b, b'[', b'1', b'8', b'~']),
            Key::F8 => Some(vec![0x1b, b'[', b'1', b'9', b'~']),
            Key::F9 => Some(vec![0x1b, b'[', b'2', b'0', b'~']),
            Key::F10 => Some(vec![0x1b, b'[', b'2', b'1', b'~']),
            Key::F11 => Some(vec![0x1b, b'[', b'2', b'3', b'~']),
            Key::F12 => Some(vec![0x1b, b'[', b'2', b'4', b'~']),
            _ => {
                // Try to handle character keys with modifiers
                let name = key.name();
                if name.len() == 1 {
                    let c = name.chars().next().unwrap_or(' ');
                    if alt {
                        let mut result = vec![0x1b];
                        let mut buf = [0u8; 4];
                        let encoded = c.encode_utf8(&mut buf);
                        result.extend_from_slice(encoded.as_bytes());
                        Some(result)
                    } else if ctrl {
                        match c {
                            'a'..='z' => Some(vec![(c as u8) - b'a' + 1]),
                            'A'..='Z' => Some(vec![(c as u8) - b'A' + 1]),
                            _ => None,
                        }
                    } else {
                        Some(c.to_string().into_bytes())
                    }
                } else {
                    None
                }
            }
        }
    }

    /// Resize the terminal pane.
    #[allow(dead_code)]
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        self.state.resize(cols, rows);
        self.backend.resize(cols, rows).ok();
    }

    /// Get the display name of this pane.
    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether the underlying process has exited.
    #[allow(dead_code)]
    pub fn is_exited(&self) -> bool {
        self.exited
    }

    /// Whether this pane currently has keyboard focus.
    pub fn has_focus(&self) -> bool {
        self.has_focus
    }
}
