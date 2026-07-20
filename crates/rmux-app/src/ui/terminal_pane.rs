//! Terminal pane widget.
//!
//! Wraps a PTY backend, terminal state, and renderer into
//! a self-contained egui widget that can be placed in split layouts.

use anyhow::Result;
use image::ImageEncoder;
use image::codecs::png::PngEncoder;
use rmux_terminal::{InputMapper, PtyBackend, PtyError, TermState, TerminalRenderer};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use crate::ui::theme;

/// The default font size for terminal text.
pub const DEFAULT_FONT_SIZE: f32 = 14.0;

/// Height of the find bar in pixels.
const FIND_BAR_HEIGHT: f32 = 28.0;

/// Appended to the pane title when [`TerminalPane::is_copy_mode`] is true.
pub const COPY_MODE_INDICATOR: &str = " [COPY]";

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
    /// Human-readable exit banner (set once when the process is reaped).
    /// Kept separate from Debug so we never paint `ExitStatus { … }` tofu.
    exit_message: Option<String>,
    /// Whether the process exited successfully (code 0, no signal).
    exit_success: bool,
    /// Cached shell cwd for tab titles (refreshed periodically; avoids
    /// calling `lsof` / reading `/proc` every frame).
    last_cwd: Option<PathBuf>,
    /// Frame counter used to throttle cwd / process-title refreshes.
    cwd_tick: u16,
    /// Cached foreground process title (`cargo run …`), when the shell is busy.
    last_fg_title: Option<String>,
    /// Cached git branch for the shell cwd (idle workspace title).
    last_git_branch: Option<String>,
    /// Cached PR chip for the shell cwd (throttled; cmux pull-request row).
    last_pull_request: Option<crate::workspace::sidebar_snapshot::PullRequestDisplay>,
    /// Frames since last PR probe (PR probe is slower than cwd/`ps`).
    pr_tick: u16,

    // Find bar state
    /// Whether the find/search bar is currently visible.
    find_visible: bool,
    /// The current search query string.
    find_query: String,
    /// List of find match positions as (row, col) in snapshot coordinates.
    find_results: Vec<(usize, usize)>,
    /// Index into `find_results` for the currently highlighted match.
    find_index: usize,

    /// Whether the pane is currently in copy mode (cmux `Cmd+Shift+M`).
    /// The flag is the only state for now — actual copy-mode behaviour
    /// (vim-style scrollback nav, selection) is out of scope and will
    /// be wired up in a later wave. The flag alone is enough to drive
    /// the title-bar indicator and to give the dispatcher a hook.
    copy_mode: bool,

    // Dimension overlay state
    /// Whether the "cols×rows" dimension overlay is currently visible.
    dimension_overlay_visible: bool,
    /// Timestamp (in seconds, from `ui.input(|i| i.time)`) when the overlay
    /// was last shown. Used to fade the overlay out after 2 seconds.
    dimension_overlay_timer: f64,

    /// Incremented on every successful [`Self::try_paste_image`] call, used
    /// to give each pasted image's temp file a unique name so a second
    /// paste doesn't overwrite (and thus corrupt) the file a CLI tool may
    /// still be reading from the first paste.
    paste_counter: u64,
}

impl TerminalPane {
    /// Spawn a new terminal pane with a shell process (default `$HOME` cwd).
    #[allow(dead_code)] // convenience wrapper; call sites use `spawn_with_cwd`
    pub fn spawn(cols: u16, rows: u16, font_size: f32) -> Result<Self, PtyError> {
        Self::spawn_with_cwd(cols, rows, font_size, None)
    }

