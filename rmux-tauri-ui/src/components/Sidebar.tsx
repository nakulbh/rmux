/**
 * Sidebar — Arbor-style vertical workspace card list.
 *
 * Matches `crates/rmux-app/src/ui/sidebar.rs`.
 * Renders cards with: workspace name (12.5px), mono metadata line
 * ("N panes · status"), unread badge (accent circle, 9px mono count),
 * and optional 3px progress capsule. Active card gets accent border +
 * panel_active_bg fill; inactive cards at 0.8 opacity.
 * Double-click starts inline rename; single click switches workspace.
 */

import { useState, useRef, useEffect, useCallback, type KeyboardEvent } from "react";
import type { Workspace, UnreadMap } from "../types";
import "../App.css";

// ── Constants (from sidebar.rs) ────────────────────────────────────────────

const CARD_RADIUS = 2;
const CARD_PAD_X = 8;
const CARD_PAD_Y = 6;
const INACTIVE_OPACITY = 0.8;

// ── Props ──────────────────────────────────────────────────────────────────

export interface SidebarProps {
  workspaces: Workspace[];
  activeIndex: number;
  unreadMap: UnreadMap;
  onSwitch: (index: number) => void;
  onRename: (index: number, name: string) => void;
  onNew: () => void;
}

// ── Component ──────────────────────────────────────────────────────────────

