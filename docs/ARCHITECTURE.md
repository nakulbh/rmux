# rmux — Architecture

> Detailed architecture, data flow, and module design for the rmux terminal multiplexer.

---

## System Overview

rmux is a multi-crate Rust workspace. The main binary (`rmux-app`) owns the GUI event loop and
orchestrates terminal emulation, workspace management, notifications, and the socket API.
A separate CLI binary (`rmux-cli`) communicates with the running app over a Unix socket.

```
┌─────────────────────────────────────────────────────────────────┐
│                         rmux-app                                │
│                                                                 │
│  ┌──────────────┐   ┌───────────────┐   ┌──────────────────┐   │
│  │   egui UI    │   │  Workspace    │   │  Notification    │   │
│  │   Layer      │   │  Manager      │   │  Manager         │   │
│  │              │   │               │   │                  │   │
│  │ - Root view  │   │ - Workspace[] │   │ - OSC parser     │   │
│  │ - Sidebar    │   │ - Pane tree   │   │ - Desktop notify │   │
│  │ - Term panes │   │ - Active pane │   │ - Unread state   │   │
│  │ - Browser    │   │ - Focus mgmt  │   │ - Sidebar badge  │   │
│  └──────┬───────┘   └───────┬───────┘   └────────┬─────────┘   │
│         │                   │                     │             │
│  ┌──────▼───────────────────▼─────────────────────▼──────────┐  │
│  │                   App State (app.rs)                       │  │
│  │                                                            │  │
│  │  - workspaces: Vec<Workspace>                              │  │
│  │  - active_workspace: usize                                 │  │
│  │  - notification_manager: NotificationManager               │  │
│  │  - config: Config                                          │  │
│  │  - socket_server: SocketServer                             │  │
│  └──────┬───────────────────┬─────────────────────┬──────────┘  │
│         │                   │                     │             │
│  ┌──────▼───────┐   ┌──────▼────────┐   ┌───────▼──────────┐   │
│  │ rmux-terminal│   │  rmux-api     │   │  rmux-config     │   │
│  │              │   │               │   │                  │   │
│  │ - PtyBackend │   │ - Socket srv  │   │ - Config schema  │   │
│  │ - TermState  │   │ - JSON-RPC    │   │ - Ghostty import │   │
│  │ - Renderer   │   │ - Methods     │   │ - Path resolution│   │
│  │ - Input map  │   │ - Events      │   │                  │   │
│  └──────────────┘   └───────────────┘   └──────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
         ▲
         │ Unix socket
┌────────▼────────┐
│   rmux-cli      │
│                 │
│ - clap CLI      │
│ - Socket client │
│ - JSON output   │
└─────────────────┘
```

---

## Crate Dependency Graph

```
rmux-app ──► rmux-terminal
   │              │
   ├──► rmux-api  │
   │       │      │
   │       ▼      │
   │   rmux-config│
   │              │
   └──────────────┘

rmux-cli ──► rmux-api (client-side protocol only)
```

**Rules:**
- `rmux-terminal` has NO dependency on `rmux-app` or `rmux-api`
- `rmux-api` has NO dependency on `rmux-terminal`
- `rmux-config` has NO dependency on any other rmux crate
- `rmux-app` is the only crate that wires everything together

---

## Module Deep Dive

### rmux-terminal

The terminal emulation layer. Wraps `alacritty_terminal` and `portable-pty` into a
clean API that the UI layer can consume.

#### `backend.rs` — PTY Backend

```rust
/// Manages a PTY child process and its I/O streams.
pub struct PtyBackend {
    master: Box<dyn MasterPty>,
    child: Box<dyn Child + Send>,
    reader: Option<Box<dyn Read + Send>>,
    writer: Option<Box<dyn Write + Send>>,
}

impl PtyBackend {
    /// Spawn a shell in a new PTY.
    pub fn spawn(shell: &str, cols: u16, rows: u16) -> Result<Self>;

    /// Write input bytes to the PTY (keyboard, paste).
    pub fn write(&mut self, data: &[u8]) -> Result<()>;

    /// Resize the PTY.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()>;

    /// Check if the child process is still alive.
    pub fn is_alive(&mut self) -> bool;

    /// Take the reader for async consumption.
    pub fn take_reader(&mut self) -> Option<Box<dyn Read + Send>>;
}
```

