# rmux — API Reference

Full public API surface for every crate, enum, struct, trait, and function.

---

## Workspace Layout

```
crates/
├── rmux-app       (binary)  Main GUI app — egui/eframe window, event loop, all subsystems
├── rmux-terminal  (library) Terminal emulation (alacritty_terminal + portable-pty)
├── rmux-cli       (binary)  CLI tool for controlling rmux via Unix socket
├── rmux-api       (library) Socket API server (JSON-RPC protocol, method dispatch, event stream)
└── rmux-config    (library) Configuration schema types
```

---

## `rmux-config` — Configuration

### `schema.rs`

| Item | Kind | Description |
|---|---|---|
| `Config` | struct | Top-level config. Loaded from `~/.config/rmux/rmux.json` (Linux), `~/Library/Application Support/rmux/rmux.json` (macOS), `%APPDATA%\rmux\rmux.json` (Windows). |
| `Config.terminal` | field | `TerminalConfig` (default via `#[serde(default)]`) |
| `TerminalConfig` | struct | Terminal emulation settings |
| `TerminalConfig.shell` | field | `Option<String>` — Shell binary. Falls back to `$SHELL` or `/bin/sh` |
| `TerminalConfig.font_family` | field | `String` — Font family name (default: `"monospace"`) |
| `TerminalConfig.font_size` | field | `f32` — Font size in points (default: `14.0`) |
| `TerminalConfig.max_scrollback_lines` | field | `usize` — Max scrollback per pane (default: `10_000`) |
| `impl Default for TerminalConfig` | impl | Sensible defaults for all fields |

Derives: `Debug, Serialize, Deserialize, Clone`

---

## `rmux-api` — Socket API Server

### Re-exports (`lib.rs`)

| Item | Source |
|---|---|
| `ApiEvent` | `dispatch` |
| `ApiRequestEnvelope` | `dispatch` |
| `ApiResponseResult` | `dispatch` |
| `ApiError` | `error` |
| `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError` | `protocol` |
| `ApiServer`, `DEFAULT_REQUEST_TIMEOUT`, `default_socket_path` | `server` |

### `protocol.rs`

| Item | Kind | Description |
|---|---|---|
| `codes::PARSE_ERROR` | const | `-32700` — Invalid JSON |
| `codes::METHOD_NOT_FOUND` | const | `-32601` — Unknown method name |
| `codes::INTERNAL_ERROR` | const | `-32000` — Server/app failure |
| `codes::TIMEOUT` | const | `-32001` — App did not answer in time |
| `JsonRpcRequest` | struct | Incoming JSON-RPC request. Fields: `id` (Value), `method` (String), `params` (Value) |
| `JsonRpcResponse` | struct | Outgoing JSON-RPC response. Fields: `id`, `ok` (bool), `result` (Option), `error` (Option) |
| `JsonRpcResponse::success(id, Value)` | method | Build success response |
| `JsonRpcResponse::failure(id, JsonRpcError)` | method | Build failure response |
| `JsonRpcError` | struct | Error detail: `code` (i32), `message` (String) |
| `JsonRpcError::new(code, msg)` | method | Constructor |
| `JsonRpcError::parse_error(detail)` | method | Creates `PARSE_ERROR` |
| `JsonRpcError::method_not_found(method)` | method | Creates `METHOD_NOT_FOUND` |
| `JsonRpcError::internal(detail)` | method | Creates `INTERNAL_ERROR` |
| `JsonRpcError::timeout()` | method | Creates `TIMEOUT` |

### `error.rs`

| Variant | Description |
|---|---|
| `ApiError::Bind { path, source }` | Binding the Unix listener failed |
| `ApiError::Io(source)` | Socket I/O error (from `std::io::Error`) |
| `ApiError::UnsupportedPlatform` | Unix sockets not available |

Derives: `Debug, Error`

### `server.rs`

| Item | Description |
|---|---|
| `DEFAULT_REQUEST_TIMEOUT` | `Duration::from_secs(5)` |
| `default_socket_path()` → `PathBuf` | `$RMUX_SOCKET_PATH` > `/tmp/rmux-debug.sock` (debug) > `/tmp/rmux.sock` (release) |
| `ApiServer` | Binds Unix socket, spawns accept loop on Tokio runtime |
| `ApiServer::bind(path, request_tx, event_tx)` | Bind with default timeout |
| `ApiServer::bind_with_timeout(path, request_tx, event_tx, timeout)` | Bind with custom timeout |
| `ApiServer::socket_path()` → `&Path` | Bound socket path |
| `ApiServer::shutdown(self)` | Stop accepting, remove socket file |

### `methods.rs` — All 30 Method Names

