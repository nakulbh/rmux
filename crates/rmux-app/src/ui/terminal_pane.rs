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
pub const DEFAULT_FONT_SIZE: f32 = 14.0;

/// Height of the find bar in pixels.
const FIND_BAR_HEIGHT: f32 = 28.0;

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
/// let mut pane = TerminalPane::spawn(80, 24, 14.0).unwrap();
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

    // Find bar state
    /// Whether the find/search bar is currently visible.
    find_visible: bool,
    /// The current search query string.
    find_query: String,
    /// List of find match positions as (row, col) in snapshot coordinates.
    find_results: Vec<(usize, usize)>,
    /// Index into `find_results` for the currently highlighted match.
    find_index: usize,
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
    /// * `font_size` - Font size for the terminal renderer.
    ///
    /// # Errors
    ///
    /// Returns an error if the PTY could not be created or the shell
    /// could not be spawned.
    pub fn spawn(cols: u16, rows: u16, font_size: f32) -> Result<Self, PtyError> {
        let mut backend = PtyBackend::spawn(cols, rows)?;
        let state = TermState::new(cols, rows, 10_000);
        let renderer = TerminalRenderer::new(font_size);
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
            find_visible: false,
            find_query: String::new(),
            find_results: Vec::new(),
            find_index: 0,
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

    /// Write raw text to the pane's PTY as if it had been typed.
    ///
    /// The text is sent verbatim — no escape interpretation is performed.
    /// Write failures are logged and swallowed (best-effort, like typing).
    pub fn send_text(&mut self, text: &str) {
        if let Err(err) = self.backend.write(text.as_bytes()) {
            tracing::warn!(error = %err, "failed to write text to PTY");
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
    /// and shows the cursor. When the find bar is active, it appears
    /// at the bottom of the pane.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Process any new PTY output
        self.process_pty_output();

        // Determine available space, reserving room for find bar if visible
        let available = ui.available_size();
        let find_bar_space =
            if self.find_visible { egui::vec2(0.0, FIND_BAR_HEIGHT) } else { egui::Vec2::ZERO };
        let terminal_available = available - find_bar_space;

        // Calculate terminal dimensions
        let (new_cols, new_rows) = self
            .renderer
            .cols_rows_for_rect(egui::Rect::from_min_size(egui::Pos2::ZERO, terminal_available));

        // Resize terminal if dimensions changed
        if new_cols != self.cols || new_rows != self.rows {
            self.cols = new_cols;
            self.rows = new_rows;
            self.state.resize(new_cols, new_rows);
            self.backend.resize(new_cols, new_rows).ok();
        }

        // Allocate space for the terminal
        let (rect, _response) =
            ui.allocate_exact_size(terminal_available, egui::Sense::click_and_drag());

        // Track focus from the terminal area response
        let term_response = ui.interact(rect, ui.id(), egui::Sense::click_and_drag());
        if term_response.clicked() {
            self.has_focus = true;
        }
        if term_response.clicked_elsewhere() {
            self.has_focus = false;
        }

        // Handle scroll wheel for terminal scrollback
        if term_response.hovered() {
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

        // Highlight find matches in the terminal
        if self.find_visible && !self.find_query.is_empty() {
            self.highlight_matches(ui, rect, &snapshot);
        }

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

        // Render find bar if visible
        if self.find_visible {
            self.show_find_bar(ui, rect.x_range().min, rect.bottom());
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

                // On non-macOS, specific Ctrl chords are reserved for app shortcuts.
                // On macOS, Ctrl is for terminal control characters (Ctrl+C=SIGINT,
                // Ctrl+D=EOF, etc.) and must always be forwarded to the shell.
                if !cfg!(target_os = "macos")
                    && modifiers.ctrl
                    && !modifiers.command
                    && self.is_reserved_app_key(key)
                {
                    continue;
                }

                // When find bar is visible, Escape and Enter are handled by
                // shortcuts.rs (close find bar / find next) — don't forward to terminal.
                if self.find_visible
                    && !modifiers.command
                    && !modifiers.ctrl
                    && !modifiers.alt
                    && !modifiers.shift
                    && matches!(key, egui::Key::Escape | egui::Key::Enter)
                {
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

    /// Check if a key is claimed by an app-level shortcut.
    ///
    /// On non-macOS platforms, specific Ctrl chords are reserved for app
    /// shortcuts (split, close, new workspace, etc.) and should not be
    /// forwarded to the terminal shell. On macOS, Ctrl is always forwarded
    /// (the terminal uses Ctrl for control characters, app shortcuts use Cmd).
    fn is_reserved_app_key(&self, key: &egui::Key) -> bool {
        match key {
            egui::Key::B
            | egui::Key::D
            | egui::Key::E
            | egui::Key::F
            | egui::Key::G
            | egui::Key::K
            | egui::Key::N
            | egui::Key::Q
            | egui::Key::W
            | egui::Key::Plus
            | egui::Key::Equals
            | egui::Key::Minus
            | egui::Key::Num0
            | egui::Key::Num1
            | egui::Key::Num2
            | egui::Key::Num3
            | egui::Key::Num4
            | egui::Key::Num5
            | egui::Key::Num6
            | egui::Key::Num7
            | egui::Key::Num8
            | egui::Key::Num9 => true,
            // Ctrl+C is only reserved when there's a text selection to copy
            egui::Key::C => self.state.copy_selected_text().is_some(),
            _ => false,
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

    /// Change the font size used by the terminal renderer.
    ///
    /// The cell grid is recalculated on the next render frame when the
    /// available pixel area is known.
    pub fn set_font_size(&mut self, font_size: f32) {
        self.renderer.set_font_size(font_size);
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

impl TerminalPane {
    /// If the terminal has an active text selection, return the selected text.
    ///
    /// Delegates to [`TermState::copy_selected_text`].
    pub fn copy_selection(&self) -> Option<String> {
        self.state.copy_selected_text()
    }

    /// Get the current font size used by the renderer.
    #[allow(dead_code)]
    pub fn font_size(&self) -> f32 {
        self.renderer.font_size
    }

    /// Clear the terminal scrollback buffer.
    ///
    /// Public entry point for the app-level shortcut (`Cmd/Ctrl+K`).
    pub fn clear_scrollback(&mut self) {
        self.state.clear_scrollback();
    }

    /// Whether the find/search bar is currently visible.
    pub fn is_find_visible(&self) -> bool {
        self.find_visible
    }

    /// Close the find bar and clear search state.
    ///
    /// Public entry point for the Escape key shortcut.
    pub fn close_find_bar(&mut self) {
        self.find_visible = false;
        self.find_query.clear();
        self.find_results.clear();
        self.find_index = 0;
    }

    /// Open the find bar (if not already open) and populate with the
    /// current terminal text selection.
    ///
    /// Public entry point for the `Cmd/Ctrl+E` shortcut.
    pub fn find_with_selection(&mut self) {
        if !self.find_visible {
            self.find_visible = true;
        }
        if let Some(sel) = self.state.copy_selected_text() {
            self.find_query = sel;
            self.perform_find();
        }
    }

    /// Toggle the find/search bar visibility.
    ///
    /// When toggling off, clears the find state. When toggling on,
    /// pre-populates with the current terminal selection if available.
    pub fn toggle_find(&mut self) {
        if self.find_visible {
            self.find_visible = false;
            self.find_query.clear();
            self.find_results.clear();
            self.find_index = 0;
        } else {
            self.find_visible = true;
            // If there's a selection, use it as the initial query
            if let Some(sel) = self.state.copy_selected_text() {
                self.find_query = sel;
                self.perform_find();
            }
        }
    }

    /// Search the visible terminal grid for all occurrences of `find_query`.
    ///
    /// Results are stored in `find_results` as `(row, col)` pairs
    /// in snapshot coordinates. The first match becomes the active one.
    fn perform_find(&mut self) {
        self.find_results.clear();
        self.find_index = 0;

        if self.find_query.is_empty() {
            return;
        }

        let snapshot = self.state.snapshot();
        let query_lower = self.find_query.to_lowercase();

        for row in 0..snapshot.rows as usize {
            let row_text: String = snapshot.cells[row].iter().map(|c| c.c).collect();
            let row_lower = row_text.to_lowercase();

            // Find all matches in this row
            let mut search_start = 0;
            while let Some(pos) = row_lower[search_start..].find(&query_lower) {
                let col = search_start + pos;
                self.find_results.push((row, col));
                search_start = col + 1;
            }
        }
    }

    /// Move to the next find match (wraps around).
    pub fn find_next_match(&mut self) {
        if self.find_results.is_empty() {
            // Re-run search if results are empty but query is non-empty
            if !self.find_query.is_empty() {
                self.perform_find();
            }
            return;
        }
        self.find_index = (self.find_index + 1) % self.find_results.len();
    }

    /// Move to the previous find match (wraps around).
    pub fn find_prev_match(&mut self) {
        if self.find_results.is_empty() {
            if !self.find_query.is_empty() {
                self.perform_find();
            }
            return;
        }
        if self.find_index == 0 {
            self.find_index = self.find_results.len() - 1;
        } else {
            self.find_index -= 1;
        }
    }

    /// Highlight find matches in the terminal viewport.
    ///
    /// Draws a colored background overlay for all matched cells,
    /// with a different color for the currently active match.
    fn highlight_matches(
        &self,
        ui: &mut egui::Ui,
        term_rect: egui::Rect,
        snapshot: &rmux_terminal::GridSnapshot,
    ) {
        if self.find_results.is_empty() {
            return;
        }

        let cell_size = self.renderer.cell_size();
        let painter = ui.painter();

        let match_bg = egui::Color32::from_rgba_premultiplied(255, 200, 0, 80);
        let active_bg = egui::Color32::from_rgba_premultiplied(255, 140, 0, 140);

        for (i, &(row, col)) in self.find_results.iter().enumerate() {
            if row >= snapshot.rows as usize || col >= snapshot.cols as usize {
                continue;
            }

            let is_active = i == self.find_index;

            // Calculate match cell position
            let x = term_rect.left() + col as f32 * cell_size.x;
            let y = term_rect.top() + row as f32 * cell_size.y;

            // Calculate match width (number of consecutive chars matching the query)
            let query_len = self.find_query.len();
            let match_width = (query_len as f32 * cell_size.x).min(term_rect.right() - x);

            let highlight_rect = egui::Rect::from_min_size(
                egui::Pos2::new(x, y),
                egui::Vec2::new(match_width, cell_size.y),
            );

            let color = if is_active { active_bg } else { match_bg };
            painter.rect_filled(highlight_rect, 0.0, color);
        }
    }

    /// Render the find bar at the bottom of the terminal pane.
    fn show_find_bar(&mut self, ui: &mut egui::Ui, x: f32, y: f32) {
        let available_width = ui.available_width();

        let bar_rect = egui::Rect::from_min_size(
            egui::Pos2::new(x, y),
            egui::Vec2::new(available_width, FIND_BAR_HEIGHT),
        );

        // Allocate space for the find bar
        let mut bar_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(bar_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        // Background
        bar_ui.painter().rect_filled(bar_rect, 0.0, egui::Color32::from_rgb(40, 44, 52));

        // Spacing
        bar_ui.add_space(8.0);

        // Query text input
        let text_response = bar_ui.add(
            egui::TextEdit::singleline(&mut self.find_query)
                .hint_text("Find...")
                .font(egui::FontId::monospace(13.0))
                .desired_width(200.0),
        );

        if text_response.changed() {
            self.perform_find();
        }

        // Match count label
        let count_text = if self.find_results.is_empty() && !self.find_query.is_empty() {
            "No matches".to_string()
        } else if self.find_results.is_empty() {
            String::new()
        } else {
            format!("{}/{}", self.find_index + 1, self.find_results.len())
        };

        if !count_text.is_empty() {
            bar_ui.label(
                egui::RichText::new(count_text)
                    .font(egui::FontId::monospace(12.0))
                    .color(egui::Color32::from_rgb(180, 180, 190)),
            );
        }

        bar_ui.add_space(8.0);

        // Previous match button
        if bar_ui.add(egui::Button::new("◀")).clicked() && !self.find_results.is_empty() {
            if self.find_index == 0 {
                self.find_index = self.find_results.len() - 1;
            } else {
                self.find_index -= 1;
            }
        }

        // Next match button
        if bar_ui.add(egui::Button::new("▶")).clicked() {
            self.find_next_match();
        }

        bar_ui.add_space(8.0);

        // Close button
        if bar_ui.add(egui::Button::new("✕")).clicked() {
            self.find_visible = false;
            self.find_query.clear();
            self.find_results.clear();
            self.find_index = 0;
        }
    }
}
