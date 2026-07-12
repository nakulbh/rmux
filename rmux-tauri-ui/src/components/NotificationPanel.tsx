/**
 * NotificationPanel — right-side toggleable notification list.
 *
 * Matches `crates/rmux-app/src/ui/notification_panel.rs`.
 * Arbor/cmux-styled: sidebar_bg panel with 1px border left edge,
 * cards on panel_bg with 2px accent stripe for unread rows.
 * Read cards at 0.85 opacity. Newest-first with relative timestamps.
 * Clicking a row marks it read and jumps to workspace/pane.
 */

import type { Notification } from "../types";
import "../App.css";

// ── Props ──────────────────────────────────────────────────────────────────

export interface NotificationPanelProps {
  visible: boolean;
  notifications: Notification[];
  onClose: () => void;
  onMarkRead: (id: number) => void;
  onMarkAllRead: () => void;
  onClear: () => void;
}

// ── Component ──────────────────────────────────────────────────────────────

export function NotificationPanel({
  visible,
  notifications,
  onClose,
  onMarkRead,
  onMarkAllRead,
  onClear,
}: NotificationPanelProps) {
  if (!visible) return null;

  const unreadCount = notifications.filter((n) => !n.read).length;
  const isMac = navigator.platform.includes("Mac");

  return (
    <aside
      className="app-notif-panel"
      style={{
        minWidth: "var(--notif-panel-min-width)",
        maxWidth: "var(--notif-panel-max-width)",
        width: "var(--notif-panel-default-width)",
        background: "var(--sidebar-bg)",
        borderLeft: "1px solid var(--border)",
        padding: 8,
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
      }}
    >
      {/* Header */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 4,
        }}
      >
        <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)" }}>
          Notifications
        </span>
        {/* Count pill */}
        <span
          className="mono"
          style={{
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            minWidth: 14,
            height: 14,
            padding: "0 4px",
            fontSize: 9,
            color: "var(--text-muted)",
            background: "var(--panel-bg)",
            border: "1px solid var(--border)",
            borderRadius: 7,
          }}
        >
          {unreadCount}
        </span>
      </div>

      {/* Actions */}
      <div style={{ display: "flex", gap: 4, marginBottom: 6 }}>
        <ActionButton label="Mark all read" onClick={onMarkAllRead} />
        <ActionButton label="Clear" onClick={onClear} />
      </div>

      {/* Separator */}
      <div style={{ height: 1, background: "var(--border)", marginBottom: 6 }} />

      {/* List */}
      {notifications.length === 0 ? (
        <EmptyState isMac={isMac} />
      ) : (
        <div style={{ flex: 1, overflowY: "auto" }}>
          {[...notifications].reverse().map((n) => (
            <NotificationRow
              key={n.id}
              notification={n}
              onClick={() => onMarkRead(n.id)}
            />
          ))}
        </div>
      )}
    </aside>
  );
}

// ── Notification Row ───────────────────────────────────────────────────────

function NotificationRow({
  notification,
  onClick,
}: {
  notification: Notification;
  onClick: () => void;
}) {
  const { title, body, timestamp, read: isRead } = notification;
  const alpha = isRead ? 0.85 : 1;

  return (
    <div
      onClick={onClick}
      style={{
        padding: "6px 8px",
        borderRadius: 2,
        background: "var(--panel-bg)",
        border: "1px solid var(--border)",
        marginBottom: 2,
        opacity: alpha,
        cursor: "pointer",
        position: "relative",
        overflow: "hidden",
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.background = "var(--panel-active-bg)";
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = "var(--panel-bg)";
      }}
    >
      {/* Unread accent stripe (2px on left edge) */}
      {!isRead && (
        <div
          style={{
            position: "absolute",
            left: 1,
            top: 1,
            bottom: 1,
            width: 2,
            background: "var(--accent)",
          }}
        />
      )}

      {/* Title + timestamp */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "baseline",
          marginBottom: body ? 2 : 0,
        }}
      >
        <span
          style={{
            fontSize: 12,
            color: isRead ? "var(--text-muted)" : "var(--text-primary)",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
            flex: 1,
            marginRight: 8,
          }}
        >
          {title}
        </span>
        <span
          style={{
            fontSize: 10,
            color: "var(--text-disabled)",
            whiteSpace: "nowrap",
          }}
        >
          {relativeTime(timestamp)}
        </span>
      </div>

      {/* Body */}
      {body && (
        <div
          style={{
            fontSize: 10.5,
            color: "var(--text-muted)",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {body}
        </div>
      )}
    </div>
  );
}

// ── Action Button (h=22, radius 2, panel_bg + 1px border) ─────────────────

function ActionButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      style={{
        height: 22,
        padding: "0 8px",
        borderRadius: 2,
        background: "var(--panel-bg)",
        border: "1px solid var(--border)",
        color: "var(--text-primary)",
        fontSize: 11,
        cursor: "pointer",
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.background = "var(--panel-active-bg)";
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = "var(--panel-bg)";
      }}
    >
      {label}
    </button>
  );
}

// ── Empty State ────────────────────────────────────────────────────────────

function EmptyState({ isMac }: { isMac: boolean }) {
  return (
    <div
      style={{
        flex: 1,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 2,
      }}
    >
      <span style={{ fontSize: 12, color: "var(--text-muted)" }}>
        No notifications
      </span>
      <span style={{ fontSize: 10, color: "var(--text-disabled)" }}>
        {isMac ? "\u2318I to toggle" : "Ctrl+I to toggle"}
      </span>
    </div>
  );
}

// ── Relative Time ──────────────────────────────────────────────────────────

function relativeTime(isoTimestamp: string): string {
  const ms = Date.now() - new Date(isoTimestamp).getTime();
  const secs = Math.floor(ms / 1000);

  if (secs < 60) return "just now";
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}h ago`;
  return `${Math.floor(secs / 86400)}d ago`;
}
