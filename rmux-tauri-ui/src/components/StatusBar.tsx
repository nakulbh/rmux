/**
 * StatusBar — 26px bottom bar matching `crates/rmux-app/src/ui/status_bar.rs`.
 *
 * Arbor One Dark: chrome_bg fill, chrome_border top hairline.
 * Left: workspace name + pane count.
 * Right: workspace count + unread notification count.
 */

import type { FC } from "react";
import "../App.css";

export interface StatusBarProps {
  workspaceName: string;
  paneCount: number;
  workspaceCount: number;
  unreadCount: number;
}

export const StatusBar: FC<StatusBarProps> = ({
  workspaceName,
  paneCount,
  workspaceCount,
  unreadCount,
}) => {
  return (
    <footer
      className="status-bar"
      style={{
        height: 26,
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "0 8px",
        background: "var(--chrome-bg)",
        borderTop: "1px solid var(--chrome-border)",
        fontSize: 11,
        color: "var(--text-muted)",
        flexShrink: 0,
        userSelect: "none",
      }}
    >
      <div
        className="status-bar-left"
        style={{ display: "flex", alignItems: "center", gap: 6 }}
      >
        <span>
          {workspaceName} — {paneCount} pane{paneCount !== 1 ? "s" : ""}
        </span>
      </div>
      <div
        className="status-bar-right"
        style={{ display: "flex", alignItems: "center", gap: 6 }}
      >
        <span>{workspaceCount} workspace{workspaceCount !== 1 ? "s" : ""}</span>
        {unreadCount > 0 && (
          <span>
            {unreadCount} notification{unreadCount !== 1 ? "s" : ""}
          </span>
        )}
      </div>
    </footer>
  );
};