**Data flow:**
```
Keyboard → PtyBackend.write() → PTY stdin
PTY stdout → reader task → TermState.feed_bytes()
Window resize → PtyBackend.resize() → PTY resize ioctl
```

#### `state.rs` — Terminal State

```rust
/// Wraps alacritty_terminal::Term and provides a clean query API.
pub struct TermState {
    term: Term<Listener>,
    scrollback_limit: usize,
}

impl TermState {
    /// Create a new terminal state with the given dimensions.
    pub fn new(cols: usize, rows: usize, scrollback_limit: usize) -> Self;

    /// Feed raw bytes from the PTY through the VTE parser.
    pub fn feed_bytes(&mut self, data: &[u8]);

    /// Get a snapshot of the current grid for rendering.
    pub fn grid_snapshot(&self) -> GridSnapshot;

    /// Get the current cursor position.
    pub fn cursor(&self) -> CursorState;

    /// Get the scrollback offset (0 = bottom).
    pub fn scroll_offset(&self) -> usize;

    /// Scroll the viewport up/down.
    pub fn scroll(&mut self, lines: i32);

    /// Get selected text, if any.
    pub fn selection(&self) -> Option<String>;
}
```

**Key design decision:** `GridSnapshot` is an owned copy of the visible grid state.
This avoids holding a borrow on `TermState` during rendering, which would conflict
with mutable access for incoming PTY data.

```rust
/// A snapshot of the terminal grid at a point in time.
pub struct GridSnapshot {
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<Vec<Cell>>,
    pub cursor: CursorState,
    pub display_offset: usize,
}

pub struct Cell {
    pub c: char,
    pub fg: Color,
    pub bg: Color,
    pub flags: CellFlags, // bold, italic, underline, etc.
}
```

#### `renderer.rs` — Grid Renderer

```rust
/// Converts a GridSnapshot into egui paint commands.
pub struct TerminalRenderer {
    font_id: egui::FontId,
    cell_size: egui::Vec2, // monospace cell dimensions
    theme: ColorTheme,
}

impl TerminalRenderer {
    /// Render the grid snapshot into the egui UI.
    pub fn show(&self, ui: &mut egui::Ui, snapshot: &GridSnapshot, scroll_offset: usize);

    /// Calculate the optimal cell size for the given font.
    pub fn calculate_cell_size(ui: &egui::Ui, font_id: &egui::FontId) -> egui::Vec2;

    /// Convert terminal color to egui color.
    fn color_to_egui(&self, color: Color) -> egui::Color32;
}
```

**Rendering strategy:**
1. For each visible row, iterate over columns
2. For each cell, draw a filled rect (background) and a glyph (foreground)
3. Batch rects by background color to minimize draw calls
4. Use egui's `Galley` for text layout (handles monospace naturally)
5. Cursor is drawn as a rect overlay

#### `input.rs` — Input Mapping

```rust
/// Maps egui keyboard/mouse events to terminal escape sequences.
pub struct InputMapper {
    bracket_paste_mode: bool,
}

impl InputMapper {
    /// Map an egui KeyEvent to terminal bytes.
    pub fn map_key(&self, event: &egui::KeyEvent) -> Option<Vec<u8>>;

    /// Map a mouse event to terminal bytes (for mouse reporting).
    pub fn map_mouse(&self, event: &egui::Event) -> Option<Vec<u8>>;

    /// Wrap text in bracket paste escape sequences.
    pub fn paste(&self, text: &str) -> Vec<u8>;
}
```

---

### rmux-app — Workspace Management

#### Pane Tree

The pane tree is a recursive enum that describes the layout of terminal panes
within a workspace.

```rust
/// A node in the pane tree.
pub enum PaneNode {
    /// A leaf node containing a terminal pane.
    Leaf {
        id: PaneId,
        pane: TerminalPane,
    },
    /// A split node containing child panes.
    Split {
        id: SplitId,
        direction: SplitDirection,
        children: Vec<PaneNode>,
        /// Relative sizes of children (must sum to 1.0).
        sizes: Vec<f32>,
    },
}

pub enum SplitDirection {
    Horizontal, // side by side
    Vertical,   // stacked
}

/// Unique identifiers.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneId(pub u64);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SplitId(pub u64);
```

**Operations on the pane tree:**
- `insert_split(parent, direction, new_pane)` — split a leaf into two
- `remove_pane(id)` — remove a leaf, collapse parent if only one child remains
- `resize_split(id, ratio)` — adjust child sizes
- `find_pane(id)` — look up a pane by ID
- `focus_next(direction)` — move focus to adjacent pane
- `iter_panes()` — iterate all leaf panes

