/**
 * WorkspaceView — recursive pane tree with split layout.
 *
 * Matches `crates/rmux-app/src/ui/workspace_view.rs`.
 * Renders the PaneNode tree into the central area. Split nodes divide
 * space proportionally with 1px border-color hairlines between children.
 * Leaf nodes render TerminalPane or browser placeholders.
 * Zoom mode renders only one pane with a restore indicator.
 */

import { useCallback } from "react";
import type { PaneNode, PaneId } from "../types";
import { TerminalPane } from "./TerminalPane";
import "../App.css";

// ── Props ──────────────────────────────────────────────────────────────────

export interface WorkspaceViewProps {
  root: PaneNode;
  activePaneId: PaneId;
  zoomedPaneId: PaneId | null;
  onActivatePane: (id: PaneId) => void;
}

// ── Split border constant (from workspace_view.rs) ─────────────────────────

const SPLIT_BORDER = 1;

// ── Component ──────────────────────────────────────────────────────────────

export function WorkspaceView({
  root,
  activePaneId,
  zoomedPaneId,
  onActivatePane,
}: WorkspaceViewProps) {
  const isMac = navigator.platform.includes("Mac");
  const modifier = isMac ? "Cmd" : "Ctrl";

  return (
    <main
      className="app-main"
      style={{
        background: "var(--app-bg)",
        position: "relative",
        overflow: "hidden",
      }}
    >
      {zoomedPaneId !== null ? (
        <>
          {/* Render only the zoomed pane */}
          <RenderZoomed
            node={root}
            zoomedId={zoomedPaneId}
            activePaneId={activePaneId}
            onActivatePane={onActivatePane}
          />
          {/* Zoom indicator pill (top-right) */}
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
        />
      )}
    </main>
  );
}

// ── Node Renderer ──────────────────────────────────────────────────────────

function RenderNode({
  node,
  activePaneId,
  onActivatePane,
}: {
  node: PaneNode;
  activePaneId: PaneId;
  onActivatePane: (id: PaneId) => void;
}) {
  if (node.type === "leaf") {
    return (
      <RenderLeaf
        id={node.id}
        isActive={node.id === activePaneId}
        onActivate={() => onActivatePane(node.id)}
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

  // Split
  return (
    <RenderSplit
      node={node}
      activePaneId={activePaneId}
      onActivatePane={onActivatePane}
    />
  );
}

// ── Leaf (Terminal Pane) ───────────────────────────────────────────────────

function RenderLeaf({
  id,
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
        background: "var(--terminal-bg)",
        border: isActive
          ? "1px solid var(--accent)"
          : "1px solid var(--border)",
        boxSizing: "border-box",
      }}
    >
      <TerminalPane paneId={id} isActive={isActive} />
    </div>
  );
}

// ── Browser Placeholder ────────────────────────────────────────────────────

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

// ── Split ──────────────────────────────────────────────────────────────────

function RenderSplit({
  node,
  activePaneId,
  onActivatePane,
}: {
  node: PaneNode & { type: "split" };
  activePaneId: PaneId;
  onActivatePane: (id: PaneId) => void;
}) {
  const { direction, children, sizes } = node;
  const isHorizontal = direction === "horizontal";
  const totalBorders = SPLIT_BORDER * (children.length - 1);

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
        const sizePct = ratio * 100;

        return (
          <div key={i} style={{ display: "flex", flexDirection: isHorizontal ? "row" : "column", flex: ratio }}>
            {/* Child */}
            <div style={{ flex: "1 1 0", minWidth: 0, minHeight: 0 }}>
              <RenderNode
                node={child}
                activePaneId={activePaneId}
                onActivatePane={onActivatePane}
              />
            </div>
            {/* Divider hairline (1px border-color) */}
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

// ── Zoomed Pane ────────────────────────────────────────────────────────────

function RenderZoomed({
  node,
  zoomedId,
  activePaneId,
  onActivatePane,
}: {
  node: PaneNode;
  zoomedId: PaneId;
  activePaneId: PaneId;
  onActivatePane: (id: PaneId) => void;
}) {
  // Recursively find the zoomed leaf/browser in the tree.
  const found = findNode(node, zoomedId);

  if (!found) {
    return (
      <RenderNode
        node={node}
        activePaneId={activePaneId}
        onActivatePane={onActivatePane}
      />
    );
  }

  if (found.type === "leaf") {
    return (
      <RenderLeaf
        id={found.id}
        isActive={found.id === activePaneId}
        onActivate={() => onActivatePane(found.id)}
      />
    );
  }

  // found must be "browser" (findNode never returns "split")
  if (found.type === "browser") {
    return (
      <RenderBrowserPlaceholder
        id={found.id}
        isActive={found.id === activePaneId}
        onActivate={() => onActivatePane(found.id)}
      />
    );
  }

  // Fallback: render the full tree
  return (
    <RenderNode
      node={node}
      activePaneId={activePaneId}
      onActivatePane={onActivatePane}
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
