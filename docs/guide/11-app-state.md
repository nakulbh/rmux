# 11. App state

RmuxApp owns app brain. One struct, all top-level systems.

File: `crates/rmux-app/src/app.rs`.

Top comment:

```rust
//! Application state and main egui rendering logic.
//!
//! The `RmuxApp` struct owns the top-level application state including the
//! workspace manager, sidebar view, notification manager, and the socket
//! API channel endpoints. It implements `eframe::App` to drive the UI.
```

## Main struct

Real fields:

```rust
pub struct RmuxApp {
    /// Manages all workspaces, panes, and splits.
    pub(crate) workspace_manager: WorkspaceManager,
    /// The sidebar view for workspace tab navigation.
    pub(crate) sidebar: SidebarView,
    /// Stores notifications and emits desktop notifications.
    pub(crate) notifications: NotificationManager,
    /// The right-side notification list panel.
    pub(crate) notification_panel: NotificationPanel,
    /// The floating settings panel (terminal theme picker, etc).
    pub(crate) settings_panel: SettingsPanel,
    /// The current terminal font size (shared by all panes).
    pub(crate) font_size: f32,
    /// The current terminal color theme (shared by all panes).
    pub(crate) terminal_theme: rmux_terminal::NamedTheme,
```

Mental model:

```text
RmuxApp -> WorkspaceManager -> workspaces/panes/splits
RmuxApp -> UI panel state
RmuxApp -> NotificationManager
RmuxApp -> API channels
RmuxApp -> ShortcutRegistry
```

## Startup

`new()` builds default state, starts socket API, spawns first terminal.

```rust
pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
    let channels = api::start_server();
    let font_size = DEFAULT_FONT_SIZE;
    let mut app = Self {
        workspace_manager: WorkspaceManager::new(),
        sidebar: SidebarView::new(),
        notifications: NotificationManager::with_system_notifier(),
        notification_panel: NotificationPanel::new(),
        settings_panel: SettingsPanel::new(),
        font_size,
        terminal_theme: rmux_terminal::NamedTheme::default(),
        last_copied_text: None,
        api_request_rx: channels.request_rx,
        api_event_tx: channels.event_tx,
        last_active_workspace: 0,
        shortcut_registry: crate::shortcuts::ShortcutRegistry::default(),
    };
```

First pane gets terminal:

```rust
let pane_id = app.workspace_manager.active().active_pane;
attach_terminal(&mut app.workspace_manager, pane_id, font_size, app.terminal_theme);
app.last_active_workspace = app.workspace_manager.active().id.0;
```

## eframe::App

egui app = type implementing `eframe::App`.

```rust
impl eframe::App for RmuxApp {
    /// Called each frame to update the UI.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
```

`update()` = frame loop. Each frame:

1. Apply theme.
2. Drain PTY output.
3. Close exited panes.
4. Drain API requests.
5. Request repaint.
6. Render panels.
7. Handle shortcuts.
8. Emit workspace change.

Real start:

```rust
crate::ui::theme::Theme::dark().apply(ctx);
let osc_notifications = self.workspace_manager.process_all_panes();
for (workspace_id, pane_id, notification) in osc_notifications {
    self.add_pane_notification(workspace_id, pane_id, notification);
}
```

Terminal output may arrive anytime. UI must keep repainting.

## Render order

egui panel order matters. Top/bottom first. Side panels next. Center last.

```rust
crate::ui::top_bar::show(
    ctx,
    &self.workspace_manager,
    &self.notifications,
    &mut self.sidebar.visible,
    &mut self.notification_panel.visible,
    &mut self.sidebar.right_sidebar_visible,
    &mut self.settings_panel.open,
);
crate::ui::status_bar::show(ctx, &self.workspace_manager, &self.notifications);
```

Sidebar asks for new workspace, app creates it:

```rust
let create_requested =
    self.sidebar.show(ctx, &mut self.workspace_manager, &self.notifications);
if create_requested {
    let count = self.workspace_manager.workspace_count() + 1;
    let ws = self.create_workspace_with_terminal(format!("Workspace {count}"));
    tracing::info!(workspace_id = ws, "Created workspace via sidebar button");
}
```

Center panel renders pane tree:

```rust
fn render_workspace(&mut self, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let zoomed = self.workspace_manager.active().zoomed_pane;
        workspace_view::render_pane_tree(ui, &mut self.workspace_manager, zoomed);
    });
}
```

## API inside frame

Socket server runs elsewhere. Main UI thread mutates app state.

```rust
fn process_api_requests(&mut self) {
    while let Ok(envelope) = self.api_request_rx.try_recv() {
        tracing::debug!(method = %envelope.method, "handling API request");
        let result = crate::api_dispatch::dispatch(self, &envelope.method, envelope.params);
        let _ = envelope.respond.send(result);
    }
}
```

`try_recv()` doesn't block UI.

## Terminal attach

New pane starts as layout node. `attach_terminal()` puts shell inside it.

```rust
fn attach_terminal(
    manager: &mut WorkspaceManager,
    pane_id: PaneId,
    font_size: f32,
    named_theme: rmux_terminal::NamedTheme,
) {
    match TerminalPane::spawn(INITIAL_COLS, INITIAL_ROWS, font_size) {
        Ok(mut terminal) => {
            terminal.set_theme(rmux_terminal::TerminalTheme::default().named(named_theme));
            manager.active_mut().set_terminal(pane_id, terminal);
        }
        Err(e) => tracing::error!(pane_id = pane_id.0, "Failed to spawn terminal pane: {e}"),
    }
}
```

Spawn failure logs error. App still lives.

← **Prev: [10 — Workspace Model](10-workspace-model.md)**

→ **Next: [12 — UI Theme](12-ui-theme.md)**