| Constant | String | Description |
|---|---|---|
| `SYSTEM_PING` | `"system.ping"` | Health check → `{pong: true}` |
| `SYSTEM_CAPABILITIES` | `"system.capabilities"` | Version + supported methods |
| `SYSTEM_IDENTIFY` | `"system.identify"` | App name, version, PID |
| `WORKSPACE_LIST` | `"workspace.list"` | List all workspaces |
| `WORKSPACE_CREATE` | `"workspace.create"` | Create named workspace |
| `WORKSPACE_SELECT` | `"workspace.select"` | Select by index |
| `WORKSPACE_CLOSE` | `"workspace.close"` | Close by id |
| `WORKSPACE_RENAME` | `"workspace.rename"` | Rename workspace by id |
| `SURFACE_LIST` | `"surface.list"` | List all panes |
| `SURFACE_SPLIT` | `"surface.split"` | Split active pane (Right/Down) |
| `SURFACE_FOCUS` | `"surface.focus"` | Focus specific pane |
| `SURFACE_CLOSE` | `"surface.close"` | Close pane (active when id omitted) |
| `SURFACE_NEW` | `"surface.new"` | New terminal tab/surface |
| `SURFACE_SEND_TEXT` | `"surface.send_text"` | Type text into active pane |
| `SURFACE_SEND_KEY` | `"surface.send_key"` | Send named key press |
| `NOTIFICATION_CREATE` | `"notification.create"` | Create notification |
| `NOTIFICATION_LIST` | `"notification.list"` | List all notifications |
| `NOTIFICATION_CLEAR` | `"notification.clear"` | Clear all notifications |
| `SIDEBAR_SET_STATUS` | `"sidebar.set_status"` | Set status text on workspace |
| `SIDEBAR_CLEAR_STATUS` | `"sidebar.clear_status"` | Clear status text |
| `SIDEBAR_SET_PROGRESS` | `"sidebar.set_progress"` | Set progress bar (0.0–1.0) |
| `BROWSER_OPEN` | `"browser.open"` | Open browser pane split |
| `BROWSER_NAVIGATE` | `"browser.navigate"` | Navigate active browser |
| `BROWSER_BACK` | `"browser.back"` | Browser history back |
| `BROWSER_FORWARD` | `"browser.forward"` | Browser history forward |
| `BROWSER_RELOAD` | `"browser.reload"` | Reload active browser |
| `BROWSER_URL` | `"browser.url"` | Read active browser URL |
| `APP_SET_FONT_SIZE` | `"app.set_font_size"` | Change/reset font size |
| `APP_SET_THEME` | `"app.set_theme"` | Set terminal color theme |
| `EVENTS_STREAM` | `"events.stream"` | Event streaming mode |

| Function | Returns |
|---|---|
| `all_methods()` | `&'static [&'static str]` — All 30 method names |

