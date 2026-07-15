# 15. Shortcuts

Shortcuts map key chords to app actions. Registry stores map. Handler dispatches action.

Files: `shortcuts.rs`, `shortcut_handler.rs`.

## ShortcutAction

Each action is app-level command.

```rust
pub enum ShortcutAction {
    /// Quit the application.
    Quit,
    /// Increase terminal font size.
    FontSizeUp,
    /// Decrease terminal font size.
    FontSizeDown,
    /// Reset terminal font size to default.
    FontSizeReset,
    /// Copy selected text from the active terminal.
    Copy,
    /// Open the find bar.
    Find,
```

Pane and workspace actions:

```rust
/// Toggle sidebar visibility.
ToggleSidebar,
/// Toggle notification panel visibility.
ToggleNotifications,
/// Create a new workspace.
NewWorkspace,
/// Split active pane to the right.
SplitRight,
/// Split active pane downward.
SplitDown,
/// Close the active pane.
ClosePane,
```

Image paste isn't `Cmd+V` because egui-winit swallows image clipboard paste.

```rust
/// Paste a clipboard image into the active terminal (writes it to a
/// temp PNG and injects the path, like iTerm2/Kitty/WezTerm).
///
/// This is intentionally **not** bound to Cmd+V: egui-winit's
/// `is_paste_command()` intercepts any Cmd+V-containing chord at the
/// windowing layer and always calls `Clipboard::get_text()`
PasteImage,
```

## ShortcutRegistry

HashMap from `(Modifiers, Key)` to `ShortcutAction`.

```rust
pub struct ShortcutRegistry {
    /// Exact-match lookups: (modifiers, key) → action.
    exact: HashMap<(Modifiers, Key), ShortcutAction>,
}

impl ShortcutRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self { exact: HashMap::new() }
    }

    /// Register a shortcut.
    pub fn register(&mut self, modifiers: Modifiers, key: Key, action: ShortcutAction) {
        self.exact.insert((modifiers, key), action);
    }
```

Lookup:

```rust
pub fn lookup(&self, modifiers: Modifiers, key: Key) -> Option<ShortcutAction> {
    self.exact.get(&(modifiers, key)).copied()
}
```

## Built-ins

Default registry registers built-ins once at startup.

```rust
impl Default for ShortcutRegistry {
    /// Populate the registry with all built-in shortcuts.
    ///
    /// Platform-specific modifier handling is applied at *lookup* time
    /// (see `RmuxApp::handle_keyboard_shortcuts`), so the registry stores
    /// shortcuts with the **canonical** modifier (Command on macOS,
    /// Control on Linux/Windows).
    fn default() -> Self {
        let mut reg = Self::new();
```

Examples:

```rust
reg.register(cmd_ctrl(), Key::Q, ShortcutAction::Quit);
reg.register(cmd_ctrl(), Key::F, ShortcutAction::Find);
reg.register(cmd_ctrl(), Key::N, ShortcutAction::NewWorkspace);
reg.register(cmd_ctrl(), Key::D, ShortcutAction::SplitRight);
```

## Platform differences

macOS: Cmd means app shortcut. Ctrl means terminal control char.

Linux and Windows: Ctrl means app shortcut.

```rust
let mod_active = if cfg!(target_os = "macos") {
    modifiers.command && !modifiers.ctrl
} else {
    modifiers.command || modifiers.ctrl
};
```

Modifier normalization:

```rust
fn normalize_lookup_mods(mut modifiers: egui::Modifiers) -> egui::Modifiers {
    modifiers.mac_cmd = false;
    if !cfg!(target_os = "macos") && modifiers.command {
        modifiers.command = false;
        modifiers.ctrl = true;
    }
    modifiers
}
```

## Dispatch

`handle_keyboard_shortcuts()` scans input events.

```rust
for event in &input.events {
    let egui::Event::Key { key, pressed: true, modifiers, .. } = event else {
        continue;
    };

    let lookup_mods = normalize_lookup_mods(*modifiers);

    let Some(action) = self.shortcut_registry.lookup(lookup_mods, *key) else {
        continue;
    };
```

Text widgets can keep keys. Some actions still pass through.

```rust
if ctx.wants_keyboard_input() && !should_dispatch_when_text_focused(action) {
    continue;
}
```

Always-active actions:

```rust
fn should_dispatch_when_text_focused(action: ShortcutAction) -> bool {
    matches!(
        action,
        ShortcutAction::Quit
            | ShortcutAction::Copy
            | ShortcutAction::FontSizeUp
            | ShortcutAction::FontSizeDown
            | ShortcutAction::FontSizeReset
            | ShortcutAction::ClearScreen
            | ShortcutAction::ClearScrollback
            | ShortcutAction::PasteImage
    )
}
```

Dispatch uses match:

```rust
ShortcutAction::SplitRight => {
    match self.split_active_with_terminal(SplitDirection::Horizontal) {
        Ok(pane_id) => tracing::info!(pane_id, "Split right"),
        Err(e) => tracing::warn!("Split right failed: {e}"),
    }
}
```

← **Prev: [14 — Terminal Pane Widget](14-terminal-pane-widget.md)**

→ **Next: [16 — Notifications](16-notifications.md)**
