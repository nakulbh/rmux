/**
 * App — rmux root component.
 *
 * Minimal shell that imports all UI components to verify they compile.
 * The full orchestrator (state management, Tauri invoke bridge, keyboard
 * shortcuts) is created by another task.
 */

import { useState } from "react";
import "./App.css";
import {
  Sidebar,
  WorkspaceView,
  NotificationPanel,
  TopBar,
  StatusBar,
} from "./components";
import type { Workspace, PaneNode, Notification } from "./types";

// ── Initial demo data ──────────────────────────────────────────────────────

const DEMO_WORKSPACES: Workspace[] = [
  { id: 1, index: 0, name: "main", paneCount: 2, status: "Working" },
  { id: 2, index: 1, name: "api-server", paneCount: 1 },
  { id: 3, index: 2, name: "database", paneCount: 3, progress: 0.65 },
];

const DEMO_PANE_TREE: PaneNode = {
  type: "split",
  id: 0,
  direction: "horizontal",
  sizes: [0.5, 0.5],
  children: [
    { type: "leaf", id: 101 },
    {
      type: "split",
      id: 1,
      direction: "vertical",
      sizes: [0.6, 0.4],
      children: [
        { type: "leaf", id: 102 },
        { type: "leaf", id: 103 },
      ],
    },
  ],
};

const DEMO_NOTIFICATIONS: Notification[] = [
  {
    id: 1,
    title: "Build complete",
    body: "api-server compiled successfully in 2.3s",
    level: "success",
    timestamp: new Date(Date.now() - 120_000).toISOString(),
    workspaceId: 2,
    read: false,
  },
  {
    id: 2,
    title: "Connection lost",
    body: "database connection timed out after 30s",
    level: "error",
    timestamp: new Date(Date.now() - 3600_000).toISOString(),
    workspaceId: 3,
    read: false,
  },
  {
    id: 3,
    title: "Lint warnings",
    body: "12 warnings in crates/rmux-app/",
    level: "warning",
    timestamp: new Date(Date.now() - 7200_000).toISOString(),
    workspaceId: 1,
    read: true,
  },
];

const DEMO_UNREAD: Record<string, number> = {
  "1": 1,
  "2": 2,
  "3": 5,
};

// ── Component ──────────────────────────────────────────────────────────────

export function App() {
  const [workspaces, _setWorkspaces] = useState<Workspace[]>(DEMO_WORKSPACES);
  const [activeIndex, setActiveIndex] = useState(0);
  const [notifications, setNotifications] = useState<Notification[]>(DEMO_NOTIFICATIONS);
  const [sidebarVisible, setSidebarVisible] = useState(true);
  const [notifVisible, setNotifVisible] = useState(false);
  const [activePaneId, _setActivePaneId] = useState(101);
  const [zoomedPaneId, _setZoomedPaneId] = useState<number | null>(null);

  const activeWs = workspaces[activeIndex];

  const handleMarkRead = (id: number) => {
    setNotifications((prev) =>
      prev.map((n) => (n.id === id ? { ...n, read: true } : n))
    );
  };

  const handleMarkAllRead = () => {
    setNotifications((prev) => prev.map((n) => ({ ...n, read: true })));
  };

  const handleClear = () => {
    setNotifications([]);
  };

  const unreadCount = notifications.filter((n) => !n.read).length;

  return (
    <div className="app-layout">
      <TopBar
        sidebarVisible={sidebarVisible}
        notifPanelVisible={notifVisible}
        workspaceName={activeWs?.name ?? "rmux"}
        paneCount={activeWs?.paneCount ?? 0}
        unreadCount={unreadCount}
        onToggleSidebar={() => setSidebarVisible((v) => !v)}
        onToggleNotifications={() => setNotifVisible((v) => !v)}
      />

      {sidebarVisible && (
        <Sidebar
          workspaces={workspaces}
          activeIndex={activeIndex}
          unreadMap={DEMO_UNREAD}
          onSwitch={setActiveIndex}
          onRename={(index, name) => {
            _setWorkspaces((prev) =>
              prev.map((w, i) => (i === index ? { ...w, name } : w))
            );
          }}
          onNew={() => {
            const newId = Date.now();
            _setWorkspaces((prev) => [
              ...prev,
              {
                id: newId,
                index: prev.length,
                name: `workspace-${prev.length + 1}`,
                paneCount: 1,
              },
            ]);
          }}
        />
      )}

      <WorkspaceView
        root={DEMO_PANE_TREE}
        activePaneId={activePaneId}
        zoomedPaneId={zoomedPaneId}
        onActivatePane={_setActivePaneId}
      />

      <NotificationPanel
        visible={notifVisible}
        notifications={notifications}
        onClose={() => setNotifVisible(false)}
        onMarkRead={handleMarkRead}
        onMarkAllRead={handleMarkAllRead}
        onClear={handleClear}
      />

      <StatusBar
        workspaceName={activeWs?.name ?? "rmux"}
        paneCount={activeWs?.paneCount ?? 0}
        workspaceCount={workspaces.length}
        unreadCount={unreadCount}
      />
    </div>
  );
}
