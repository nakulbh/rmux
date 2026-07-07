# Key Bindings

rmux keyboard shortcuts. macOS uses `Cmd`, Linux/Windows uses `Ctrl`.

## Global Shortcuts

Always active. Work regardless of focus.

| Shortcut | Action | Notes |
|----------|--------|-------|
| `Cmd/Ctrl+Q` | Quit application | |
| `Cmd/Ctrl+C` | Copy terminal selection | If no selection, `Ctrl+C` sends SIGINT to shell (Linux/Windows only; macOS always copies) |
| `Cmd/Ctrl+F` | Toggle find bar | |
| `Cmd/Ctrl+G` | Find next match | |
| `Alt+Cmd/Ctrl+G` | Find previous match | Matches cmux |
| `Cmd/Ctrl+E` | Use selection for find | |
| `Cmd/Ctrl+K` | Clear terminal scrollback | CSI 3 J |
| `Cmd/Ctrl+Shift+K` | Clear screen (keep scrollback) | Sends `Ctrl+L` to shell. Matches cmux |
| `Cmd/Ctrl++` / `Cmd/Ctrl+=` | Increase font size | Range: 6pt–60pt |
| `Cmd/Ctrl+-` | Decrease font size | Range: 6pt–60pt |
| `Cmd/Ctrl+0` | Reset font size to default (14pt) | |
| `Escape` | Close find bar | When find bar visible |
| `Enter` | Find next match | When find bar visible |

## Workspace Shortcuts

Active when no text widget has focus.

| Shortcut | Action | Notes |
|----------|--------|-------|
| `Cmd/Ctrl+N` | New workspace | |
| `Cmd/Ctrl+B` | Toggle sidebar | |
| `Cmd/Ctrl+I` | Toggle notification panel | Matches cmux (`Cmd+I`) |
| `Cmd/Ctrl+D` | Split pane right | |
| `Cmd/Ctrl+Shift+D` | Split pane down | |
| `Cmd/Ctrl+W` | Close active pane | |
| `Cmd/Ctrl+Shift+W` | Close active workspace | |
| `Cmd/Ctrl+Shift+R` | Rename active workspace | |
| `Cmd/Ctrl+Shift+Enter` | Toggle pane zoom (maximize/restore) | |
| `Cmd/Ctrl+Shift+=` | Equalize all split sizes | |
| `Cmd/Ctrl+Shift+[` | Previous workspace | |
| `Cmd/Ctrl+Shift+]` | Next workspace | |
| `Cmd/Ctrl+1` – `Cmd/Ctrl+9` | Switch to workspace by index | |
| `Cmd/Ctrl+Arrow Left/Up` | Focus previous pane | |
| `Cmd/Ctrl+Arrow Right/Down` | Focus next pane | |

## Terminal Input

When terminal pane has focus, keys forward to shell:

- Printable characters: typed as-is
- `Ctrl+A..Z` (macOS): forwarded as control characters (SIGINT, EOF, etc.)
- `Ctrl+C` (Linux/Windows): SIGINT if no selection, copy if selection exists
- `Alt+char`: sends `ESC` + char (meta prefix)
- Arrow keys, F-keys, Home/End, PgUp/PgDn: ANSI escape sequences
- Scroll wheel: scrollback navigation

### Platform-Specific Ctrl Chord Handling

On macOS, `Cmd` is for app shortcuts and `Ctrl` is for terminal control characters. No conflicts.

On Linux/Windows, `Ctrl` is used for both. rmux resolves this by reserving specific Ctrl chords for app shortcuts — they are NOT forwarded to the shell:

| Reserved (app shortcut) | Forwarded to shell |
|------------------------|--------------------|
| `Ctrl+B` (sidebar) | `Ctrl+A` (line start) |
| `Ctrl+C` (copy/SIGINT) | `Ctrl+R` (reverse search) |
| `Ctrl+D` (split right) | `Ctrl+L` (clear screen) |
| `Ctrl+E` (find selection) | `Ctrl+Z` (suspend) |
| `Ctrl+F` (find) | `Ctrl+P` (previous command) |
| `Ctrl+G` (find next) | `Ctrl+U` (delete line) |
| `Ctrl+K` (clear scrollback) | `Ctrl+W` is reserved (close pane) |
| `Ctrl+N` (new workspace) | `Ctrl+O` (open) |
| `Ctrl+Q` (quit) | `Ctrl+S` (stop output) |
| `Ctrl+W` (close pane) | `Ctrl+X` (various) |
| `Ctrl+0-9` (font/workspace) | `Ctrl+]` (escape) |

