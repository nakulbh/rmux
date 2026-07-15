# 16. Notifications

Notifications live in memory. Some come from terminal OSC. Some come from CLI.

Files: `notifications/manager.rs`, `ui/notification_panel.rs`, `app.rs`.

## Manager struct

NotificationManager stores rows and owns desktop notifier.

```rust
/// Stores notifications, tracks read/unread state, and emits desktop
/// notifications through a [`DesktopNotifier`].
///
/// Notifications are kept in insertion order (newest last) and capped at
/// 200 entries — the oldest entries are evicted first.
pub struct NotificationManager {
    /// Stored notifications, newest last.
    notifications: Vec<Notification>,
    /// Next id to assign.
    next_id: u64,
    /// Desktop notification sink.
    notifier: Box<dyn DesktopNotifier>,
}
```

Cap protects memory:

```rust
const MAX_NOTIFICATIONS: usize = 200;
```

## Desktop notifier

App creates real OS notifier at startup.

```rust
pub fn with_system_notifier() -> Self {
    Self::new(Box::new(SystemNotifier))
}
```

Boxed trait means tests can use fake notifier.

## Add notification

`add()` assigns id, emits desktop notification, stores row.

```rust
pub fn add(
    &mut self,
    title: String,
    body: Option<String>,
    pane_id: Option<u64>,
    workspace_id: Option<u64>,
) -> u64 {
    let id = self.next_id;
    self.next_id += 1;

    self.notifier.notify(&title, body.as_deref());

    self.notifications.push(Notification {
        id,
        pane_id,
        workspace_id,
        title,
        body,
        timestamp: SystemTime::now(),
        read: false,
    });
```

Trim old rows:

```rust
if self.notifications.len() > MAX_NOTIFICATIONS {
    let excess = self.notifications.len() - MAX_NOTIFICATIONS;
    self.notifications.drain(..excess);
}

id
```

## Unread counts

Global count:

```rust
pub fn unread_count(&self) -> usize {
    self.notifications.iter().filter(|n| !n.read).count()
}
```

Workspace count:

```rust
pub fn unread_count_for_workspace(&self, ws: u64) -> usize {
    self.notifications.iter().filter(|n| !n.read && n.workspace_id == Some(ws)).count()
}
```

Sidebar uses per-workspace count. Top bar and panel use global count.

## Mark read

One row:

```rust
pub fn mark_read(&mut self, id: u64) {
    if let Some(notification) = self.notifications.iter_mut().find(|n| n.id == id) {
        notification.read = true;
    }
}
```

`mark_all_read()` loops all rows and sets `read = true`. `clear()` empties list.

## OSC notifications

TerminalPane scans shell output. App collects parsed notifications in `update()`.

```rust
let osc_notifications = self.workspace_manager.process_all_panes();
for (workspace_id, pane_id, notification) in osc_notifications {
    self.add_pane_notification(workspace_id, pane_id, notification);
}
```

App stores and publishes event:

```rust
fn add_pane_notification(
    &mut self,
    workspace_id: u64,
    pane_id: u64,
    notification: rmux_terminal::OscNotification,
) {
    let id = self.notifications.add(
        notification.title.clone(),
        notification.body.clone(),
        Some(pane_id),
        Some(workspace_id),
    );
```

Socket event:

```rust
self.publish_event(
    "notification",
    json!({
        "id": id,
        "title": notification.title,
        "body": notification.body,
        "pane_id": pane_id,
        "workspace_id": workspace_id,
    }),
);
```

## NotificationPanel rendering

Right panel lists newest first.

```rust
egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
    ui.spacing_mut().item_spacing.y = 2.0_f32;
    for notification in notifications.list().iter().rev() {
        if render_row(ui, notification).clicked() {
            clicked = Some((notification.id, notification.workspace_id, notification.pane_id));
        }
    }
});
```

Click marks read and jumps:

```rust
if let Some((id, workspace_id, pane_id)) = clicked {
    notifications.mark_read(id);
    jump_to(manager, workspace_id, pane_id);
}
```

Unread rows get accent stripe:

```rust
if !notification.read {
    let stripe = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 1.0_f32, rect.top() + 1.0_f32),
        egui::pos2(rect.left() + 3.0_f32, rect.bottom() - 1.0_f32),
    );
    painter.rect_filled(stripe, egui::CornerRadius::ZERO, palette.accent);
}
```

← **Prev: [15 — Shortcuts](15-shortcuts.md)**

→ **Next: [17 — API Server](17-api-server.md)**