    /// Spawn a terminal whose shell starts in `cwd` when provided.
    ///
    /// Used by splits and Cmd+T so new shells open in the same directory
    /// the focused terminal is currently in.
    pub fn spawn_with_cwd(
        cols: u16,
        rows: u16,
        font_size: f32,
        cwd: Option<&Path>,
    ) -> Result<Self, PtyError> {
        let mut backend = PtyBackend::spawn_with_cwd(cols, rows, cwd)?;
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
            has_focus: false,
            show_cursor: true,
            name,
            cols,
            rows,
            exited: false,
            exit_message: None,
            exit_success: false,
            // Prefer the spawn cwd so tab labels are correct before the first
            // process-query refresh.
            last_cwd: cwd.map(Path::to_path_buf),
            cwd_tick: 0,
            last_fg_title: None,
            last_git_branch: None,
            last_pull_request: None,
            pr_tick: 0,
            find_visible: false,
            find_query: String::new(),
            find_results: Vec::new(),
            find_index: 0,
            copy_mode: false,
            dimension_overlay_visible: false,
            dimension_overlay_timer: 0.0_f64,
            paste_counter: 0,
        })
    }

    /// Process any new PTY output from the background reader thread.
    ///
    /// Drains the channel and feeds bytes into the terminal state.
    /// Should be called once per frame before rendering.
    pub fn process_pty_output(&mut self) {
        while let Ok(data) = self.pty_rx.try_recv() {
            // OSC notification scanning is intentionally disabled: OSC 9 is also
            // used by iTerm2 progress bars (`OSC 9;4;…`), which produced junk
            // entries like "4;0;" in the notification panel. Re-enable only with
            // a tighter parser that rejects progress / non-notify sequences.
            self.state.feed_bytes(&data);
        }

        // Check if the PTY process has exited; record a clean banner once.
        if !self.exited
            && let Some(status) = self.backend.try_wait()
        {
            self.exited = true;
            self.exit_success = status.success();
            self.exit_message = Some(if status.success() {
                "Process exited".to_owned()
            } else {
                // portable_pty::ExitStatus Display: "Exited with code N"
                // or "Terminated by SIGINT", etc.
                format!("Process exited · {status}")
            });
            if !self.name.ends_with(" [exited]") {
                self.name.push_str(" [exited]");
            }
        }

        // Refresh cwd / process title / git branch ~every 45 frames (~0.7s at
        // 60fps) for tab labels and cmux-style workspace auto-titles.
        self.cwd_tick = self.cwd_tick.wrapping_add(1);
        if self.cwd_tick.is_multiple_of(45) {
            self.refresh_title_sources();
        } else if self.last_cwd.is_none() {
            // First chance: populate immediately so new tabs aren't empty.
            self.refresh_title_sources();
        }
    }

    /// Probe cwd, foreground process, and git branch (throttled by caller).
    fn refresh_title_sources(&mut self) {
        let mut cwd_changed = false;
        if let Some(cwd) = self.backend.working_directory() {
            cwd_changed = self.last_cwd.as_ref() != Some(&cwd);
            self.last_cwd = Some(cwd);
            if (cwd_changed || self.last_git_branch.is_none())
                && let Some(ref path) = self.last_cwd
            {
                self.last_git_branch = crate::workspace::title::git_branch_for_cwd(path);
            }
        } else if self.last_cwd.is_none() {
            self.last_cwd = self.backend.working_directory();
        }

        self.last_fg_title = self.backend.foreground_process_title();

        // PR probe ~every 180 frames (~3s) or when cwd changes — `gh` is slow.
        self.pr_tick = self.pr_tick.wrapping_add(1);
        if cwd_changed || self.pr_tick.is_multiple_of(180) {
            if let Some(ref path) = self.last_cwd {
                self.last_pull_request =
                    crate::workspace::sidebar_snapshot::pull_request_for_cwd(path);
            } else {
                self.last_pull_request = None;
            }
        }
    }

    /// Best-effort current working directory of this pane's shell.
    ///
    /// Used when spawning a sibling tab/split so the new shell opens in
    /// the same directory the user has already navigated to. Prefers the
    /// cached value, falling back to a live query.
    pub fn working_directory(&self) -> Option<PathBuf> {
        self.last_cwd.clone().or_else(|| self.backend.working_directory())
    }

    /// Cached cwd (no live process query). For tab labels during paint.
    pub fn cached_cwd(&self) -> Option<&Path> {
        self.last_cwd.as_deref()
    }

    /// cmux-style tab label: short path (`~/…/project`) or `user@host` at `$HOME`.
    pub fn tab_label(&self) -> String {
        if let Some(cwd) = self.cached_cwd() {
            return format_cwd_tab_title(cwd);
        }
        user_host_title()
    }

    /// cmux-style workspace / process title for the sidebar.
    ///
    /// Prefers a running non-shell command; otherwise `{branch} · {path}`.
    pub fn auto_workspace_title(&self) -> String {
        crate::workspace::title::compose_auto_title(
            self.last_fg_title.as_deref(),
            self.cached_cwd(),
            self.last_git_branch.as_deref(),
        )
    }

    /// Cached git branch for the shell cwd, if known.
    pub fn cached_git_branch(&self) -> Option<&str> {
        self.last_git_branch.as_deref()
    }

    /// Cached foreground process title, if the shell is busy.
    pub fn cached_fg_title(&self) -> Option<&str> {
        self.last_fg_title.as_deref()
    }

    /// Cached PR for the shell cwd, if `gh` reported one.
    pub fn cached_pull_request(
        &self,
    ) -> Option<&crate::workspace::sidebar_snapshot::PullRequestDisplay> {
        self.last_pull_request.as_ref()
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
            if self.find_visible { egui::vec2(0.0_f32, FIND_BAR_HEIGHT) } else { egui::Vec2::ZERO };
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
            if self.has_focus {
                self.dimension_overlay_visible = true;
                self.dimension_overlay_timer = ui.input(|i| i.time);
            }
        }

        // Allocate space for the terminal
        let (rect, term_response) =
            ui.allocate_exact_size(terminal_available, egui::Sense::click_and_drag());

        // Track focus from the terminal area response
        if term_response.clicked() || term_response.drag_started() {
            self.has_focus = true;
        }
        if term_response.clicked_elsewhere() {
            self.has_focus = false;
        }

        // Mouse selection: drag selects cells; double-click word; triple line.
        self.handle_mouse_selection(ui, rect, &term_response);

        // Handle scroll wheel for terminal scrollback
        if term_response.hovered() {
            let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
            if scroll_delta.y != 0.0_f32 {
                let lines = (-scroll_delta.y / self.renderer.cell_size().y) as i32;
                if lines != 0 {
                    self.state.scroll(lines);
                }
            }
        }

        // Keyboard: active terminal claims egui focus so Tab is not stolen for
        // widget navigation (which used to silence all subsequent keystrokes —
        // see `text_sink`). Yield only to real TextEdits (rename / find / URL).
        //
        // When the find bar is open we always yield — otherwise `request_focus`
        // would fight the find TextEdit every frame.
        let text_sink = crate::ui::text_sink::is_active(ui.ctx()) || self.find_visible;
        if self.has_focus && !text_sink {
            // Lock Tab / arrows / Escape onto this surface so shell autocomplete
            // and readline keep working like Ghostty/cmux.
            term_response.request_focus();
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    term_response.id,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                );
            });
            self.handle_keyboard_input(ui);
        }

        // Toggle cursor blink
        self.show_cursor = (ui.input(|i| i.time) as u64 % 1000) < 500;

        self.show_title_bar(ui, rect);

        // Take a snapshot of the terminal grid and render it
        let snapshot = self.state.snapshot();
        self.renderer.draw(ui, rect, &snapshot, self.show_cursor);

        // Highlight find matches in the terminal
        if self.find_visible && !self.find_query.is_empty() {
            self.highlight_matches(ui, rect, &snapshot);
        }

        // Focus is indicated by dimming *inactive* panes in the workspace
        // view (cmux-style) — no accent glow border here.

        // Render find bar if visible
        if self.find_visible {
            self.show_find_bar(ui, rect.x_range().min, rect.bottom());
        }

        // Exit banner — short human text, never Debug of ExitStatus.
        if self.exited {
            let palette = theme::palette();
            let msg = self.exit_message.as_deref().unwrap_or("Process exited");
            let color = if self.exit_success { palette.text_muted } else { palette.danger };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                msg,
                egui::FontId::proportional(13.0_f32),
                color,
            );
        }

        // Render dimension overlay if visible and still within 2s window
        if self.dimension_overlay_visible && self.has_focus {
            let now = ui.input(|i| i.time);
            if now - self.dimension_overlay_timer < 2.0_f64 {
                let palette = theme::palette();
                let painter = ui.painter();
                let text = format!("{}\u{00d7}{}", self.cols, self.rows);
                let font = egui::FontId::monospace(10.0_f32);

                let galley =
                    painter.layout_no_wrap(text.clone(), font.clone(), palette.text_disabled);
                let pad = 2.0_f32;
                let badge_size =
                    egui::vec2(galley.size().x + pad * 2.0_f32, galley.size().y + pad * 2.0_f32);
                let badge_min = egui::Pos2::new(
                    rect.right() - badge_size.x - 4.0_f32,
                    rect.bottom() - badge_size.y - 4.0_f32,
                );
                let badge_rect = egui::Rect::from_min_size(badge_min, badge_size);

                painter.rect_filled(badge_rect, egui::CornerRadius::same(2), palette.panel_bg);
                painter.text(
                    badge_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    font,
                    palette.text_disabled,
                );
            } else {
                self.dimension_overlay_visible = false;
            }
        }
    }

    /// Render the pane title in the top-left corner of the terminal area.
    ///
    /// Drawn before the terminal snapshot so it sits as chrome on top
    /// of the grid (no row is consumed). The text is `self.name` plus
    /// [`COPY_MODE_INDICATOR`] when `self.copy_mode` is true. The
    /// string concatenation is the single point that flips on
    /// `copy_mode` — keep it that way so a future change to the
    /// indicator or the trigger condition is a one-line edit.
    fn show_title_bar(&self, ui: &egui::Ui, rect: egui::Rect) {
        let title = if self.copy_mode {
            format!("{}{}", self.name, COPY_MODE_INDICATOR)
        } else {
            self.name.clone()
        };
        let palette = theme::palette();
        let font = egui::FontId::monospace(10.0_f32);
        let galley = ui.painter().layout_no_wrap(title.clone(), font.clone(), palette.text_muted);
        let pad = 2.0_f32;
        let badge_size =
            egui::vec2(galley.size().x + pad * 2.0_f32, galley.size().y + pad * 2.0_f32);
        let badge_min = egui::Pos2::new(rect.left() + 4.0_f32, rect.top() + 4.0_f32);
        let badge_rect = egui::Rect::from_min_size(badge_min, badge_size);
        let painter = ui.painter();
        painter.rect_filled(badge_rect, egui::CornerRadius::same(2), palette.panel_bg);
        painter.text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            title,
            font,
            palette.text_muted,
        );
    }

    /// Handle keyboard input events when this pane is focused.
    fn handle_keyboard_input(&mut self, ui: &mut egui::Ui) {
        let events: Vec<egui::Event> = ui.input(|i| i.events.clone());

        for event in &events {
            // egui-winit intercepts ⌘/Ctrl+C/V and emits Copy/Paste events
            // instead of Key events — handle those first.
            match event {
                egui::Event::Copy => {
                    self.handle_copy_event(ui.ctx());
                    continue;
                }
                egui::Event::Paste(text) => {
                    self.paste_text(text);
                    continue;
                }
                egui::Event::Cut => {
                    // Terminals don't cut grid cells; treat as copy if selected.
                    self.handle_copy_event(ui.ctx());
                    continue;
                }
                _ => {}
            }

            if let egui::Event::Key { key, pressed, modifiers, .. } = event {
                if !pressed {
                    continue;
                }

                // App shortcuts use egui's logical `command` modifier (⌘ on
                // macOS, Ctrl on Linux/Windows). The ShortcutManager consumes
                // those chords before this UI runs; any remaining `command`
                // events that still match reserved app keys must not reach
                // the PTY (Linux previously failed here because it required
                // `ctrl && !command`, but egui sets BOTH bits for Ctrl).
                //
                // Physical Ctrl without `command` only exists on macOS and is
                // forwarded for terminal control characters (SIGINT, EOF, …).
                if modifiers.command && self.is_reserved_app_key(key) {
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
                    // Typing clears the visual selection (standard terminal UX).
                    self.state.clear_selection();
                    self.backend.write(&data).ok();
                }
            }

            // Handle text input (for actual character typing, IME)
            if let egui::Event::Text(text) = event {
                // Typing replaces the selection (clear highlight, then type).
                self.state.clear_selection();
                for c in text.chars() {
                    let bytes = self.input_mapper.map_char(c, false, false);
                    if !bytes.is_empty() {
                        self.backend.write(&bytes).ok();
                    }
                }
            }
        }
    }

    /// Handle `egui::Event::Copy` (⌘/Ctrl+C from egui-winit).
    ///
    /// With a selection: put text on the system clipboard.
    /// Without: forward SIGINT (`^C`) so shells still work on Linux/Windows
    /// where Ctrl sets both `ctrl` and `command` and egui treats Ctrl+C as copy.
    fn handle_copy_event(&mut self, ctx: &egui::Context) {
        if let Some(text) = self.state.copy_selected_text() {
            ctx.copy_text(text);
            tracing::debug!("Copied terminal selection via Event::Copy");
        } else {
            // No selection → interrupt the foreground process.
            let _ = self.backend.write(&[0x03]);
            tracing::debug!("No selection; sent SIGINT (^C) to PTY");
        }
    }

    /// Drag / multi-click mouse selection over the grid.
    fn handle_mouse_selection(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        response: &egui::Response,
    ) {
        let cell = self.renderer.cell_size();
        if cell.x <= 0.0 || cell.y <= 0.0 {
            return;
        }

        let pointer_cell = |pos: egui::Pos2| -> (u16, u16) {
            let local = pos - rect.min;
            let col = (local.x / cell.x).floor().max(0.0) as u16;
            let row = (local.y / cell.y).floor().max(0.0) as u16;
            let col = col.min(self.cols.saturating_sub(1));
            let row = row.min(self.rows.saturating_sub(1));
            (col, row)
        };

        // Triple-click → full line; double-click → word; otherwise simple drag.
        if response.triple_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let (col, row) = pointer_cell(pos);
                self.state.select_line_at(col, row);
            }
            return;
        }
        if response.double_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let (col, row) = pointer_cell(pos);
                self.state.select_word_at(col, row);
            }
            return;
        }

        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                let (col, row) = pointer_cell(pos);
                self.state.start_selection(col, row);
            }
        } else if response.dragged() {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                let (col, row) = pointer_cell(pos);
                self.state.update_selection(col, row);
            }
        } else if response.clicked() && !response.dragged() {
            // Single click without drag: clear selection (standard terminal).
            // Only clear when it was a primary click and not a multi-click
            // residual (handled above).
            if !response.double_clicked() && !response.triple_clicked() {
                self.state.clear_selection();
            }
        }
    }

    /// Paste clipboard text into the PTY, using bracketed paste when enabled.
    pub fn paste_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        // Normalize Windows CRLF so shells don't see `\r` as Enter mid-line.
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let bytes = self.input_mapper.wrap_paste(&normalized);
        if let Err(err) = self.backend.write(&bytes) {
            tracing::warn!(error = %err, "failed to paste into PTY");
            return;
        }
        tracing::debug!(len = normalized.len(), "Pasted text into terminal");
    }

    /// Whether the terminal currently has a non-empty text selection.
    pub fn has_selection(&self) -> bool {
        self.state.has_selection()
    }

    /// Keys that pair with the logical `command` modifier for app shortcuts.
    ///
    /// Defense-in-depth: the ShortcutManager normally consumes these before
    /// the terminal runs. Kept in sync with default bindings in
    /// `shortcut_manager.rs`.
    fn is_reserved_app_key(&self, key: &egui::Key) -> bool {
        match key {
            egui::Key::B
            | egui::Key::D
            | egui::Key::E
            | egui::Key::F
            | egui::Key::G
            | egui::Key::I
            | egui::Key::K
            | egui::Key::L
            | egui::Key::M
            | egui::Key::N
            | egui::Key::Q
            | egui::Key::R
            | egui::Key::T
            | egui::Key::W
            | egui::Key::Plus
            | egui::Key::Equals
            | egui::Key::Minus
            | egui::Key::OpenBracket
            | egui::Key::CloseBracket
            | egui::Key::Enter
            | egui::Key::ArrowLeft
            | egui::Key::ArrowRight
            | egui::Key::ArrowUp
            | egui::Key::ArrowDown
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
            // COMMAND+C only reserved when there is a selection to copy;
            // otherwise Linux still needs Ctrl+C → SIGINT for the shell.
            egui::Key::C => self.state.has_selection(),
            // COMMAND+V is always an app paste shortcut.
            egui::Key::V => true,
            _ => false,
        }
    }

    /// Try to paste a clipboard image into the terminal.
    ///
    /// Saves the image as a PNG in the system temp directory and writes the
    /// file path to the PTY input, matching the behavior of iTerm2, Kitty,
    /// and WezTerm for image pastes. Bound to `Cmd+Shift+I` via
    /// `ShortcutAction::PasteImage`, not `Cmd+V` — see that variant's doc
    /// comment in `shortcuts.rs` for why.
    ///
    /// Returns `true` if an image was found on the clipboard and pasted.
    pub fn try_paste_image(&mut self) -> bool {
        let mut clipboard = match arboard::Clipboard::new() {
            Ok(c) => c,
            Err(_) => return false,
        };

        let image_data = match clipboard.get_image() {
            Ok(img) => img,
            Err(_) => return false,
        };

        let (width, height) = (image_data.width, image_data.height);
        let bytes = image_data.bytes.into_owned();

        let mut png = Vec::new();
        let encoder = PngEncoder::new(&mut png);
        if encoder
            .write_image(&bytes, width as u32, height as u32, image::ExtendedColorType::Rgba8)
            .is_err()
        {
            return false;
        }

        self.paste_counter += 1;
        let mut path = std::env::temp_dir();
        path.push(format!("rmux-paste-{}-{}.png", std::process::id(), self.paste_counter));

        if std::fs::write(&path, &png).is_err() {
            return false;
        }

        // Quote the path (spaces are rare in temp dirs but not impossible)
        // and don't send a trailing newline: the user is typically about to
        // type more around the pasted reference (e.g. "describe this: <path>
        // please"), so auto-submitting the line would be surprising.
        let path_str = path.to_string_lossy();
        self.backend.write(format!("\"{path_str}\"").as_bytes()).ok();

        tracing::debug!(path = %path_str, width, height, "Pasted clipboard image");
        true
    }

    /// Stable id for the find-bar TextEdit (must match `show_find_bar`).
    fn find_query_id(ui: &egui::Ui) -> egui::Id {
        ui.id().with("rmux_find_query")
    }

    /// Map an egui key event to terminal bytes.
    fn map_key_to_terminal(&self, key: &egui::Key, modifiers: &egui::Modifiers) -> Option<Vec<u8>> {
        use egui::Key;

        let ctrl = modifiers.ctrl;
        let shift = modifiers.shift;
        let alt = modifiers.alt;

        match key {
            Key::Enter => Some(vec![b'\r']),
            // Shift+Tab → reverse completion (bash/zsh/fish: CSI Z).
            Key::Tab if shift => Some(vec![0x1b, b'[', b'Z']),
            Key::Tab => Some(vec![b'\t']),
            // DEL (0x7f) is what modern terminals / readline expect for ⌫.
            Key::Backspace => Some(vec![0x7f]),
            Key::Escape => Some(vec![0x1b]),
            // Space arrives as Event::Text(" ") — do not also map Key::Space
            // or the shell sees a double space.
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

    /// Change the ANSI/fg/bg/cursor color theme used by this pane.
    pub fn set_theme(&mut self, theme: rmux_terminal::TerminalTheme) {
        self.state.theme = theme;
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

    /// Force this pane's focus state, e.g. to sync with `Workspace::active_pane`
    /// after a keyboard-driven focus change (`FocusLeft`/`Right`/`Up`/`Down`).
    /// Click-to-focus still applies afterward within the same `show()` call.
    pub fn set_focus(&mut self, focus: bool) {
        self.has_focus = focus;
    }
}

// ─── Tab title helpers (cmux-style path / user@host) ─────────────────────────

/// Format a cwd for the multi-terminal tab strip.
///
/// * Under `$HOME` → `~/…` relative path; at home itself → `user@host`
/// * Deep paths keep the last two components with a `…/` prefix
pub fn format_cwd_tab_title(cwd: &Path) -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if let Some(ref home) = home {
        if cwd == home.as_path() {
            return user_host_title();
        }
        if let Ok(rest) = cwd.strip_prefix(home) {
            let rel = format!("~/{}", rest.display());
            return shorten_path_title(&rel);
        }
    }
    shorten_path_title(&cwd.display().to_string())
}

/// `user@hostname` for home-directory tabs (matches cmux default title).
fn user_host_title() -> String {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "user".to_owned());
    let host = std::env::var("HOST").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| {
        // Best-effort hostname without extra crates.
        std::process::Command::new("hostname")
            .output()
            .ok()
            .and_then(|o| {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_owned();
                if s.is_empty() { None } else { Some(s) }
            })
            .unwrap_or_else(|| "localhost".to_owned())
    });
    // Drop domain suffix for compactness: MacBook.local → MacBook
    let host = host.split('.').next().unwrap_or(&host);
    format!("{user}@{host}")
}