**Request/Response types** (all derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`):

| Type | Fields |
|---|---|
| `PingResult` | `pong: bool` |
| `CapabilitiesResult` | `version: String, methods: Vec<String>` |
| `IdentifyResult` | `app: String, version: String, pid: u32` |
| `WorkspaceInfo` | `id: u64, name: String, pane_count: usize, active: bool` |
| `WorkspaceListResult` | `workspaces: Vec<WorkspaceInfo>` |
| `WorkspaceCreateParams` | `name: Option<String>` |
| `WorkspaceCreateResult` | `id: u64` |
| `WorkspaceSelectParams` | `index: usize` |
| `WorkspaceCloseParams` | `id: u64` |
| `WorkspaceRenameParams` | `id: u64, name: String` |
| `SurfaceInfo` | `pane_id: u64, workspace_id: u64, active: bool` |
| `SurfaceListResult` | `surfaces: Vec<SurfaceInfo>` |
| `SplitDirection` | enum: `Right`, `Down` |
| `SurfaceSplitParams` | `direction: SplitDirection` |
| `SurfaceSplitResult` | `pane_id: u64` |
| `SurfaceFocusParams` | `pane_id: u64` |
| `SurfaceCloseParams` | `pane_id: Option<u64>` |
| `SurfaceNewParams` | `title: Option<String>` |
| `SurfaceNewResult` | `pane_id: u64` |
| `SurfaceSendTextParams` | `text: String` |
| `SurfaceSendKeyParams` | `key: String` |
| `NotificationCreateParams` | `title: String, subtitle: Option<String>, body: Option<String>` |
| `NotificationCreateResult` | `id: u64` |
| `NotificationInfo` | `id: u64, title, subtitle: Option<String>, body: Option<String>` |
| `NotificationListResult` | `notifications: Vec<NotificationInfo>` |
| `SidebarSetStatusParams` | `workspace_id: Option<u64>, status: String` |
| `SidebarClearStatusParams` | `workspace_id: Option<u64>` |
| `SidebarSetProgressParams` | `value: f32` |
| `BrowserOpenParams` | `url: Option<String>` |
| `BrowserOpenResult` | `pane_id: u64` |
| `BrowserNavigateParams` | `url: String` |
| `BrowserUrlResult` | `url: String` |
| `AppSetFontSizeParams` | `delta: Option<f32>, reset: bool` |
| `AppSetFontSizeResult` | `font_size: f32` |
| `AppSetThemeParams` | `theme: String` |

### `dispatch.rs`

| Item | Description |
|---|---|
| `ApiResponseResult` | Type alias: `Result<Value, JsonRpcError>` |
| `ApiRequestEnvelope` | Fields: `method` (String), `params` (Value), `respond` (oneshot::Sender) |
| `ApiEvent` | Fields: `event` (String), `data` (Value). Constructor: `ApiEvent::new(event, data)` |

---

## `rmux-terminal` — Terminal Emulation

### Re-exports (`lib.rs`)

| Item | Source |
|---|---|
| `PtyBackend`, `PtyError`, `PtyResult` | `backend` |
| `InputMapper` | `input` |
| `OscKind`, `OscNotification`, `OscScanner` | `osc` |
| `TerminalRenderer` | `renderer` |
| `GridCell`, `GridSnapshot`, `TermState` | `state` |
| `CursorShape` | re-exported from `alacritty_terminal::vte::ansi` |

### `backend.rs`

| Item | Kind | Description |
|---|---|---|
| `PtyError` | enum | PTY errors: `OpenPty`, `SpawnProcess`, `WriteError`, `ResizeError`, `IoSetup` |
| `PtyResult<T>` | type | `Result<T, PtyError>` |
| `PtyBackend` | struct | Cross-platform PTY via `portable-pty` |
| `PtyBackend::spawn(cols, rows)` | fn | Spawns `$SHELL` or `/bin/sh`. Returns `PtyResult<Self>` |
| `PtyBackend::write(&mut self, &[u8])` | fn | Write bytes to PTY |
| `PtyBackend::try_read(&mut self, &mut [u8])` | fn | Non-blocking read. Returns `Option<usize>` |
| `PtyBackend::take_reader(&mut self)` | fn | Move reader to background thread. Returns `Option<Box<dyn Read + Send>>` |
| `PtyBackend::resize(&mut self, cols, rows)` | fn | Resize PTY terminal |
| `PtyBackend::is_alive(&self)` | fn | `bool` — Process running? |
| `PtyBackend::try_wait(&mut self)` | fn | Non-blocking exit status check |
| `PtyBackend::kill(&mut self)` | fn | Force-kill child process |

### `state.rs`

| Item | Kind | Description |
|---|---|---|
| `TermState` | struct | Wraps `alacritty_terminal::Term`. VTE parser + grid |
| `TermState::new(cols, rows, scrollback_limit)` | fn | Initialize with dimensions |
| `TermState::feed_bytes(&mut self, &[u8])` | fn | Feed PTY output through VTE parser |
| `TermState::snapshot(&self)` | fn | `GridSnapshot` — Owned copy of visible grid |
| `TermState::resize(&mut self, cols, rows)` | fn | Resize terminal character grid |
| `TermState::cursor_pos(&self)` | fn | `(u16, u16)` — Current cursor position |
| `TermState::scroll(&mut self, lines)` | fn | Scroll viewport up (+)/down (-) into scrollback |
| `TermState::colors(&self)` | fn | `Colors` — Underlying alacritty color palette |
| `TermState::copy_selected_text(&self)` | fn | `Option<String>` — Copy selected text |
| `TermState::clear_scrollback(&mut self)` | fn | Send `CSI 3 J` to clear scrollback |
| `GridSnapshot` | struct | Immutable view of grid at point in time |
| `GridSnapshot.cols/rows/cells` | field | Dimensions and `Vec<Vec<GridCell>>` indexed `[row][col]` |
| `GridSnapshot.cursor_row/cursor_col` | field | Cursor position |
| `GridSnapshot.cursor_shape` | field | `CursorShape` from alacritty |
| `GridSnapshot.display_offset` | field | `usize` — Scrollback offset |
| `GridCell` | struct | Single cell: `c` (char), `fg`/`bg` (Color32), `bold/italic/underline` (bool), `is_cursor` (bool) |

### `renderer.rs`

| Item | Kind | Description |
|---|---|---|
| `TerminalRenderer` | struct | Grid → egui paint commands |
| `TerminalRenderer.font_size` | field | `f32` — Pub field for querying |
| `TerminalRenderer::new(font_size)` | fn | Initialize with font metrics |
| `TerminalRenderer::draw(&self, ui, rect, snapshot, cursor_visible)` | fn | Render terminal grid into egui Ui |
| `TerminalRenderer::set_font_size(&mut self, font_size)` | fn | Update font size, recalculate cell metrics |
| `TerminalRenderer::cell_size()` | fn | `Vec2` — Pre-calculated cell dimensions |
| `TerminalRenderer::cols_rows_for_rect(&self, rect)` | fn | `(u16, u16)` — Cells fitting in area |

### `input.rs`

| Item | Kind | Description |
|---|---|---|
| `InputMapper` | struct | Maps egui key events → terminal byte sequences |
| `InputMapper::new()` | fn | Default state (bracket paste off) |
| `InputMapper::map_char(&self, c, ctrl, alt)` | fn | `Vec<u8>` — Character input to terminal bytes |
| `InputMapper::map_named_key(&self, name, ctrl, shift)` | fn | `Option<Vec<u8>>` — Named keys: arrows, F-keys, home/end, pgup/pgdn, delete, insert, tab, esc |
| `InputMapper::wrap_paste(&self, text)` | fn | `Vec<u8>` — Wrap text in bracket-paste sequences |
| `InputMapper::set_bracket_paste_mode(&mut self, enabled)` | fn | Toggle paste wrapping |

### `osc.rs`

| Item | Kind | Description |
|---|---|---|
| `OscKind` | enum | `Simple9` (OSC 9), `Rich99` (OSC 99), `Legacy777` (OSC 777) |
| `OscNotification` | struct | `title: String`, `body: Option<String>`, `kind: OscKind` |
| `OscScanner` | struct | Incremental OSC sequence parser |
| `OscScanner::new()` | fn | Fresh scanner |
| `OscScanner::feed(&mut self, &[u8])` | fn | `Vec<OscNotification>` — Feed bytes, get completed notifications. Payload capped at 4096 bytes. Splits across calls OK. |

---

## `rmux-cli` — Command-Line Tool

### `main.rs`

**Subcommands** (clap `Command` enum):

| Command | Options | Description |
|---|---|---|
| `ping` | — | Health check: prints "pong" or error |
| `capabilities` | — | Prints server capabilities as pretty JSON |
| `notify` | `--title T --subtitle S --body B` | Create desktop notification |
| `new-workspace` | `--name N` | Create new workspace. Prints workspace id |
| `list-workspaces` | `--json` (flag) | List workspaces. Table mode (default) or JSON |
| `new-split` | `--direction right\|down` | Split active pane. Prints new pane id |
| `send` | `--text T` | Send text to active pane. Supports `\n`, `\r`, `\t`, `\e`, `\\` |

Exit codes: `0` success, `1` error, `2` cannot connect

### `commands.rs`

All return `anyhow::Result<()>`:

| Function | Description |
|---|---|
| `ping(&Path)` | Sends `system.ping`, prints "pong" |
| `capabilities(&Path)` | Prints pretty JSON of server capabilities |
| `notify(&Path, title, subtitle, body)` | Creates notification, prints id |
| `new_workspace(&Path, name)` | Creates workspace, prints id |
| `list_workspaces(&Path, json)` | Prints table (`"ID   Name   Panes"`) or JSON |
| `new_split(&Path, direction)` | Splits pane (`"right"`/`"down"`), prints pane id |
| `send(&Path, text)` | Interprets escape sequences, sends text |

### `socket.rs`

| Item | Description |
|---|---|
| `SOCKET_PATH_ENV` | `"RMUX_SOCKET_PATH"` — Env var for custom socket location |
| `ConnectError` | Error when socket connection fails (path + io::Error) |
| `ServerError` | JSON-RPC error from the server (code + message) |
| `effective_socket_path(Option<PathBuf>)` | Resolves socket path: flag > env var > built-in default |
| `call(path, method, params)` | `anyhow::Result<Value>` — Blocking request/response roundtrip. 5s timeout. Unix only. |

---

## `rmux-app` — Main Application

### `main.rs`

| Item | Kind | Description |
|---|---|---|
| `Cli` | struct | clap argument parser |
| `Cli.verbose` | field | `-v/--verbose`: enable debug logging |
| `Cli.config` | field | `-c/--config <PATH>`: custom config file |
| `Cli.session` | field | `-s/--session <PATH>`: session restore file |
| `init_logging(verbose)` | fn | Sets up tracing subscriber with env-filter |
| `setup_fonts(ctx)` | fn | No-op — uses egui default font definitions |

### `app.rs`

| Item | Kind | Description |
|---|---|---|
| `RmuxApp` | struct | Root application state. Implements `eframe::App` |
| `RmuxApp.workspace_manager` | field | `WorkspaceManager` — All workspaces, panes, splits |
| `RmuxApp.sidebar` | field | `SidebarView` — Left workspace tab panel |
| `RmuxApp.notifications` | field | `NotificationManager` — Store + desktop notifications |
| `RmuxApp.notification_panel` | field | `NotificationPanel` — Right notification list |
| `RmuxApp.font_size` | field | `f32` — Shared terminal font size |
| `RmuxApp.last_copied_text` | field | `Option<String>` — Most recent terminal copy |
| `RmuxApp::new(cc)` | fn | Initialize: create default workspace, terminal, start API server |
| `RmuxApp::publish_event(&self, event, data)` | fn | Publish to `events.stream` broadcast channel |
| `RmuxApp::create_workspace_with_terminal(&mut self, name) → u64` | fn | New workspace + terminal. Publishes events |
| `RmuxApp::split_active_with_terminal(&mut self, direction) → Result<u64>` | fn | Split + spawn terminal. Publishes event |
| `RmuxApp::close_active_pane_with_event(&mut self) → Result<()>` | fn | Close pane. Publishes event |
| `RmuxApp::open_browser_split(&mut self, url) → Result<u64>` | fn | Split + create browser pane |
| `RmuxApp::close_active_workspace_with_event(&mut self) → Result<u64>` | fn | Close workspace. Publishes event |
| `RmuxApp::start_workspace_rename(&mut self)` | fn | Start inline rename in sidebar |
| `RmuxApp::set_font_size(&mut self, delta)` | fn | `delta=0.0` resets to default; clamped `[6.0, 60.0]` |
| `RmuxApp::active_terminal_mut(&mut self)` | fn | `Option<&mut TerminalPane>` |
| `RmuxApp::active_browser_mut(&mut self)` | fn | `Option<&mut BrowserPane>` |
| `RmuxApp::handle_keyboard_shortcuts(&mut self, ctx)` | fn | All global keyboard shortcuts (see Key Bindings below) |

### `api.rs`

| Item | Description |
|---|---|
| `ApiChannels` | Struct: `request_rx` (mpsc::Receiver), `event_tx` (broadcast::Sender) |
| `start_server()` → `ApiChannels` | Spawns background thread with Tokio runtime for API server |

### `api_dispatch.rs`

| Item | Description |
|---|---|
| `dispatch(app, method, params)` → `Result<Value, JsonRpcError>` | Routes to handler for all 19 methods |

### Key Bindings (`shortcuts.rs`)

**Always active** (even when text widget has focus):

| Shortcut | Action |
|---|---|
| `Cmd/Ctrl+Q` | Quit application |
| `Cmd/Ctrl++/=` | Increase font size |
| `Cmd/Ctrl+-` | Decrease font size |
| `Cmd/Ctrl+0` | Reset font size to default |
| `Cmd/Ctrl+C` | Copy terminal selection to clipboard |
| `Escape` (find visible) | Close find bar |
| `Enter` (find visible) | Find next match |
| `Cmd/Ctrl+F` | Toggle find bar |
| `Cmd/Ctrl+G` | Find next match |
| `Alt+Cmd/Ctrl+G` | Find previous match |
| `Cmd/Ctrl+E` | Use selection for find |
| `Cmd/Ctrl+K` | Clear terminal scrollback |
| `Cmd/Ctrl+Shift+K` | Clear screen (send Ctrl+L) |

**Focus-dependent** (skip when typing in terminal):

| Shortcut | Action |
|---|---|
| `Cmd/Ctrl+B` | Toggle sidebar |
| `Cmd/Ctrl+I` | Toggle notification panel |
| `Cmd/Ctrl+N` | New workspace with terminal |
| `Cmd/Ctrl+D` | Split right (horizontal split) |
| `Cmd/Ctrl+Shift+D` | Split down (vertical split) |
| `Cmd/Ctrl+W` | Close active pane |
| `Cmd/Ctrl+Shift+L` | Open browser split |
| `Cmd/Ctrl+L` | Focus browser URL bar |
| `Cmd/Ctrl+R` | Reload browser page |
| `Cmd/Ctrl+1-9` | Switch to workspace by index |
| `Cmd/Ctrl+Shift+W` | Close active workspace |
| `Cmd/Ctrl+Shift+R` | Rename active workspace |
| `Cmd/Ctrl+Shift+Enter` | Toggle pane zoom |
| `Cmd/Ctrl+Shift+=` | Equalize split sizes |
| `Cmd/Ctrl+Shift+[/]` | Previous/next workspace |
| `Cmd/Ctrl+ArrowLeft/Up` | Focus previous pane |
| `Cmd/Ctrl+ArrowRight/Down` | Focus next pane |

---

## `rmux-app/ui/` — UI Components

### `ui/theme.rs`

| Item | Kind | Description |
|---|---|---|
| `Palette` | struct | Arbor One Dark color tokens (see `docs/UI_REDESIGN.md`) |
| `Palette` surfaces | fields | `app_bg`, `terminal_bg`, `sidebar_bg`, `panel_bg`, `panel_active_bg`, `chrome_bg`, `tab_active_bg` |
| `Palette` lines | fields | `border`, `chrome_border` |
| `Palette` text | fields | `text_primary`, `text_muted`, `text_disabled` |
| `Palette` accent | fields | `accent`, `accent_fg`, `success`, `danger`, `warning`, `info` |
| `Palette` terminal | fields | `terminal_cursor`, `terminal_selection_bg` |
| `Palette::dark()` | fn | Returns the One Dark palette |
| `Theme` | struct | Palette + radius + dark flag |
| `Theme::dark()` | fn | Builds dark theme |
| `Theme::apply(&self, ctx)` | fn | Push theme into egui context |
| `Theme::current(ctx)` | fn | Read theme from egui context |
| `palette()` | fn | Convenience: `Palette::dark()` |
| `metrics` constants | | `TOP_BAR_HEIGHT` (34), `STATUS_BAR_HEIGHT` (26), `SIDEBAR_*` widths (200-320), `BUTTON_HEIGHT` (24), `INPUT_HEIGHT` (28) |

### `ui/terminal_pane.rs`

| Item | Kind | Description |
|---|---|---|
| `DEFAULT_FONT_SIZE` | const | `14.0` |
| `TerminalPane` | struct | Full terminal pane: PTY, rendering, input, find bar |
| `TerminalPane::spawn(cols, rows, font_size)` | fn | Spawn process, start PTY reader thread |
| `TerminalPane::process_pty_output(&mut self)` | fn | Drain channel, feed VTE parser, check exit |
| `TerminalPane::send_text(&mut self, text)` | fn | Write text to PTY |
| `TerminalPane::take_notifications(&mut self)` | fn | Drain OSC notifications |
| `TerminalPane::show(&mut self, ui)` | fn | Render terminal + handle input |
| `TerminalPane::set_font_size(&mut self, size)` | fn | Update font, recalibrate grid |
| `TerminalPane::is_exited(&self)` | fn | Process has exited? |
| `TerminalPane::has_focus(&self)` | fn | Does the rendered widget have focus? |
| `TerminalPane::copy_selection(&self)` | fn | Copy selected text |
| `TerminalPane::clear_scrollback(&mut self)` | fn | Clear scrollback buffer |
| `TerminalPane::is_find_visible(&self)` | fn | Find bar open? |
| `TerminalPane::close_find_bar(&mut self)` | fn | Close find bar |
| `TerminalPane::toggle_find(&mut self)` | fn | Toggle find bar visibility |
| `TerminalPane::find_with_selection(&mut self)` | fn | Populate find with selection |
| `TerminalPane::find_next_match(&mut self)` | fn | Jump to next match |
| `TerminalPane::find_prev_match(&mut self)` | fn | Jump to previous match |

### `ui/sidebar.rs`

| Item | Kind | Description |
|---|---|---|
| `SidebarView` | struct | Left sidebar for workspace tabs |
| `SidebarView.visible` | field | `bool` (default: `true`) |
| `SidebarView::new()` | fn | Default state |
| `SidebarView::toggle(&mut self)` | fn | Flip visibility |
| `SidebarView::start_rename(&mut self, index, name)` | fn | Begin inline rename |
| `SidebarView::show(&mut self, ctx, manager, notifications)` | fn | Render sidebar. Returns `true` if "+ New" clicked |

### `ui/workspace_view.rs`

| Item | Description |
|---|---|
| `render_pane_tree(ui, root, active_pane, zoomed_pane)` | Recursively renders `PaneNode` tree as split layout. Supports `Leaf`, `Browser`, and `Split` node rendering. Zoomed mode shows only the target pane. |

### `ui/status_bar.rs`

| Item | Description |
|---|---|
| `show(ctx, manager, notifications)` | 26px bottom bar: workspace name + pane count (left), total workspaces + unread count (right) |

### `ui/top_bar.rs`

| Item | Description |
|---|---|
| `show(ctx, manager, notifications, sidebar_visible, notification_panel_visible)` | 34px top bar: hamburger (sidebar toggle), centered workspace title, bell (notification panel toggle) |

### `ui/notification_panel.rs`

| Item | Kind | Description |
|---|---|---|
| `NotificationPanel` | struct | Right-side notification list |
| `NotificationPanel.visible` | field | `bool` (default: `false`) |
| `NotificationPanel::new()` | fn | Default state |
| `NotificationPanel::toggle(&mut self)` | fn | Flip visibility |
| `NotificationPanel::show(&mut self, ctx, notifications, manager)` | fn | Render panel: header, actions, card list |

---

## `rmux-app/workspace/` — Workspace Management

### `workspace/mod.rs`

| Item | Kind | Description |
|---|---|---|
| `ExitCleanup` | struct | Tracks closed panes `(Vec<(u64, u64)>)` and workspaces `(Vec<u64>)` |
| `WorkspaceManager` | struct | Owns all workspaces, manages IDs, orchestrates operations |
| `WorkspaceManager::new()` | fn | Creates default "Workspace 1" |
| `WorkspaceManager::create_workspace(name)` | fn | `WorkspaceId` — New workspace with empty pane |
| `WorkspaceManager::switch_to(index)` | fn | Set active workspace |
| `WorkspaceManager::switch_next()` | fn | Next workspace (wraps around) |
| `WorkspaceManager::switch_prev()` | fn | Previous workspace (wraps around) |
| `WorkspaceManager::active()` | fn | `&Workspace` |
| `WorkspaceManager::active_mut()` | fn | `&mut Workspace` |
| `WorkspaceManager::workspaces()` | fn | `&[Workspace]` |
| `WorkspaceManager::workspaces_mut()` | fn | `&mut [Workspace]` |
| `WorkspaceManager::workspace_mut(id)` | fn | `Option<&mut Workspace>` |
| `WorkspaceManager::workspace_count()` | fn | `usize` |
| `WorkspaceManager::total_pane_count()` | fn | `usize` — Across all workspaces |
| `WorkspaceManager::split_active_right()` | fn | `Result<PaneId>` — Horizontal split |
| `WorkspaceManager::split_active_down()` | fn | `Result<PaneId>` — Vertical split |
| `WorkspaceManager::close_active_pane()` | fn | `Result<()>` |
| `WorkspaceManager::process_all_panes()` | fn | Drain PTY output everywhere. Returns `Vec<(u64, u64, OscNotification)>` |
| `WorkspaceManager::close_exited_panes()` | fn | Auto-close panes with dead processes |
| `WorkspaceManager::rename_workspace(id, name)` | fn | Rename workspace |
| `WorkspaceManager::close_active_workspace()` | fn | `Result<WorkspaceId>` — Error if last |
| `WorkspaceManager::toggle_zoom()` | fn | `Option<PaneId>` — Zoom in/out |
| `WorkspaceManager::equalize_splits()` | fn | Reset all split ratios to equal |
| `WorkspaceManager::focus_pane_global(pane)` | fn | `bool` — Switch workspace to find pane |
| `WorkspaceManager::check_pane_guardrail()` | fn | Warns if total panes > 50 |

### `workspace/model.rs`

| Item | Kind | Description |
|---|---|---|
| `WorkspaceId(pub u64)` | struct | Newtype for workspace identity |
| `FocusDirection` | enum | `Next`, `Previous` |
| `Workspace` | struct | Groups a pane tree with metadata |
| `Workspace.id` | field | `WorkspaceId` |
| `Workspace.name` | field | `String` — Display name in sidebar |
| `Workspace.root` | field | `PaneNode` — Root of pane tree |
| `Workspace.active_pane` | field | `PaneId` — Currently focused pane |
| `Workspace.status` | field | `Option<String>` — Sidebar status text |
| `Workspace.progress` | field | `Option<f32>` — Sidebar progress bar (0.0–1.0) |
| `Workspace.zoomed_pane` | field | `Option<PaneId>` — Maximized single pane |
| `Workspace::new(id, name, next_pane_id)` | fn | Create with single empty Leaf pane |
| `Workspace::set_terminal(pane_id, terminal)` | fn | Attach terminal to pane |
| `Workspace::set_browser(pane_id, browser)` | fn | Replace pane with browser |
| `Workspace::process_pty_outputs(&mut self, &mut Vec)` | fn | Drain all panes' PTY output |
| `Workspace::split_right(pane_id, next_ids)` | fn | `Result<PaneId>` — Horizontal split |
| `Workspace::split_down(pane_id, next_ids)` | fn | `Result<PaneId>` — Vertical split |
| `Workspace::close_pane(pane_id)` | fn | `Result<()>` — Close pane, collapse |
| `Workspace::focus_pane(pane_id)` | fn | Set active pane |
| `Workspace::focus_next()` | fn | Next pane in DFS order |
| `Workspace::focus_prev()` | fn | Previous pane in DFS order |
| `Workspace::pane_ids()` | fn | `Vec<PaneId>` |
| `Workspace::pane_count()` | fn | `usize` |
| `Workspace::active_terminal()` | fn | `Option<&mut TerminalPane>` |

### `workspace/splits.rs`

| Item | Kind | Description |
|---|---|---|
| `PaneTreeError` | enum | `PaneNotFound(PaneId)`, `SplitNotFound(SplitId)`, `CannotCloseLastPane`, `NotALeaf`, `InvalidChildIndex(usize)` |
| `PaneId(pub u64)` | struct | Newtype for pane identity |
| `SplitId(pub u64)` | struct | Newtype for split container identity |
| `SplitDirection` | enum | `Horizontal` (side-by-side), `Vertical` (stacked) |
| `PaneNode` | enum | Recursive pane tree node |
| `PaneNode::Leaf` | variant | `{ id: PaneId, terminal: Box<Option<TerminalPane>> }` |
| `PaneNode::Browser` | variant | `{ id: PaneId, browser: Box<BrowserPane> }` |
| `PaneNode::Split` | variant | `{ id: SplitId, direction, children: Vec<PaneNode>, sizes: Vec<f32> }` |
| `PaneNode::new_leaf(id)` | fn | Create empty leaf |
| `PaneNode::new_leaf_with_terminal(id, terminal)` | fn | Create leaf with terminal |
| `PaneNode::new_browser(id, browser)` | fn | Create browser node |
| `PaneNode::new_split(id, direction, children)` | fn | Create split with equal sizes |
| `PaneNode::is_leaf()/is_browser()/is_split()` | fn | Type checks |
| `PaneNode::pane_id()` | fn | `Option<PaneId>` |
| `PaneNode::find_terminal_mut(target)` | fn | `Option<&mut Option<TerminalPane>>` — DFS search |
| `PaneNode::get_terminal(target)` | fn | `Option<&mut TerminalPane>` — Convenience |
| `PaneNode::find_browser_mut(target)` | fn | `Option<&mut BrowserPane>` — DFS search |
| `PaneNode::find_pane_mut(target)` | fn | `Option<&mut PaneNode>` — Generic DFS search |
| `PaneNode::replace_pane(target, new_node)` | fn | `bool` — Replace pane by ID |
| `PaneNode::is_browser_pane(target)` | fn | `bool` |
| `PaneNode::process_pty_outputs(&mut self, &mut Vec)` | fn | Drain PTY + collect OSC |
| `PaneNode::pane_ids()` | fn | `Vec<PaneId>` — All leaf/browser IDs |
| `PaneNode::pane_count()` | fn | `usize` — Total leaf/browser nodes |
| `PaneNode::split_at(target, direction, new_id, split_id)` | fn | `Result<PaneId>` — Split a leaf/browser into a container |
| `PaneNode::close_pane(target)` | fn | `Result<()>` — Remove + auto-collapse |
| `PaneNode::resize_split(split_id, index, delta)` | fn | `Result<()>` — Adjust sizes, re-normalize |
| `PaneNode::leaf_panes()` | fn | `Vec<(PaneId, Option<&TerminalPane>)>` — All terminal panes |
| `PaneNode::leaf_panes_mut()` | fn | Mutable version |
| `PaneNode::equalize_splits()` | fn | Recursively reset all split ratios |
| `PaneNode::collect_exited_panes()` | fn | `Vec<PaneId>` — Find panes with dead processes |

---

## `rmux-app/browser/` — Browser Pane

### `browser/webview.rs`

| Item | Kind | Description |
|---|---|---|
| `BrowserPane` | struct | In-app browser pane wrapping `wry` OS webview |
| `BrowserPane.focus_url_bar` | field | `bool` — Set by `Cmd/Ctrl+L` to request focus |
| `BrowserPane::new()` | fn | Default state: `"about:blank"` |
| `BrowserPane::is_open()` | fn | `bool` — Webview created? |
| `BrowserPane::set_open(bool)` | fn | Toggle webview state |
| `BrowserPane::url()` | fn | `&str` — Current URL |
| `BrowserPane::title()` | fn | `&str` — Page title |
| `BrowserPane::is_loading()` | fn | `bool` — Page loading? |
| `BrowserPane::history()` | fn | `&[String]` — Navigation history |
| `BrowserPane::navigate(&mut self, url)` | fn | `Result<()>` — Load URL. Auto-prepends `https://` |
| `BrowserPane::go_back()` | fn | `Result<()>` — Previous page |
| `BrowserPane::go_forward()` | fn | `Result<()>` — Next page |
| `BrowserPane::reload()` | fn | `Result<()>` — Reload via JS |
| `BrowserPane::can_go_back()` | fn | `bool` |
| `BrowserPane::can_go_forward()` | fn | `bool` |
| `BrowserPane::evaluate_javascript(&mut self, script)` | fn | `Result<()>` — Execute JS in webview |
| `BrowserPane::set_bounds(&mut self, Rect)` | fn | Set egui rect for webview positioning |
| `BrowserPane::create_webview(&mut self, &impl HasWindowHandle)` | fn | `Result<()>` — Build native wry webview as child window |
| `BrowserPane::destroy_webview(&mut self)` | fn | Tear down webview |
| `BrowserPane::reposition_webview(&mut self)` | fn | Apply pending bounds to native webview |

---

## `rmux-app/notifications/` — Notification System

### `notifications/mod.rs`

| Item | Kind | Description |
|---|---|---|
| `Notification` | struct | Stored notification: `id` (u64), `pane_id` (Option<u64>), `workspace_id` (Option<u64>), `title` (String), `body` (Option<String>), `timestamp` (SystemTime), `read` (bool) |
| `DesktopNotifier` | trait | `fn notify(&self, title: &str, body: Option<&str>)`. Must not panic. Send |
| `SystemNotifier` | struct | Real OS notification via `notify-rust`. Pumps from background thread |

### `notifications/manager.rs`

| Item | Kind | Description |
|---|---|---|
| `NotificationManager` | struct | Stores notifications, emits desktop alerts |
| `NotificationManager::new(notifier)` | fn | `NotificationManager` — With custom notifier |
| `NotificationManager::with_system_notifier()` | fn | `NotificationManager` — Uses `SystemNotifier` |
| `NotificationManager::add(title, body, pane_id, workspace_id)` | fn | `u64` — Add notification. Emits desktop. Caps at 200 |
| `NotificationManager::list()` | fn | `&[Notification]` — Oldest first |
| `NotificationManager::unread_count()` | fn | `usize` |
| `NotificationManager::unread_count_for_workspace(ws)` | fn | `usize` |
| `NotificationManager::mark_read(id)` | fn | Mark single notification read |
| `NotificationManager::mark_all_read()` | fn | Mark all read |
| `NotificationManager::clear()` | fn | Remove all notifications |

---

## Thread Model

```
Main thread (egui event loop)
  +-- UI rendering (TerminalPane::show, sidebar, workspace view, browser pane)
  +-- Keyboard/mouse event handling
  +-- Workspace management
  +-- API request processing (drained each frame from mpsc channel)

Background threads:
  +-- PTY reader thread (per terminal pane, std::thread)
      Reads PTY master → mpsc channel → main thread
  +-- rmux-api-server thread (std::thread hosting Tokio runtime)
      Unix socket listener → accept connections → JSON-RPC wire protocol
  +-- rmux-notify thread (per desktop notification)
      notify-rust delivery (avoids macOS main thread re-entry)
```

## Data Flow

```
┌─────────────────────────────────────────────────────────────┐
│  Keyboard → egui Event → TerminalPane::handle_keyboard_input │
│       → InputMapper::map_* → PtyBackend::write               │
├─────────────────────────────────────────────────────────────┤
│  PTY output → reader thread → mpsc channel                   │
│       → TerminalPane::process_pty_output                     │
│       → TermState::feed_bytes + OscScanner::feed             │
├─────────────────────────────────────────────────────────────┤
│  Render → TermState::snapshot → GridSnapshot                │
│       → TerminalRenderer::draw → egui paint commands         │
├─────────────────────────────────────────────────────────────┤
│  API → Unix socket → ApiServer → mpsc                        │
│       → RmuxApp::process_api_requests                        │
│       → api_dispatch::dispatch → workspace ops               │
├─────────────────────────────────────────────────────────────┤
│  Events → broadcast channel → events.stream subscribers      │
├─────────────────────────────────────────────────────────────┤
│  OSC sequences → OscScanner → OscNotification                │
│       → NotificationManager → SystemNotifier + UI badge      │
└─────────────────────────────────────────────────────────────┘
```

## Memory Budget

| State | Target |
|---|---|
| Empty window, no panes | < 20 MB |
| 1 terminal pane, shell running | < 30 MB |
| 10 terminal panes, active | < 60 MB |
| 20 terminal panes, mixed | < 100 MB |
| 20 panes + 1 browser pane | < 150 MB |

**Guardrails:**
- Scrollback capped at 10,000 lines per pane (default)
- Notification store capped at 200 entries
- Pane count warning at > 50 panes total
- OSC scanner payload capped at 4096 bytes
- Browser pane: single `wry` webview, lazy spawn