**Workaround for Ctrl+D (EOF):** Type `exit` in the shell, or close the pane with `Ctrl+W`.

## Find Bar

When find bar is active:

- Type to search (case-insensitive)
- `Enter`: next match
- `Cmd/Ctrl+G`: next match
- `Alt+Cmd/Ctrl+G`: previous match
- `Escape`: close find bar
- Click `<` / `>` buttons: navigate matches
- Click `x`: close find bar

## Font Size

Range: 6pt to 60pt. Default: 14pt.

Font size change triggers PTY resize for all panes (recalculates grid cols/rows).

## Notes

- `Cmd` on macOS, `Ctrl` on Linux/Windows
- Shortcuts processed after UI render so `ctx.wants_keyboard_input()` works correctly
- Find bar text input handled by egui `TextEdit` widget
- Terminal pane focus tracked via click interaction

---

## cmux Comparison

### Bindings Matching cmux

| rmux Shortcut | cmux Shortcut | Action |
|---------------|---------------|--------|
| `Cmd/Ctrl+Q` | `Cmd+Q` | Quit |
| `Cmd/Ctrl+F` | `Cmd+F` | Find |
| `Cmd/Ctrl+G` | `Cmd+G` | Find next |
| `Alt+Cmd/Ctrl+G` | `Alt+Cmd+G` | Find previous |
| `Cmd/Ctrl+E` | `Cmd+E` | Use selection for find |
| `Cmd/Ctrl+B` | `Cmd+B` | Toggle sidebar |
| `Cmd/Ctrl+N` | `Cmd+N` | New workspace |
| `Cmd/Ctrl+D` | `Cmd+D` | Split right |
| `Cmd/Ctrl+Shift+D` | `Cmd+Shift+D` | Split down |
| `Cmd/Ctrl+W` | `Cmd+W` | Close tab/pane |
| `Cmd/Ctrl+Shift+W` | `Cmd+Shift+W` | Close workspace |
| `Cmd/Ctrl+Shift+R` | `Cmd+Shift+R` | Rename workspace |
| `Cmd/Ctrl+Shift+Enter` | `Cmd+Shift+Enter` | Toggle pane zoom |
| `Cmd/Ctrl+Shift+=` | `Ctrl+Cmd+=` | Equalize splits |
| `Cmd/Ctrl+1-9` | `Cmd+1-9` | Select workspace |
| `Cmd/Ctrl+I` | `Cmd+I` | Show notifications |
| `Cmd/Ctrl+Shift+K` | `Cmd+Shift+K` | Clear screen |

### Differences from cmux

| rmux | cmux | Reason |
|------|------|--------|
| `Cmd/Ctrl+Shift+[` / `]` | `Ctrl+Cmd+[` / `]` | Workspace switch — different modifier order |
| `Cmd/Ctrl+Arrow` | `Alt+Cmd+Arrow` | Pane focus — rmux uses simpler chord |
| `Cmd/Ctrl+K` | — | Clear scrollback (rmux-specific, not in cmux) |
| `Cmd/Ctrl+C` | — | Copy selection (cmux uses `Cmd+C` implicitly) |
| `Cmd/Ctrl++/-/0` | — | Font size (cmux uses `Cmd+=` for browser zoom) |

---

## Future Bindings (Not Yet Implemented)

### Requires Command Palette (Phase 5+)

| cmux Shortcut | Action | Status |
|---------------|--------|--------|
| `Cmd+Shift+P` | Command palette | Not implemented |
| `Ctrl+N` | Palette next result | Not implemented |
| `Ctrl+P` | Palette previous result | Not implemented |

### Requires Settings UI (Phase 4+)

| cmux Shortcut | Action | Status |
|---------------|--------|--------|
| `Cmd+,` | Open settings | Not implemented |
| `Cmd+Shift+,` | Reload configuration | Not implemented |

### Requires Surfaces/Tabs (Phase 5+)

| cmux Shortcut | Action | Status |
|---------------|--------|--------|
| `Cmd+T` | New surface/tab | Not implemented |
| `Cmd+Shift+]` | Next surface | Not implemented (used for workspace switch) |
| `Cmd+Shift+[` | Previous surface | Not implemented (used for workspace switch) |
| `Ctrl+1-9` | Select surface by index | Not implemented |
| `Cmd+R` | Rename tab | Not implemented |
| `Cmd+Shift+T` | Reopen last closed | Not implemented |
| `Cmd+Shift+M` | Toggle copy mode | Not implemented |
| `Cmd+Shift+A` | Switch terminal/TextBox focus | Not implemented |

