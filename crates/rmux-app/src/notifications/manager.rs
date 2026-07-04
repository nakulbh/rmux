//! In-memory notification store with desktop notification emission.

use std::time::SystemTime;

use super::{DesktopNotifier, Notification, SystemNotifier};

/// Maximum number of stored notifications; the oldest are dropped beyond this.
const MAX_NOTIFICATIONS: usize = 200;

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

impl NotificationManager {
    /// Create a manager that emits desktop notifications through `notifier`.
    #[must_use]
    pub fn new(notifier: Box<dyn DesktopNotifier>) -> Self {
        Self { notifications: Vec::new(), next_id: 0, notifier }
    }

    /// Create a manager backed by the real OS notification system.
    #[must_use]
    pub fn with_system_notifier() -> Self {
        Self::new(Box::new(SystemNotifier))
    }

    /// Add a notification and emit it to the desktop; returns its id.
    ///
    /// `pane_id` / `workspace_id` are the raw inner values of `PaneId` /
    /// `WorkspaceId`; `None` marks an external (CLI-created) notification.
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

        if self.notifications.len() > MAX_NOTIFICATIONS {
            let excess = self.notifications.len() - MAX_NOTIFICATIONS;
            self.notifications.drain(..excess);
        }

        id
    }

    /// All stored notifications, oldest first (newest last).
    #[must_use]
    pub fn list(&self) -> &[Notification] {
        &self.notifications
    }

    /// Number of unread notifications across all workspaces.
    #[must_use]
    pub fn unread_count(&self) -> usize {
        self.notifications.iter().filter(|n| !n.read).count()
    }

    /// Number of unread notifications belonging to the given workspace.
    #[must_use]
    pub fn unread_count_for_workspace(&self, ws: u64) -> usize {
        self.notifications.iter().filter(|n| !n.read && n.workspace_id == Some(ws)).count()
    }

    /// Mark the notification with the given id as read (no-op if absent).
    pub fn mark_read(&mut self, id: u64) {
        if let Some(notification) = self.notifications.iter_mut().find(|n| n.id == id) {
            notification.read = true;
        }
    }

    /// Mark all stored notifications as read.
    pub fn mark_all_read(&mut self) {
        for notification in &mut self.notifications {
            notification.read = true;
        }
    }

    /// Remove all stored notifications.
    pub fn clear(&mut self) {
        self.notifications.clear();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    /// Shared record of `(title, body)` pairs passed to the fake notifier.
    type RecordedCalls = Arc<Mutex<Vec<(String, Option<String>)>>>;

    /// Recording fake that captures notify calls instead of firing real
    /// desktop notifications.
    struct RecordingNotifier {
        calls: RecordedCalls,
    }

    impl DesktopNotifier for RecordingNotifier {
        fn notify(&self, title: &str, body: Option<&str>) {
            self.calls.lock().unwrap().push((title.to_owned(), body.map(str::to_owned)));
        }
    }

    fn manager_with_recorder() -> (NotificationManager, RecordedCalls) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let manager =
            NotificationManager::new(Box::new(RecordingNotifier { calls: calls.clone() }));
        (manager, calls)
    }

    #[test]
    fn test_add_assigns_monotonic_ids_and_lists_newest_last() {
        let (mut manager, _calls) = manager_with_recorder();
        let first = manager.add("first".into(), None, Some(1), Some(10));
        let second = manager.add("second".into(), Some("body".into()), None, None);

        assert!(first < second);
        let list = manager.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].title, "first");
        assert_eq!(list[0].pane_id, Some(1));
        assert_eq!(list[0].workspace_id, Some(10));
        assert_eq!(list[1].title, "second");
        assert_eq!(list[1].body.as_deref(), Some("body"));
        assert_eq!(list[1].pane_id, None);
    }

    #[test]
    fn test_unread_counts_global_and_per_workspace() {
        let (mut manager, _calls) = manager_with_recorder();
        let a = manager.add("a".into(), None, None, Some(1));
        manager.add("b".into(), None, None, Some(1));
        manager.add("c".into(), None, None, Some(2));
        manager.add("d".into(), None, None, None);

        assert_eq!(manager.unread_count(), 4);
        assert_eq!(manager.unread_count_for_workspace(1), 2);
        assert_eq!(manager.unread_count_for_workspace(2), 1);
        assert_eq!(manager.unread_count_for_workspace(99), 0);

        manager.mark_read(a);
        assert_eq!(manager.unread_count(), 3);
        assert_eq!(manager.unread_count_for_workspace(1), 1);
    }

    #[test]
    fn test_mark_read_unknown_id_is_noop() {
        let (mut manager, _calls) = manager_with_recorder();
        manager.add("a".into(), None, None, None);
        manager.mark_read(12345);
        assert_eq!(manager.unread_count(), 1);
    }

    #[test]
    fn test_mark_all_read_and_clear() {
        let (mut manager, _calls) = manager_with_recorder();
        manager.add("a".into(), None, None, None);
        manager.add("b".into(), None, None, None);

        manager.mark_all_read();
        assert_eq!(manager.unread_count(), 0);
        assert_eq!(manager.list().len(), 2);

        manager.clear();
        assert!(manager.list().is_empty());
        assert_eq!(manager.unread_count(), 0);
    }

    #[test]
    fn test_cap_evicts_oldest_beyond_200() {
        let (mut manager, _calls) = manager_with_recorder();
        for i in 0..205 {
            manager.add(format!("n{i}"), None, None, None);
        }

        let list = manager.list();
        assert_eq!(list.len(), 200);
        // The five oldest (ids 0..=4) were evicted.
        assert_eq!(list[0].id, 5);
        assert_eq!(list[0].title, "n5");
        assert_eq!(list[199].id, 204);
    }

    #[test]
    fn test_notifier_called_exactly_once_per_add_with_args() {
        let (mut manager, calls) = manager_with_recorder();
        manager.add("Title".into(), Some("Body".into()), Some(7), Some(3));
        manager.add("NoBody".into(), None, None, None);

        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], ("Title".to_owned(), Some("Body".to_owned())));
        assert_eq!(calls[1], ("NoBody".to_owned(), None));
    }
}