#### Workspace

```rust
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    pub root: PaneNode,
    pub active_pane: PaneId,
    pub created_at: Instant,
}

impl Workspace {
    pub fn new(name: String) -> Self;
    pub fn split_right(&mut self, pane_id: PaneId) -> Result<PaneId>;
    pub fn split_down(&mut self, pane_id: PaneId) -> Result<PaneId>;
    pub fn close_pane(&mut self, pane_id: PaneId) -> Result<()>;
    pub fn focus_pane(&mut self, pane_id: PaneId);
    pub fn pane_count(&self) -> usize;
    pub fn active_pane(&self) -> &TerminalPane;
    pub fn active_pane_mut(&mut self) -> &mut TerminalPane;
}
```

#### WorkspaceManager

```rust
pub struct WorkspaceManager {
    workspaces: Vec<Workspace>,
    active_index: usize,
    next_id: u64,
}

impl WorkspaceManager {
    pub fn create_workspace(&mut self, name: String) -> WorkspaceId;
    pub fn close_workspace(&mut self, id: WorkspaceId) -> Result<()>;
    pub fn rename_workspace(&mut self, id: WorkspaceId, name: String);
    pub fn switch_to(&mut self, index: usize);
    pub fn active(&self) -> &Workspace;
    pub fn active_mut(&mut self) -> &mut Workspace;
    pub fn list(&self) -> Vec<WorkspaceInfo>;
}
```

---

### rmux-app — Notification System

#### OSC Parser

```rust
/// Parse terminal output for notification escape sequences.
pub struct OscParser {
    buffer: Vec<u8>,
    state: ParserState,
}

impl OscParser {
    /// Feed bytes from terminal output. Returns notifications if found.
    pub fn feed(&mut self, data: &[u8]) -> Vec<Notification>;
}

pub struct Notification {
    pub id: NotificationId,
    pub title: String,
    pub body: String,
    pub source_pane: PaneId,
    pub created_at: Instant,
    pub read: bool,
}
```

**Supported OSC sequences:**
- `ESC ] 9 ; message BEL` — simple OSC 9
- `ESC ] 99 ; i=1;e=1;d=0;p=title:body ESC \` — rich OSC 99
- `ESC ] 777 ; notify ; Title ; Body BEL` — legacy OSC 777

#### NotificationManager

```rust
pub struct NotificationManager {
    notifications: Vec<Notification>,
    desktop_enabled: bool,
}

impl NotificationManager {
    pub fn push(&mut self, notification: Notification);
    pub fn mark_read(&mut self, id: NotificationId);
    pub fn clear(&mut self, id: NotificationId);
    pub fn clear_all(&mut self);
    pub fn unread_count(&self) -> usize;
    pub fn list_unread(&self) -> &[Notification];
    pub fn latest_unread(&self) -> Option<&Notification>;
}
```

---

### rmux-api — Socket Server

#### Protocol

Newline-delimited JSON-RPC over Unix socket.

**Request:**
```json
{"id": "req-1", "method": "workspace.list", "params": {}}
```

**Response:**
```json
{"id": "req-1", "ok": true, "result": {"workspaces": [...]}}
```

**Error:**
```json
{"id": "req-1", "ok": false, "error": {"code": -1, "message": "Not found"}}
```

#### Method Registry

```rust
pub struct MethodRegistry {
    handlers: HashMap<String, Box<dyn MethodHandler>>,
}

pub trait MethodHandler: Send + Sync {
    fn handle(&self, params: Value, state: &AppState) -> Result<Value, ApiError>;
}
```

**Registered methods:**

| Method | Phase | Description |
|---|---|---|
| `system.ping` | 3 | Health check |
| `system.capabilities` | 3 | List supported methods |
| `system.identify` | 3 | App version, PID |
| `workspace.list` | 3 | List all workspaces |
| `workspace.create` | 3 | Create new workspace |
| `workspace.select` | 3 | Switch to workspace |
| `workspace.close` | 3 | Close workspace |
| `surface.list` | 3 | List panes in workspace |
| `surface.split` | 3 | Split a pane |
| `surface.focus` | 3 | Focus a pane |
| `surface.send_text` | 3 | Type text into pane |
| `surface.send_key` | 3 | Send key event |
| `notification.create` | 3 | Create notification |
| `notification.list` | 3 | List notifications |
| `notification.clear` | 3 | Clear notification |
| `events.stream` | 3 | Stream real-time events |

---

### rmux-config — Configuration

#### Config Schema

```rust
#[derive(Serialize, Deserialize)]
pub struct Config {
    pub app: AppConfig,
    pub terminal: TerminalConfig,
    pub browser: BrowserConfig,
    pub notifications: NotificationConfig,
    pub sidebar: SidebarConfig,
    pub shortcuts: ShortcutsConfig,
    pub automation: AutomationConfig,
}

