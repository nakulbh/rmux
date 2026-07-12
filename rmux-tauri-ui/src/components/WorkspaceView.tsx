/**
 * WorkspaceView — recursive pane tree with split layout.
 */

import { useCallback } from "react";
import type { PaneNode, PaneId } from "../types";
import { TerminalPane } from "./TerminalPane";
import "../App.css";

const SPLIT_BORDER = 1;

export interface WorkspaceViewProps {
  root: PaneNode;
  activePaneId: PaneId;
  zoomedPaneId: PaneId | null;
  onActivatePane: (id: PaneId) => void;
  workspaceId: number;
}

export function WorkspaceView({
  root,
  activePaneId,
  zoomedPaneId,
  onActivatePane,
  workspaceId,
}: WorkspaceViewProps) {
  const isMac = navigator.platform.includes("Mac");
  const modifier = isMac ? "Cmd" : "Ctrl";

  return (
    <main className="workspace-area">
      {zoomedPaneId !== null ? (
        <>
          <RenderZoomed
            node={root}
            zoomedId={zoomedPaneId}
            activePaneId={activePaneId}
            onActivatePane={onActivatePane}
            workspaceId={workspaceId}
          />
          <div
            style={{
              position: "absolute",
              top: 2,
              right: 4,
              height: 18,
              padding: "0 8px",
              display: "flex",
              alignItems: "center",
              borderRadius: 6,
              background: "var(--chrome-bg)",
              border: "1px solid var(--chrome-border)",
              fontSize: 10,
              color: "var(--text-muted)",
              zIndex: 10,
            }}
          >
            Zoom: {modifier}+Shift+Enter to restore
          </div>
        </>
      ) : (
        <RenderNode
          node={root}
          activePaneId={activePaneId}
          onActivatePane={onActivatePane}
          workspaceId={workspaceId}
        />
      )}
    </main>
  );
}

function RenderNode({
  node,
  activePaneId,
  onActivatePane,
  workspaceId,
}: {
  node: PaneNode;
  activePaneId: PaneId;
  onActivatePane: (id: PaneId) => void;
  workspaceId: number;
}) {
  if (node.type === "leaf") {
    return (
      <RenderLeaf
        id={node.id}
        isActive={node.id === activePaneId}
        onActivate={() => onActivatePane(node.id)}
        workspaceId={workspaceId}
      />
    );
  }

  if (node.type === "browser") {
    return (
      <RenderBrowserPlaceholder
        id={node.id}
        isActive={node.id === activePaneId}
        onActivate={() => onActivatePane(node.id)}
      />
    );
  }

  return (
    <RenderSplit
      node={node}
      activePaneId={activePaneId}
      onActivatePane={onActivatePane}
      workspaceId={workspaceId}
    />
  );
}

function RenderLeaf({
  id,
  isActive,
  onActivate,
  workspaceId,
}: {
  id: PaneId;
  isActive: boolean;
  onActivate: () => void;
  workspaceId: number;
}) {
  return (
    <div
      onClick={onActivate}
      style={{
        width: "100%",
        height: "100%",
        background: "var(--terminal-bg)",
        border: isActive
          ? "1px solid var(--accent)"
          : "1px solid var(--border)",
        boxSizing: "border-box",
      }}
    >
      <TerminalPane paneId={id} workspaceId={workspaceId} isActive={isActive} />
    </div>
  );
}

function RenderBrowserPlaceholder({
  id: _id,
  isActive,
  onActivate,
}: {
  id: PaneId;
  isActive: boolean;
  onActivate: () => void;
}) {
  return (
    <div
      onClick={onActivate}
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "var(--panel-bg)",
        border: isActive
          ? "1px solid var(--accent)"
          : "1px solid var(--border)",
        color: "var(--text-muted)",
        fontSize: 12,
      }}
    >
      Waiting for webview...
    </div>
  );
}

function RenderSplit({
  node,
  activePaneId,
  onActivatePane,
  workspaceId,
}: {
  node: PaneNode & { type: "split" };
  activePaneId: PaneId;
  onActivatePane: (id: PaneId) => void;
  workspaceId: number;
}) {
  const { direction, children, sizes } = node;
  const isHorizontal = direction === "horizontal";

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: isHorizontal ? "row" : "column",
      }}
    >
      {children.map((child, i) => {
        const ratio = sizes[i] ?? 1 / children.length;

        return (
          <div key={i} style={{ display: "flex", flexDirection: isHorizontal ? "row" : "column", flex: ratio }}>
            <div style={{ flex: "1 1 0", minWidth: 0, minHeight: 0 }}>
              <RenderNode
                node={child}
                activePaneId={activePaneId}
                onActivatePane={onActivatePane}
                workspaceId={workspaceId}
              />
            </div>
            {i < children.length - 1 && (
              <div
                style={{
                  [isHorizontal ? "width" : "height"]: SPLIT_BORDER,
                  [isHorizontal ? "minWidth" : "minHeight"]: SPLIT_BORDER,
                  background: "var(--border)",
                  flexShrink: 0,
                }}
              />
            )}
          </div>
        );
      })}
    </div>
  );
}

function RenderZoomed({
  node,
  zoomedId,
  activePaneId,
  onActivatePane,
  workspaceId,
}: {
  node: PaneNode;
  zoomedId: PaneId;
  activePaneId: PaneId;
  onActivatePane: (id: PaneId) => void;
  workspaceId: number;
}) {
  const found = findNode(node, zoomedId);

  if (!found) {
    return (
      <RenderNode
        node={node}
        activePaneId={activePaneId}
        onActivatePane={onActivatePane}
        workspaceId={workspaceId}
      />
    );
  }

  if (found.type === "leaf") {
    return (
      <RenderLeaf
        id={found.id}
        isActive={found.id === activePaneId}
        onActivate={() => onActivatePane(found.id)}
        workspaceId={workspaceId}
      />
    );
  }

  if (found.type === "browser") {
    return (
      <RenderBrowserPlaceholder
        id={found.id}
        isActive={found.id === activePaneId}
        onActivate={() => onActivatePane(found.id)}
      />
    );
  }

  return (
    <RenderNode
      node={node}
      activePaneId={activePaneId}
      onActivatePane={onActivatePane}
      workspaceId={workspaceId}
    />
  );
}

function findNode(node: PaneNode, id: PaneId): PaneNode | null {
  if (node.type === "leaf" || node.type === "browser") {
    return node.id === id ? node : null;
  }
  for (const child of node.children) {
    const found = findNode(child, id);
    if (found) return found;
  }
  return null;
}