### Requires Browser Pane (Phase 4)

| cmux Shortcut | Action | Status |
|---------------|--------|--------|
| `Cmd+Shift+L` | Open browser | Not implemented |
| `Cmd+L` | Focus address bar | Not implemented |
| `Cmd+[` / `Cmd+]` | Browser back/forward | Not implemented |
| `Cmd+R` | Reload page | Not implemented |
| `Cmd+Shift+R` | Hard refresh | Not implemented (used for rename) |
| `Alt+Cmd+I` | Toggle devtools | Not implemented |
| `Alt+Cmd+C` | JS console | Not implemented (used for copy) |
| `Alt+Cmd+Enter` | Browser focus mode | Not implemented |
| `Alt+Cmd+D` | Split browser right | Not implemented |
| `Alt+Cmd+Shift+D` | Split browser down | Not implemented |
| `Alt+Cmd+N` | New browser workspace | Not implemented |

### Requires Canvas Layout (Future)

| cmux Shortcut | Action | Status |
|---------------|--------|--------|
| `Ctrl+Cmd+C` | Toggle canvas layout | Not implemented |
| `Ctrl+Cmd+R` | Reveal focused pane | Not implemented |
| `Ctrl+Cmd+O` | Toggle overview zoom | Not implemented |
| `Alt+Cmd+=` / `Alt+Cmd+-` | Canvas zoom in/out | Not implemented |
| `Ctrl+Cmd+T` | Tidy panes into grid | Not implemented |

### Requires Workspace Groups (Future)

| cmux Shortcut | Action | Status |
|---------------|--------|--------|
| `Ctrl+Cmd+G` | New empty group | Not implemented |
| `Cmd+Shift+G` | Group selected workspaces | Not implemented |
| `Ctrl+Cmd+.` | Collapse/expand group | Not implemented |
| `Cmd+Shift+E` | Toggle right-sidebar focus | Not implemented |

### Requires File Explorer (Future)

| cmux Shortcut | Action | Status |
|---------------|--------|--------|
| `Alt+Cmd+B` | Toggle file explorer | Not implemented |
| `Cmd+O` | Open folder | Not implemented |
| `J` / `K` / `H` / `L` | Navigate file rows | Not implemented |
| `Enter` | Open file / toggle folder | Not implemented |
| `Cmd+ArrowDown` | Open file (Finder-style) | Not implemented |

### Requires Session Restore (Phase 5+)

| cmux Shortcut | Action | Status |
|---------------|--------|--------|
| `Cmd+Shift+O` | Reopen previous session | Not implemented |

### Requires Diff Viewer (Future)

| cmux Shortcut | Action | Status |
|---------------|--------|--------|
| `Ctrl+Cmd+Shift+D` | Open diff viewer | Not implemented |
| `J` / `K` | Scroll diff | Not implemented |
| `Shift+G` | Scroll to bottom | Not implemented |
| `G+G` | Scroll to top | Not implemented |
| `/` | Open diff file search | Not implemented |

### Requires Window Management

| cmux Shortcut | Action | Status |
|---------------|--------|--------|
| `Cmd+Shift+N` | New window | Not implemented |
| `Ctrl+Cmd+W` | Close window | Not implemented |
| `Ctrl+Cmd+F` | Toggle fullscreen | Not implemented |
| `Alt+Cmd+F` | Global search | Not implemented |

### Requires Workspace Switcher

| cmux Shortcut | Action | Status |
|---------------|--------|--------|
| `Cmd+P` | Go to workspace (switcher) | Not implemented |

### Requires Find in Directory

| cmux Shortcut | Action | Status |
|---------------|--------|--------|
| `Cmd+Shift+F` | Find in directory | Not implemented |
| `Alt+Cmd+Shift+F` | Hide find bar | Not implemented (use Escape) |

### Requires Focus History

| cmux Shortcut | Action | Status |
|---------------|--------|--------|
| `Cmd+[` | Focus back | Not implemented (used for workspace switch) |
| `Cmd+]` | Focus forward | Not implemented (used for workspace switch) |

### Requires Notification Management

| cmux Shortcut | Action | Status |
|---------------|--------|--------|
| `Cmd+Shift+U` | Jump to latest unread | Not implemented |
| `Alt+Cmd+U` | Toggle unread state | Not implemented |
| `Ctrl+Cmd+U` | Mark oldest unread | Not implemented |
| `Cmd+Shift+H` | Flash focused panel | Not implemented |