/// Keep tab labels short: at most ~28 chars, prefer trailing path segments.
fn shorten_path_title(path: &str) -> String {
    const MAX: usize = 28;
    if path.chars().count() <= MAX {
        return path.to_owned();
    }
    // Prefer last two components: …/parent/name
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() >= 2 {
        let tail = format!("…/{}/{}", parts[parts.len() - 2], parts[parts.len() - 1]);
        if tail.chars().count() <= MAX {
            return tail;
        }
        let last = parts[parts.len() - 1];
        if last.chars().count() <= MAX - 2 {
            return format!("…/{last}");
        }
        return truncate_chars(last, MAX);
    }
    truncate_chars(path, MAX)
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let end = s.char_indices().nth(max.saturating_sub(1)).map(|(i, _)| i).unwrap_or(s.len());
    format!("{}…", &s[..end])
}

#[cfg(test)]
mod title_tests {
    use super::*;

    #[test]
    fn test_shorten_keeps_short_paths() {
        assert_eq!(shorten_path_title("~/proj"), "~/proj");
    }

    #[test]
    fn test_shorten_deep_path_uses_ellipsis() {
        let deep = "~/Developer/PersonalProjects/RustPersonalProjects/rmux-cmd-hints";
        let short = shorten_path_title(deep);
        assert!(short.contains("rmux-cmd-hints"), "got {short}");
        assert!(short.chars().count() <= 28, "got {short}");
    }

