/**
 * TerminalPane — xterm.js terminal emulator with PTY backend integration.
 */

import { useRef, useEffect, useCallback, useState, type KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { PaneId } from "../types";
import "../App.css";

import { Terminal } from "xterm";
import { FitAddon } from "@xterm/addon-fit";
import "xterm/css/xterm.css";

// ── Props ──────────────────────────────────────────────────────────────────

export interface TerminalPaneProps {
  paneId: PaneId;
  workspaceId: number;
  isActive: boolean;
}

const FIND_BAR_HEIGHT = 28;

// ── Component ──────────────────────────────────────────────────────────────

export function TerminalPane({ paneId, workspaceId, isActive }: TerminalPaneProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const terminalIdRef = useRef<number | null>(null);
  const unlistenRef = useRef<UnlistenFn | null>(null);

  // Find bar state
  const [findVisible, setFindVisible] = useState(false);
  const [findQuery, setFindQuery] = useState("");
  const [findResults, setFindResults] = useState<{ row: number; col: number }[]>([]);
  const [findIndex, setFindIndex] = useState(0);
  const findInputRef = useRef<HTMLInputElement>(null);

  // Initialize terminal and spawn PTY
  useEffect(() => {
    if (!containerRef.current) return;

    const term = new Terminal({
      cursorBlink: true,
      fontSize: 14,
      fontFamily:
        '"SF Mono", "Fira Code", "Fira Mono", "Roboto Mono", Menlo, Monaco, Consolas, monospace',
      theme: {
        background: "#282c34",
        foreground: "#c8ccd4",
        cursor: "#ebdbb2",
        cursorAccent: "#282c34",
        selectionBackground: "#3e4451",
        selectionForeground: "#c8ccd4",
        black: "#282c34",
        red: "#eb6f92",
        green: "#72d69c",
        yellow: "#e5c07b",
        blue: "#74ade8",
        magenta: "#d291bc",
        cyan: "#56b6c2",
        white: "#c8ccd4",
        brightBlack: "#696b77",
        brightRed: "#eb6f92",
        brightGreen: "#72d69c",
        brightYellow: "#e5c07b",
        brightBlue: "#74ade8",
        brightMagenta: "#d291bc",
        brightCyan: "#56b6c2",
        brightWhite: "#ffffff",
      },
      allowProposedApi: true,
    });

    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(containerRef.current);
    fit.fit();

    terminalRef.current = term;
    fitAddonRef.current = fit;

    // Spawn PTY backend
    const spawnPty = async () => {
      try {
        const dims = fit.proposeDimensions();
        const cols = dims?.cols ?? 80;
        const rows = dims?.rows ?? 24;

        const result = await invoke<{ id: number }>("spawn_terminal", {
          workspace_id: workspaceId,
          pane_id: paneId,
          cols,
          rows,
        });
        terminalIdRef.current = result.id;

        // Listen for PTY output events
        const unlisten = await listen<{
          terminal_id: number;
          data: number[];
        }>("pty-output", (event) => {
          if (event.payload.terminal_id === result.id) {
            const bytes = new Uint8Array(event.payload.data);
            term.write(bytes);
          }
        });
        unlistenRef.current = unlisten;
      } catch (e) {
        console.error("Failed to spawn terminal:", e);
        term.writeln("\r\nFailed to spawn terminal. Check backend logs.");
      }
    };

    spawnPty();

    // Handle keyboard input
    const dataDisposable = term.onData(async (data) => {
      const id = terminalIdRef.current;
      if (id === null) return;
      try {
        const encoder = new TextEncoder();
        const bytes = Array.from(encoder.encode(data));
        await invoke("write_terminal", { terminal_id: id, data: bytes });
      } catch (e) {
        console.error("Failed to write to terminal:", e);
      }
    });

    // Resize observer
    const observer = new ResizeObserver(() => {
      fit.fit();
      const dims = fit.proposeDimensions();
      if (dims && terminalIdRef.current !== null) {
        invoke("resize_terminal", {
          terminal_id: terminalIdRef.current,
          cols: dims.cols,
          rows: dims.rows,
        }).catch((e) => console.error("Resize failed:", e));
      }
    });
    observer.observe(containerRef.current);

    return () => {
      observer.disconnect();
      dataDisposable.dispose();
      if (unlistenRef.current) {
        unlistenRef.current();
      }
      if (terminalIdRef.current !== null) {
        invoke("close_terminal", { terminal_id: terminalIdRef.current }).catch(
          (e) => console.error("Close terminal failed:", e)
        );
      }
      term.dispose();
    };
  }, [paneId, workspaceId]);

  // Focus find input when find bar opens
  useEffect(() => {
    if (findVisible) {
      findInputRef.current?.focus();
    }
  }, [findVisible]);

  // Keyboard handler: Cmd/Ctrl+F toggles find bar
  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLDivElement>) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "f") {
        e.preventDefault();
        setFindVisible((v) => !v);
      }
    },
    []
  );

  // Search the terminal buffer
  const doFind = (query: string, term: Terminal) => {
    if (!query) {
      setFindResults([]);
      setFindIndex(0);
      return;
    }

    const results: { row: number; col: number }[] = [];
    const lowerQuery = query.toLowerCase();
    const buffer = term.buffer.active;

    for (let row = 0; row < buffer.length; row++) {
      const line = buffer.getLine(row);
      if (!line) continue;
      const text = line.translateToString(true);
      const lowerText = text.toLowerCase();

      let col = 0;
      while (col <= text.length - lowerQuery.length) {
        if (lowerText.startsWith(lowerQuery, col)) {
          results.push({ row, col });
          col += lowerQuery.length;
        } else {
          col++;
        }
      }
    }

    setFindResults(results);
    setFindIndex(0);
  };

  const handleFindInputChange = (value: string) => {
    setFindQuery(value);
    const term = terminalRef.current;
    if (term) {
      doFind(value, term);
    }
  };

  const handleFindPrev = () => {
    setFindIndex((prev) =>
      findResults.length === 0 ? 0 : (prev - 1 + findResults.length) % findResults.length
    );
  };

  const handleFindNext = () => {
    setFindIndex((prev) =>
      findResults.length === 0 ? 0 : (prev + 1) % findResults.length
    );
  };

  const handleFindClose = () => {
    setFindVisible(false);
    setFindQuery("");
    setFindResults([]);
    setFindIndex(0);
  };

  const handleFindKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      handleFindClose();
    } else if (e.key === "Enter") {
      handleFindNext();
    }
  };

  return (
    <div
      onKeyDown={handleKeyDown}
      tabIndex={0}
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        outline: "none",
        position: "relative",
      }}
    >
      {/* Terminal container */}
      <div
        ref={containerRef}
        style={{
          flex: 1,
          minHeight: 0,
          overflow: "hidden",
        }}
      />

      {/* Find bar */}
      {findVisible && (
        <div
          className="find-bar"
          style={{
            height: FIND_BAR_HEIGHT,
            background: "var(--chrome-bg)",
            borderTop: "1px solid var(--chrome-border)",
            display: "flex",
            alignItems: "center",
            padding: "0 8px",
            gap: 8,
            flexShrink: 0,
          }}
        >
          <div
            style={{
              display: "flex",
              alignItems: "center",
              background: "var(--panel-bg)",
              border: "1px solid var(--border)",
              borderRadius: 2,
              padding: "0 6px",
              height: 20,
              width: 200,
            }}
          >
            <input
              ref={findInputRef}
              className="mono"
              value={findQuery}
              onChange={(e) => handleFindInputChange(e.target.value)}
              onKeyDown={handleFindKeyDown}
              placeholder="Find..."
              style={{
                width: "100%",
                background: "transparent",
                border: "none",
                outline: "none",
                color: "var(--text-primary)",
                fontSize: 12,
                fontFamily: "inherit",
              }}
            />
          </div>

          {findQuery && (
            <span
              className="mono"
              style={{ fontSize: 10, color: "var(--text-muted)", minWidth: 60 }}
            >
              {findResults.length === 0
                ? "No matches"
                : `${findIndex + 1}/${findResults.length}`}
            </span>
          )}

          <FindButton onClick={handleFindPrev} disabled={findResults.length === 0}>
            {"\u2039"}
          </FindButton>
          <FindButton onClick={handleFindNext} disabled={findResults.length === 0}>
            {"\u203A"}
          </FindButton>
          <FindButton onClick={handleFindClose}>{
"\u2715"}
          </FindButton>
        </div>
      )}
    </div>
  );
}

// ── Find Button ────────────────────────────────────────────────────────────

function FindButton({
  onClick,
  disabled = false,
  children,
}: {
  onClick: () => void;
  disabled?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      style={{
        width: 20,
        height: 20,
        borderRadius: 2,
        background: "var(--panel-bg)",
        border: "1px solid var(--border)",
        color: disabled ? "var(--text-disabled)" : "var(--text-primary)",
        fontSize: 12,
        cursor: disabled ? "default" : "pointer",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 0,
        lineHeight: 1,
      }}
      onMouseEnter={(e) => {
        if (!disabled) {
          e.currentTarget.style.background = "var(--panel-active-bg)";
        }
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = "var(--panel-bg)";
      }}
    >
      {children}
    </button>
  );
}