export function Sidebar({
  workspaces,
  activeIndex,
  unreadMap,
  onSwitch,
  onRename,
  onNew,
}: SidebarProps) {
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [editBuffer, setEditBuffer] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  // Focus the rename input when editing begins.
  useEffect(() => {
    if (editingIndex !== null) {
      inputRef.current?.focus();
      inputRef.current?.select();
    }
  }, [editingIndex]);

  const commitRename = useCallback(() => {
    if (editingIndex !== null && editBuffer.trim()) {
      onRename(editingIndex, editBuffer.trim());
    }
    setEditingIndex(null);
    setEditBuffer("");
  }, [editingIndex, editBuffer, onRename]);

  const handleRenameKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      commitRename();
    } else if (e.key === "Escape") {
      setEditingIndex(null);
      setEditBuffer("");
    }
  };

  const handleCardClick = (index: number) => {
    if (editingIndex !== index) {
      onSwitch(index);
    }
  };

  const handleCardDoubleClick = (index: number, name: string) => {
    setEditingIndex(index);
    setEditBuffer(name);
  };

  const isMac = navigator.platform.includes("Mac");
  const toggleHint = isMac ? "\u2318B to toggle" : "Ctrl+B to toggle";
  const newHint = isMac ? "\u2318N" : "Ctrl+N";

  return (
    <aside
      className="app-sidebar"
      style={{
        minWidth: "var(--sidebar-min-width)",
        maxWidth: "var(--sidebar-max-width)",
        width: "var(--sidebar-default-width)",
        background: "var(--sidebar-bg)",
        padding: 8,
        display: "flex",
        flexDirection: "column",
      }}
    >
      {/* Header */}
      <Header count={workspaces.length} />

      {/* Card List */}
      <div style={{ flex: 1, overflowY: "auto", paddingTop: 4 }}>
        {workspaces.map((ws, i) => {
          const isActive = i === activeIndex;
          const isEditing = editingIndex === i;
          const unread = unreadMap[String(ws.id)] ?? 0;

          if (isEditing) {
            return (
              <div
                key={ws.id}
                className="sidebar-card editing"
                style={{
                  height: ws.status ? 52 : 42,
                  padding: `${CARD_PAD_Y}px ${CARD_PAD_X}px`,
                  borderRadius: CARD_RADIUS,
                  background: "var(--panel-bg)",
                  border: "1px solid var(--accent)",
                  marginBottom: 2,
                  display: "flex",
                  alignItems: "center",
                }}
              >
                <input
                  ref={inputRef}
                  className="mono"
                  value={editBuffer}
                  onChange={(e) => setEditBuffer(e.target.value)}
                  onBlur={commitRename}
                  onKeyDown={handleRenameKeyDown}
                  style={{
                    width: "100%",
                    background: "transparent",
                    border: "none",
                    outline: "none",
                    color: "var(--text-primary)",
                    fontSize: 12.5,
                    fontFamily: "inherit",
                  }}
                />
              </div>
            );
          }

          const opacity = isActive ? 1 : INACTIVE_OPACITY;

          return (
            <div
              key={ws.id}
              className="sidebar-card"
              onClick={() => handleCardClick(i)}
              onDoubleClick={() => handleCardDoubleClick(i, ws.name)}
              style={{
                height: ws.status ? 52 : 42,
                padding: `${CARD_PAD_Y}px ${CARD_PAD_X}px`,
                borderRadius: CARD_RADIUS,
                background: isActive ? "var(--panel-active-bg)" : "var(--panel-bg)",
                border: `1px solid ${isActive ? "var(--accent)" : "var(--border)"}`,
                marginBottom: 2,
                opacity,
                cursor: "pointer",
                position: "relative",
                overflow: "hidden",
              }}
            >
              {/* Unread badge */}
              {unread > 0 && (
                <span
                  className="unread-badge mono"
                  style={{
                    position: "absolute",
                    top: 8,
                    right: 8,
                    width: 16,
                    height: 16,
                    borderRadius: "50%",
                    background: "var(--accent)",
                    color: "var(--accent-fg)",
                    fontSize: 9,
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                  }}
                >
                  {unread}
                </span>
              )}

              {/* Line 1: workspace name */}
              <div
                style={{
                  fontSize: 12.5,
                  color: "var(--text-primary)",
                  whiteSpace: "nowrap",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  paddingRight: unread > 0 ? 22 : 0,
                  lineHeight: 1.3,
                }}
              >
                {ws.name}
              </div>

              {/* Line 2: mono metadata */}
              <div
                className="mono"
                style={{
                  fontSize: 10,
                  color: "var(--text-muted)",
                  marginTop: 2,
                }}
              >
                {ws.paneCount === 1 ? "1 pane" : `${ws.paneCount} panes`}
                {ws.status && (
                  <>
                    {" · "}
                    <span style={{ color: "var(--warning)" }}>{ws.status}</span>
                  </>
                )}
              </div>

              {/* Progress capsule */}
              {ws.progress !== undefined && ws.progress > 0 && (
                <div
                  style={{
                    position: "absolute",
                    bottom: 1,
                    left: 1,
                    right: 1,
                    height: 3,
                    borderRadius: 2,
                    background: "var(--border)",
                  }}
                >
                  <div
                    style={{
                      height: "100%",
                      width: `${Math.min(ws.progress * 100, 100)}%`,
                      borderRadius: 2,
                      background: "var(--accent)",
                    }}
                  />
                </div>
              )}
            </div>
          );
        })}
      </div>

      {/* Footer */}
      <div style={{ paddingTop: 4 }}>
        {/* Separator */}
        <div
          style={{
            height: 1,
            background: "var(--border)",
            marginBottom: 4,
          }}
        />
        {/* New Workspace button */}
        <button
          className="new-workspace-btn"
          onClick={onNew}
          title={`New workspace (${newHint})`}
          style={{
            width: "100%",
            height: "var(--button-height)",
            borderRadius: CARD_RADIUS,
            background: "var(--panel-bg)",
            border: "1px solid var(--border)",
            color: "var(--text-primary)",
            fontSize: 12,
            cursor: "pointer",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            gap: 4,
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.background = "var(--panel-active-bg)";
            e.currentTarget.style.borderColor = "var(--accent)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = "var(--panel-bg)";
            e.currentTarget.style.borderColor = "var(--border)";
          }}
        >
          <span style={{ color: "var(--accent)" }}>+</span> New Workspace
        </button>
        {/* Toggle hint */}
        <div
          style={{
            fontSize: 10,
            color: "var(--text-disabled)",
            textAlign: "center",
            marginTop: 4,
          }}
        >
          {toggleHint}
        </div>
      </div>
    </aside>
  );
}

// ── Header ─────────────────────────────────────────────────────────────────

function Header({ count }: { count: number }) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        height: 32,
        paddingRight: 4,
      }}
    >
      <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
        Workspaces
      </span>
      <span
        className="mono"
        style={{
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          minWidth: 14,
          height: 14,
          padding: "0 5px",
          fontSize: 9,
          color: "var(--text-muted)",
          background: "var(--panel-bg)",
          border: "1px solid var(--border)",
          borderRadius: 7,
        }}
      >
        {count}
      </span>
    </div>
  );
}
