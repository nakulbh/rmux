/**
 * App — rmux root component with real Tauri backend integration.
 */

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import {
  Sidebar,
  WorkspaceView,
  NotificationPanel,
  TopBar,
  StatusBar,
} from "./components";
import type { Workspace, PaneNode, Notification } from "./types";

// ── Component ──────────────────────────────────────────────────────────────

export function App() {
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<number | null>(null);
  const [notifications, setNotifications] = useState<Notification[]>([]);
  const [sidebarVisible, setSidebarVisible] = useState(true);
  const [notifVisible, setNotifVisible] = useState(false);
  const [activePaneId, setActivePaneId] = useState<number>(1);
  const [zoomedPaneId, setZoomedPaneId] = useState<number | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  // Load workspaces and notifications on mount
  useEffect(() => {
    const loadData = async () => {
      try {
        const wsList = await invoke<Workspace[]>("list_workspaces");
        setWorkspaces(wsList);
        if (wsList.length > 0) {
          setActiveWorkspaceId(wsList[0].id);
          setActivePaneId(wsList[0].active_pane ?? 1);
        }

        const notifs = await invoke<Notification[]>("get_notifications");
        setNotifications(notifs);
      } catch (e) {
        console.error("Failed to load initial data:", e);
      } finally {
        setIsLoading(false);
      }
    };
    loadData();
  }, []);

  const activeWs = workspaces.find((w) => w.id === activeWorkspaceId);

  const handleSwitchWorkspace = useCallback(
    async (index: number) => {
      const ws = workspaces[index];
      if (!ws) return;
      try {
        await invoke("switch_workspace", { workspace_id: ws.id });
        setActiveWorkspaceId(ws.id);
        setActivePaneId(ws.active_pane ?? 1);
      } catch (e) {
        console.error("Failed to switch workspace:", e);
      }
    },
    [workspaces]
  );

  const handleCreateWorkspace = useCallback(async () => {
    try {
      const newId = await invoke<number>("create_workspace", {
        name: `workspace-${workspaces.length + 1}`,
      });
      const wsList = await invoke<Workspace[]>("list_workspaces");
      setWorkspaces(wsList);
      setActiveWorkspaceId(newId);
      // Find the new workspace and set its initial pane
      const newWs = wsList.find((w) => w.id === newId);
      if (newWs) {
        setActivePaneId(newWs.active_pane ?? 1);
      }
    } catch (e) {
      console.error("Failed to create workspace:", e);
    }
  }, [workspaces.length]);

  const handleRenameWorkspace = useCallback(
    async (index: number, name: string) => {
      const ws = workspaces[index];
      if (!ws) return;
      try {
        await invoke("rename_workspace", { workspace_id: ws.id, name });
        const wsList = await invoke<Workspace[]>("list_workspaces");
        setWorkspaces(wsList);
      } catch (e) {
        console.error("Failed to rename workspace:", e);
      }
    },
    [workspaces]
  );

  const handleCloseWorkspace = useCallback(
    async (id: number) => {
      try {
        await invoke("close_workspace", { workspace_id: id });
        const wsList = await invoke<Workspace[]>("list_workspaces");
        setWorkspaces(wsList);
        if (activeWorkspaceId === id && wsList.length > 0) {
          setActiveWorkspaceId(wsList[0].id);
          setActivePaneId(wsList[0].active_pane ?? 1);
        }
      } catch (e) {
        console.error("Failed to close workspace:", e);
      }
    },
    [activeWorkspaceId]
  );

  const handleActivatePane = useCallback(
    async (paneId: number) => {
      if (!activeWorkspaceId) return;
      try {
        await invoke("focus_pane", {
          workspace_id: activeWorkspaceId,
          pane_id: paneId,
        });
        setActivePaneId(paneId);
      } catch (e) {
        console.error("Failed to focus pane:", e);
      }
    },
    [activeWorkspaceId]
  );

  const handleSplitPaneRight = useCallback(async () => {
    if (!activeWorkspaceId || !activePaneId) return;
    try {
      await invoke("split_pane_right", {
        workspace_id: activeWorkspaceId,
        pane_id: activePaneId,
      });
      const wsList = await invoke<Workspace[]>("list_workspaces");
      setWorkspaces(wsList);
    } catch (e) {
      console.error("Failed to split pane:", e);
    }
  }, [activeWorkspaceId, activePaneId]);

  const handleSplitPaneDown = useCallback(async () => {
    if (!activeWorkspaceId || !activePaneId) return;
    try {
      await invoke("split_pane_down", {
        workspace_id: activeWorkspaceId,
        pane_id: activePaneId,
      });
      const wsList = await invoke<Workspace[]>("list_workspaces");
      setWorkspaces(wsList);
    } catch (e) {
      console.error("Failed to split pane:", e);
    }
  }, [activeWorkspaceId, activePaneId]);

  const handleClosePane = useCallback(async () => {
    if (!activeWorkspaceId || !activePaneId) return;
    try {
      await invoke("close_pane", {
        workspace_id: activeWorkspaceId,
        pane_id: activePaneId,
      });
      const wsList = await invoke<Workspace[]>("list_workspaces");
      setWorkspaces(wsList);
    } catch (e) {
      console.error("Failed to close pane:", e);
    }
  }, [activeWorkspaceId, activePaneId]);

  const handleMarkRead = useCallback(async (id: number) => {
    try {
      await invoke("dismiss_notification", { notification_id: id });
      setNotifications((prev) =>
        prev.map((n) => (n.id === id ? { ...n, read: true } : n))
      );
    } catch (e) {
      console.error("Failed to dismiss notification:", e);
    }
  }, []);

  const handleMarkAllRead = useCallback(async () => {
    try {
      await invoke("dismiss_all_notifications");
      setNotifications((prev) => prev.map((n) => ({ ...n, read: true })));
    } catch (e) {
      console.error("Failed to dismiss all notifications:", e);
    }
  }, []);

  const handleClearNotifications = useCallback(() => {
    setNotifications([]);
  }, []);

  const unreadCount = notifications.filter((n) => !n.read).length;

  // Build unread map from notifications
  const unreadMap: Record<string, number> = {};
  for (const n of notifications) {
    if (!n.read && n.workspaceId) {
      unreadMap[n.workspaceId] = (unreadMap[n.workspaceId] || 0) + 1;
    }
  }

  // Get active workspace index
  const activeIndex = workspaces.findIndex((w) => w.id === activeWorkspaceId);

  // Get pane tree for active workspace
  const paneTree: PaneNode = activeWs?.root ?? { type: "leaf", id: 1 };

  if (isLoading) {
    return (
      <div className="app-layout" style={{ alignItems: "center", justifyContent: "center" }}>
        <div style={{ color: "var(--text-muted)", fontSize: 14 }}>Loading rmux...</div>
      </div>
    );
  }

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

      <div className="main-content">
        {sidebarVisible && (
          <div className="sidebar">
            <Sidebar
              workspaces={workspaces}
              activeIndex={activeIndex >= 0 ? activeIndex : 0}
              unreadMap={unreadMap}
              onSwitch={handleSwitchWorkspace}
              onRename={handleRenameWorkspace}
              onNew={handleCreateWorkspace}
            />
          </div>
        )}

        <WorkspaceView
          root={paneTree}
          activePaneId={activePaneId}
          zoomedPaneId={zoomedPaneId}
          onActivatePane={handleActivatePane}
          workspaceId={activeWorkspaceId ?? 0}
        />

        {notifVisible && (
          <div className="notification-panel">
            <NotificationPanel
              visible={notifVisible}
              notifications={notifications}
              onClose={() => setNotifVisible(false)}
              onMarkRead={handleMarkRead}
              onMarkAllRead={handleMarkAllRead}
              onClear={handleClearNotifications}
            />
          </div>
        )}
      </div>

      <StatusBar
        workspaceName={activeWs?.name ?? "rmux"}
        paneCount={activeWs?.paneCount ?? 0}
        workspaceCount={workspaces.length}
        unreadCount={unreadCount}
      />
    </div>
  );
}