#[derive(Serialize, Deserialize)]
pub struct TerminalConfig {
    pub shell: Option<String>,           // default: $SHELL or /bin/sh
    pub font_family: String,             // default: "monospace"
    pub font_size: f32,                  // default: 14.0
    pub max_scrollback_lines: usize,     // default: 10000
    pub copy_on_select: bool,            // default: false
    pub cursor_style: CursorStyle,       // default: Block
}

#[derive(Serialize, Deserialize)]
pub struct AutomationConfig {
    pub socket_control_mode: SocketMode, // default: CmuxProcessesOnly
    pub socket_path: Option<String>,     // default: auto
}
```

**Config file locations:**
- Linux: `~/.config/rmux/rmux.json`
- macOS: `~/Library/Application Support/rmux/rmux.json`
- Windows: `%APPDATA%\rmux\rmux.json`

**Ghostty config import (optional):**
- `~/.config/ghostty/config` — read for themes, fonts, colors
- Map Ghostty keys to rmux equivalents where possible

---

## Memory Management Strategy

### Surface Hibernation

When a terminal pane is not visible (hidden behind other workspaces or minimized),
stop rendering it but keep the PTY alive.

```rust
pub enum PaneState {
    Active,     // visible, rendering, accepting input
    Background, // visible but not focused
    Hibernated, // not visible, PTY alive, no rendering
}

impl TerminalPane {
    pub fn hibernate(&mut self) {
        // Drop the GridSnapshot cache
        // Stop the render timer
        // Keep PtyBackend alive
    }

    pub fn wake(&mut self) {
        // Request a full redraw
        // Resume render timer
    }
}
```

### Scrollback Limits

- Default max: 10,000 lines
- Configurable per-pane
- When limit is reached, oldest lines are discarded
- Session restore saves only the visible + N most recent lines

### Browser Pane Lifecycle

- Browser webview is created lazily (only when a browser pane is first opened)
- Hidden browser panes are suspended (wry supports this)
- Only one active browser pane renders at a time

---

## Thread Model

```
Main thread (egui event loop)
  ├── UI rendering
  ├── Event handling
  └── Workspace management

tokio runtime (multi-threaded)
  ├── PTY reader task (per pane)
  │   └── reads PTY → feeds TermState
  ├── Socket server task
  │   └── accepts connections → dispatches methods
  ├── Notification task
  │   └── desktop notification delivery
  └── Browser IPC task (if browser pane active)
      └── wry webview communication
```

**Key constraint:** The egui UI runs on the main thread. All I/O happens on the tokio runtime.
Communication between UI and I/O tasks uses `tokio::sync::mpsc` channels.

```rust
// PTY → UI channel
let (pty_tx, pty_rx) = mpsc::channel::<PtyEvent>(256);

// UI → PTY channel (for input)
let (input_tx, input_rx) = mpsc::channel::<InputEvent>(64);

// Socket → App channel
let (api_tx, api_rx) = mpsc::channel::<ApiRequest>(32);
```

---

## Error Recovery

| Scenario | Recovery |
|---|---|
| PTY child exits | Mark pane as "exited", show exit code, allow restart |
| Socket client disconnects | Clean up client state, continue serving |
| Config parse error | Log warning, use defaults |
| Browser webview crash | Show error in pane, allow reload |
| Notification delivery failure | Log error, don't crash |
| OOM (pane count too high) | Warn user, refuse to create new panes |

---

## Future Considerations

- **GPU rendering**: If egui's CPU rendering becomes a bottleneck for many panes,
  consider a custom wgpu-based terminal renderer
- **Plugin system**: WASM-based plugins for custom commands and integrations
- **tmux integration**: Attach to remote tmux sessions (like cmux's tmux-compat)
- **Lua scripting**: Embed a Lua runtime for user-defined automation
- **Themes**: Load themes from Ghostty, Alacritty, or custom JSON format
