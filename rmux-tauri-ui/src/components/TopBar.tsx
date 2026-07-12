/**
 * TopBar — 34px chrome strip with sidebar toggle, workspace title, and notification bell.
 *
 * Matches `crates/rmux-app/src/ui/top_bar.rs`.
 * chrome_bg fill with a 1px chrome_border hairline at the bottom edge.
 * Left: hamburger (☰) toggle button; accent when sidebar is hidden.
 * Center: workspace name (14px strong) + optional " · N panes" suffix (11px muted).
 * Right: bell button with unread count in accent color.
 */

import "../App.css";

// ── Props ──────────────────────────────────────────────────────────────────

export interface TopBarProps {
  sidebarVisible: boolean;
  notifPanelVisible: boolean;
  workspaceName: string;
  paneCount: number;
  unreadCount: number;
  onToggleSidebar: () => void;
  onToggleNotifications: () => void;
}

// ── Constants (from top_bar.rs + theme.rs) ─────────────────────────────────

const TOP_BAR_HEIGHT = 34;

// ── Component ──────────────────────────────────────────────────────────────

export function TopBar({
  sidebarVisible,
  workspaceName,
  paneCount,
  unreadCount,
  onToggleSidebar,
  onToggleNotifications,
}: TopBarProps) {
  const isMac = navigator.platform.includes("Mac");
  const leftPad = isMac ? 76 : 12;

  return (
    <header
      className="app-top-bar"
      style={{
        height: TOP_BAR_HEIGHT,
        background: "var(--chrome-bg)",
        borderBottom: "1px solid var(--chrome-border)",
        display: "flex",
        alignItems: "center",
        position: "relative",
        userSelect: "none",
      }}
    >
      {/* Sidebar toggle (left) — 20x20, radius 2 */}
      <button
        className="top-bar-toggle"
        onClick={onToggleSidebar}
        title={isMac ? "\u2318B to toggle" : "Ctrl+B to toggle"}
        style={{
          position: "absolute",
          left: leftPad + 10,
          top: "50%",
          transform: "translateY(-50%)",
          width: 20,
          height: 20,
          borderRadius: 2,
          background: "transparent",
          border: "none",
          color: sidebarVisible ? "var(--text-muted)" : "var(--accent)",
          fontSize: 12,
          cursor: "pointer",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          padding: 0,
          lineHeight: 1,
        }}
      >
        {"\u2630"}
      </button>

      {/* Center: workspace name + pane count */}
      <div
        style={{
          flex: 1,
          display: "flex",
          justifyContent: "center",
          alignItems: "baseline",
          gap: 0,
        }}
      >
        <span style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)" }}>
          {workspaceName}
        </span>
        {paneCount > 1 && (
          <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
            {" \u00B7 "}
            {paneCount} panes
          </span>
        )}
      </div>

      {/* Notification bell (right) */}
      <button
        className="top-bar-bell"
        onClick={onToggleNotifications}
        title={isMac ? "\u2318I" : "Ctrl+I"}
        style={{
          position: "absolute",
          right: 12,
          top: "50%",
          transform: "translateY(-50%)",
          height: 22,
          padding: "0 6px",
          borderRadius: 2,
          background: "var(--chrome-bg)",
          border: "1px solid var(--border)",
          color: "var(--text-muted)",
          fontSize: 11,
          cursor: "pointer",
          display: "flex",
          alignItems: "center",
          gap: 4,
        }}
      >
        <span role="img" aria-label="notifications" style={{ fontSize: 11 }}>
          {"\uD83D\uDD14"}
        </span>
        {unreadCount > 0 && (
          <span style={{ color: "var(--accent)", fontWeight: 600 }}>
            {unreadCount}
          </span>
        )}
      </button>
    </header>
  );
}