    #[test]
    fn test_format_cwd_home_is_user_host() {
        if let Ok(home) = std::env::var("HOME") {
            let title = format_cwd_tab_title(Path::new(&home));
            assert!(title.contains('@'), "expected user@host, got {title}");
        }
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

    /// Flip the copy-mode flag and return its new value.
    ///
    /// Bound to the cmux `Cmd+Shift+M` shortcut (registered in
    /// Wave 1). The flag itself is the only state for now; actual
    /// copy-mode behaviour (vim-style scrollback nav, selection) is
    /// out of scope and will be wired up in a later wave.
    pub fn toggle_copy_mode(&mut self) -> bool {
        self.copy_mode = !self.copy_mode;
        self.copy_mode
    }

    /// Whether the pane is currently in copy mode.
    #[allow(dead_code)]
    pub fn is_copy_mode(&self) -> bool {
        self.copy_mode
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
        let query_lower: Vec<char> = self.find_query.to_lowercase().chars().collect();
        let query_len = query_lower.len();

        for row in 0..snapshot.rows as usize {
            let row_chars: Vec<char> = snapshot.cells[row].iter().map(|c| c.c).collect();

            // Search on char grid (not byte offsets) to handle non-ASCII correctly
            if row_chars.len() < query_len {
                continue;
            }
            let mut col = 0;
            while col + query_len <= row_chars.len() {
                let mut matched = true;
                for q in 0..query_len {
                    if row_chars[col + q].to_lowercase().next() != Some(query_lower[q]) {
                        matched = false;
                        break;
                    }
                }
                if matched {
                    self.find_results.push((row, col));
                    col += query_len;
                } else {
                    col += 1;
                }
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

        let palette = theme::palette();
        let match_bg = palette.warning.gamma_multiply(0.35_f32);
        let active_bg = palette.accent.gamma_multiply(0.45_f32);

        for (i, &(row, col)) in self.find_results.iter().enumerate() {
            if row >= snapshot.rows as usize || col >= snapshot.cols as usize {
                continue;
            }

            let is_active = i == self.find_index;

            // Calculate match cell position
            let x = term_rect.left() + col as f32 * cell_size.x;
            let y = term_rect.top() + row as f32 * cell_size.y;

            // Calculate match width (number of consecutive chars matching the query)
            let query_len = self.find_query.chars().count();
            let match_width = (query_len as f32 * cell_size.x).min(term_rect.right() - x);

            let highlight_rect = egui::Rect::from_min_size(
                egui::Pos2::new(x, y),
                egui::Vec2::new(match_width, cell_size.y),
            );

            let color = if is_active { active_bg } else { match_bg };
            painter.rect_filled(highlight_rect, 0.0_f32, color);
        }
    }

    /// Render the find bar at the bottom of the terminal pane.
    ///
    /// A 28px chrome strip: `chrome_bg` fill with a 1px `chrome_border`
    /// top hairline, a mono-12 input on `panel_bg` (border turns `accent`
    /// while focused), a mono-10 match counter, and 20x20 nav/close buttons.
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

        let palette = theme::palette();

        // Background: chrome strip with a 1px top hairline
        bar_ui.painter().rect_filled(bar_rect, 0.0_f32, palette.chrome_bg);
        bar_ui.painter().rect_filled(
            egui::Rect::from_min_size(
                bar_rect.left_top(),
                egui::Vec2::new(bar_rect.width(), 1.0_f32),
            ),
            0.0_f32,
            palette.chrome_border,
        );

        // Spacing
        bar_ui.add_space(8.0_f32);

        // Query text input: panel_bg fill, 1px border (accent when focused)
        let input_rect = egui::Rect::from_min_size(
            egui::Pos2::new(bar_ui.cursor().min.x, bar_rect.center().y - 10.0_f32),
            egui::Vec2::new(200.0_f32, 20.0_f32),
        );
        bar_ui.painter().rect_filled(
            input_rect,
            egui::CornerRadius::same(theme::radius_sm()),
            palette.panel_bg,
        );
        let find_id = Self::find_query_id(ui);
        let text_response = bar_ui.put(
            input_rect.shrink2(egui::Vec2::new(6.0_f32, 1.0_f32)),
            egui::TextEdit::singleline(&mut self.find_query)
                .id(find_id)
                .hint_text("Find...")
                .font(egui::FontId::monospace(12.0_f32))
                .vertical_align(egui::Align::Center)
                .frame(false),
        );
        // Prefer focus on the find field while the bar is open. Do **not**
        // mark `text_sink` here — Escape/Enter must still close / step via
        // shortcuts (`find_only` bindings). The terminal already yields for
        // the whole time `find_visible` is true.
        if !text_response.has_focus() {
            ui.memory_mut(|m| m.request_focus(find_id));
        }
        let input_border = if text_response.has_focus() { palette.accent } else { palette.border };
        bar_ui.painter().rect_stroke(
            input_rect,
            egui::CornerRadius::same(theme::radius_sm()),
            egui::Stroke::new(1.0_f32, input_border),
            egui::StrokeKind::Inside,
        );

        if text_response.changed() {
            self.perform_find();
        }

        bar_ui.add_space(8.0_f32);

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
                    .font(egui::FontId::monospace(10.0_f32))
                    .color(palette.text_muted),
            );
        }

        bar_ui.add_space(8.0_f32);

        // Nav/close buttons: 20x20, panel_bg + 1px border, hover panel_active_bg
        // (hover fill comes from the theme's widget visuals).
        let button = |bar_ui: &mut egui::Ui, label: &str| {
            bar_ui.add_sized(
                [20.0_f32, 20.0_f32],
                egui::Button::new(
                    egui::RichText::new(label).color(palette.text_primary).size(12.0_f32),
                )
                .stroke(egui::Stroke::new(1.0_f32, palette.border))
                .corner_radius(egui::CornerRadius::same(theme::radius_sm())),
            )
        };

        // Previous match button
        if button(&mut bar_ui, "\u{2039}").clicked() && !self.find_results.is_empty() {
            if self.find_index == 0 {
                self.find_index = self.find_results.len() - 1;
            } else {
                self.find_index -= 1;
            }
        }

        // Next match button
        if button(&mut bar_ui, "\u{203a}").clicked() {
            self.find_next_match();
        }

        bar_ui.add_space(8.0_f32);

        // Close button
        if button(&mut bar_ui, "\u{2715}").clicked() {
            self.find_visible = false;
            self.find_query.clear();
            self.find_results.clear();
            self.find_index = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_mode_default_false() {
        let pane = TerminalPane::spawn(1, 1, 14.0_f32).expect("PTY spawn should succeed");
        assert!(!pane.is_copy_mode(), "new pane should not be in copy mode");
    }

    #[test]
    fn test_copy_mode_toggle_flips_state() {
        let mut pane = TerminalPane::spawn(1, 1, 14.0_f32).expect("PTY spawn should succeed");
        let new_state = pane.toggle_copy_mode();
        assert!(new_state, "toggle_copy_mode should return the new value (true)");
        assert!(pane.is_copy_mode(), "is_copy_mode should reflect the toggled state");
    }

    #[test]
    fn test_copy_mode_toggle_twice_returns_to_false() {
        let mut pane = TerminalPane::spawn(1, 1, 14.0_f32).expect("PTY spawn should succeed");
        let _ = pane.toggle_copy_mode();
        let final_state = pane.toggle_copy_mode();
        assert!(!final_state, "two toggles should return the original state");
        assert!(!pane.is_copy_mode(), "is_copy_mode should report the original state");
    }

    #[test]
    fn test_copy_mode_indicator_constant_present() {
        assert_eq!(COPY_MODE_INDICATOR, " [COPY]");
        assert!(!COPY_MODE_INDICATOR.is_empty());
    }
}
