# 13. UI topbar and sidebar

Top bar = window chrome. Sidebar = workspace cards. Notification panel = right list.

Files: `top_bar.rs`, `sidebar.rs`, `notification_panel.rs`.

## Top bar

Top comment:

```rust
//! Top chrome bar: sidebar toggle, centered workspace title, notification bell.
//!
//! 34px `chrome_bg` strip with a 1px `chrome_border` hairline along its
//! bottom edge (see `docs/UI_REDESIGN.md` §D).
```

macOS traffic-light offset:

```rust
fn left_offset() -> f32 {
    if cfg!(target_os = "macos") { 76.0_f32 } else { 12.0_f32 }
}
```

`show()` renders whole strip:

```rust
pub fn show(
    ctx: &egui::Context,
    manager: &WorkspaceManager,
    notifications: &NotificationManager,
    sidebar_visible: &mut bool,
    notification_panel_visible: &mut bool,
    right_sidebar_visible: &mut bool,
    settings_open: &mut bool,
) {
    let p = theme::palette();
    egui::TopBottomPanel::top("rmux_top_bar")
        .exact_height(metrics::TOP_BAR_HEIGHT)
        .frame(egui::Frame::default().fill(p.chrome_bg))
```

Center title uses active workspace:

```rust
let ws = manager.active();
let name_galley = ui.painter().layout_no_wrap(
    ws.name.clone(),
    FontId::proportional(14.0_f32),
    p.text_primary,
);
let panes = ws.pane_count();
```

## SidebarView

Sidebar owns visible flag, right panel flag, rename buffer.

```rust
pub struct SidebarView {
    /// Whether the sidebar is currently visible.
    pub visible: bool,
    /// Whether the right-side notification panel is currently visible.
    pub right_sidebar_visible: bool,
    /// Index of the card currently being renamed (None if not renaming).
    editing_index: Option<usize>,
    /// Temporary buffer for the rename text edit.
    edit_buffer: String,
}
```

Toggle:

```rust
pub fn toggle(&mut self) {
    self.visible = !self.visible;
    tracing::debug!(visible = self.visible, "Sidebar toggled");
}
```

## Sidebar render

`show()` creates egui left panel.

```rust
pub fn show(
    &mut self,
    ctx: &egui::Context,
    manager: &mut WorkspaceManager,
    notifications: &NotificationManager,
) -> bool {
    if !self.visible {
        return false;
    }

    egui::SidePanel::left("rmux_sidebar")
        .frame(egui::Frame::default().fill(p().sidebar_bg).inner_margin(egui::Margin::same(8)))
        .min_width(crate::ui::theme::metrics::SIDEBAR_MIN_WIDTH)
        .max_width(crate::ui::theme::metrics::SIDEBAR_MAX_WIDTH)
        .default_width(crate::ui::theme::metrics::SIDEBAR_DEFAULT_WIDTH)
        .resizable(true)
        .show(ctx, |ui| self.render_sidebar(ui, manager, notifications))
        .inner
}
```

Return value = new workspace button clicked. App creates terminal, not sidebar.

## Card data snapshot

Sidebar clones display data before drawing cards.

```rust
struct TabData {
    /// Workspace id.
    id: WorkspaceId,
    /// Display name.
    name: String,
    /// Number of panes.
    pane_count: usize,
    /// Status text set via `sidebar.set_status`.
    status: Option<String>,
    /// Progress in `0.0..=1.0` set via `sidebar.set_progress`.
    progress: Option<f32>,
```

Why snapshot? Rust borrowing. Drawing reads data, clicks mutate manager.

## NotificationPanel

Right panel state:

```rust
pub struct NotificationPanel {
    /// Whether the panel is currently visible.
    pub visible: bool,
}
```

Render guard:

```rust
pub fn show(
    &mut self,
    ctx: &egui::Context,
    notifications: &mut NotificationManager,
    manager: &mut WorkspaceManager,
) {
    if !self.visible {
        return;
    }
```

Right side panel:

```rust
egui::SidePanel::right("rmux_notification_panel")
    .frame(
        egui::Frame::default()
            .fill(palette.sidebar_bg)
            .inner_margin(egui::Margin::same(8)),
    )
    .min_width(240.0_f32)
    .max_width(340.0_f32)
    .default_width(280.0_f32)
```

Actions mutate notifications:

```rust
if action_button(ui, &palette, "Mark all read").clicked() {
    notifications.mark_all_read();
}
if action_button(ui, &palette, "Clear").clicked() {
    notifications.clear();
}
```

Click row = mark read, jump to workspace/pane.

```rust
if let Some((id, workspace_id, pane_id)) = clicked {
    notifications.mark_read(id);
    jump_to(manager, workspace_id, pane_id);
}
```

← **Prev: [12 — UI Theme](12-ui-theme.md)**

→ **Next: [14 — Terminal Pane Widget](14-terminal-pane-widget.md)**
