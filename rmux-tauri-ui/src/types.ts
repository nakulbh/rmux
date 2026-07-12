/** Unique identifier for a pane, matches Rust PaneId(u64). */
export type PaneId = number;

/** Direction for pane splits. */
export type SplitDirection = "horizontal" | "vertical";

/** Map from workspace id string to unread notification count. */
export type UnreadMap = Record<string, number>;

/* ------------------------------------------------------------------ */
/*  PaneNode — discriminated union matching the Rust PaneTree          */
/* ------------------------------------------------------------------ */

export type PaneNode = PaneNodeLeaf | PaneNodeBrowser | PaneNodeSplit;

export interface PaneNodeLeaf {
  type: "leaf";
  id: PaneId;
  terminalId?: number;
}

export interface PaneNodeBrowser {
  type: "browser";
  id: PaneId;
}

export interface PaneNodeSplit {
  type: "split";
  id: PaneId;
  direction: SplitDirection;
  children: PaneNode[];
  /** Proportional sizes for each child (sums to 1.0). */
  sizes: number[];
}

/* ------------------------------------------------------------------ */
/*  Workspace                                                          */
/* ------------------------------------------------------------------ */

export interface Workspace {
  id: number;
  index: number;
  name: string;
  paneCount: number;
  /** Optional status string (e.g., "building…"). */
  status?: string;
  /** Optional progress fraction (0.0–1.0) for status bar capsule. */
  progress?: number;
}

/* ------------------------------------------------------------------ */
/*  Notifications                                                      */
/* ------------------------------------------------------------------ */

export type NotificationLevel = "info" | "success" | "warning" | "error";

/** @deprecated Use `Notification` instead. */
export type AppNotification = Notification;

export interface Notification {
  id: number;
  title: string;
  body: string;
  level?: NotificationLevel;
  /** ISO 8601 timestamp. */
  timestamp: string;
  workspaceId?: number;
  paneId?: PaneId;
  read: boolean;
}
