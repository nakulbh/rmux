/**
 * TerminalPane — xterm.js terminal emulator widget.
 *
 * Matches `crates/rmux-app/src/ui/terminal_pane.rs`.
 * Embeds a full xterm.js Terminal with FitAddon for auto-resize.
 * Shows a 1px accent focus border when active.
 * Find bar (Cmd/Ctrl+F): 28px chrome strip with query input,
 * match counter (mono 10px), and prev/next/close buttons.
 */

import { useRef, useEffect, useCallback, useState, type KeyboardEvent } from "react";
import type { PaneId } from "../types";
import "../App.css";

import { Terminal } from "xterm";
import { FitAddon } from "@xterm/addon-fit";
import "xterm/css/xterm.css";

// ── Props ──────────────────────────────────────────────────────────────────

export interface TerminalPaneProps {
  paneId: PaneId;
  isActive: boolean;
}

// ── Find bar height (from terminal_pane.rs) ───────────────────────────────

const FIND_BAR_HEIGHT = 28;

// ── Component ──────────────────────────────────────────────────────────────

export function TerminalPane({ paneId: _paneId, isActive }: TerminalPaneProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);

  // Find bar state
  const [findVisible, setFindVisible] = useState(false);
  const [findQuery, setFindQuery] = useState("");
  const [findResults, setFindResults] = useState<{ row: number; col: number }[]>([]);
  const [findIndex, setFindIndex] = useState(0);
  const findInputRef = useRef<HTMLInputElement>(null);

  // Initialize xterm.js terminal
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

    // Demo welcome message
    term.writeln("rmux \u2014 Terminal Multiplexer");
    term.writeln("");
    term.write("$ ");

    terminalRef.current = term;
    fitAddonRef.current = fit;

    // Resize observer
    const observer = new ResizeObserver(() => {
      fit.fit();
    });
    observer.observe(containerRef.current);

    return () => {
      observer.disconnect();
      term.dispose();
    };
  }, []);

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
        if (!findVisible) {
          // Pre-populate with terminal selection
          const term = terminalRef.current;
          if (term) {
            const sel = term.getSelection();
            if (sel) {
              setFindQuery(sel);
              doFind(sel, term);
            }
          }
        }
      }
      // Forward keyboard to terminal (simplified — full PTY integration in real impl)
      if (terminalRef.current && !findVisible) {
        // In a real implementation, keyboard events would be forwarded to the PTY
        // via Tauri invoke() calls.
      }
    },
    [findVisible]
  );

  // Search the terminal buffer for the query
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

      {/* Find bar (28px chrome strip at bottom) */}
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
          {/* Query input */}
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

          {/* Match count */}
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

          {/* Navigation buttons (20x20) */}
          <FindButton onClick={handleFindPrev} disabled={findResults.length === 0}>
            {"\u2039"}
          </FindButton>
          <FindButton onClick={handleFindNext} disabled={findResults.length === 0}>
            {"\u203A"}
          </FindButton>

          {/* Close button */}
          <FindButton onClick={handleFindClose}>
            {"\u2715"}
          </FindButton>
        </div>
      )}
    </div>
  );
}

// ── Find Button (20x20, panel_bg + 1px border) ────────────────────────────

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
        background: disabled ? "var(--panel-bg)" : "var(--panel-bg)",
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
