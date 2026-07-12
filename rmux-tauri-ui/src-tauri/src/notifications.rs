//! In-memory notification store for the Tauri backend.
//!
//! Stores notifications, tracks read/unread state, and provides
//! desktop notification emission.

use std::time::SystemTime;

/// Maximum number of stored notifications.
#[allow(dead_code)]
const MAX_NOTIFICATIONS: usize = 200;

/// A single notification.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Notification {
    pub id: u64,
    pub pane_id: Option<u64>,
    pub workspace_id: Option<u64>,
    pub title: String,
    pub body: Option<String>,
    pub timestamp: SystemTime,
    pub read: bool,
}

/// Stores notifications and tracks read/unread state.
pub struct NotificationManager {
    notifications: Vec<Notification>,
    #[allow(dead_code)]
    next_id: u64,
}

#[allow(dead_code)]
impl NotificationManager {
    pub fn new() -> Self {
        Self { notifications: Vec::new(), next_id: 0 }
    }

    /// Add a notification and return its id.
    pub fn add(
        &mut self,
        title: String,
        body: Option<String>,
        pane_id: Option<u64>,
        workspace_id: Option<u64>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

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

    /// All stored notifications, oldest first.
    pub fn list(&self) -> &[Notification] {
        &self.notifications
    }

    /// Number of unread notifications.
    pub fn unread_count(&self) -> usize {
        self.notifications.iter().filter(|n| !n.read).count()
    }

    /// Mark a notification as read.
    pub fn mark_read(&mut self, id: u64) {
        if let Some(notification) = self.notifications.iter_mut().find(|n| n.id == id) {
            notification.read = true;
        }
    }

    /// Mark all notifications as read.
    pub fn mark_all_read(&mut self) {
        for notification in &mut self.notifications {
            notification.read = true;
        }
    }

    /// Remove all notifications.
    pub fn clear(&mut self) {
        self.notifications.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_list() {
        let mut mgr = NotificationManager::new();
        let first = mgr.add("first".into(), None, Some(1), Some(10));
        let second = mgr.add("second".into(), Some("body".into()), None, None);

        assert!(first < second);
        let list = mgr.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].title, "first");
        assert_eq!(list[1].title, "second");
    }

    #[test]
    fn test_unread_count() {
        let mut mgr = NotificationManager::new();
        mgr.add("a".into(), None, None, None);
        mgr.add("b".into(), None, None, None);
        assert_eq!(mgr.unread_count(), 2);
        mgr.mark_read(0);
        assert_eq!(mgr.unread_count(), 1);
    }

    #[test]
    fn test_mark_all_read_and_clear() {
        let mut mgr = NotificationManager::new();
        mgr.add("a".into(), None, None, None);
        mgr.add("b".into(), None, None, None);
        mgr.mark_all_read();
        assert_eq!(mgr.unread_count(), 0);
        mgr.clear();
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_cap_evicts_oldest() {
        let mut mgr = NotificationManager::new();
        for i in 0..205 {
            mgr.add(format!("n{i}"), None, None, None);
        }
        let list = mgr.list();
        assert_eq!(list.len(), 200);
        assert_eq!(list[0].id, 5);
    }
}
