# rmux UI Redesign — Arbor/cmux Design Language

> Source-of-truth spec for the UI restyle. Derived from source-level research of
> [penso/arbor](https://github.com/penso/arbor) (GPUI, One Dark default theme) and
> [manaflow-ai/cmux](https://github.com/manaflow-ai/cmux) (Swift/AppKit).
> Every builder agent implements against THIS document. Exact values only — no improvisation.

---

## Design Principles (why these apps look good)

1. **Three-surface depth model, zero shadows.** Content (darkest) vs chrome (mid) vs
   interaction/hover (lightest). Separation via 1px borders only.
2. **One accent color does all "active" work** — selection border, focus ring, caret,
   active-tab indicator, progress fill, unread badges. Red/green/yellow/blue reserved
   strictly for status semantics. Chrome stays neutral.
3. **High density, strict rhythm.** 24–34px control heights, 11–12px body text, 2px corner
   radii on rows/buttons (6px only for popovers/overlays), 4/8px spacing grid.
4. **Mono-first identity.** Sidebar metadata, badges, status text share the terminal's
   monospace face. Chrome labels are proportional.
5. **Motion restraint.** No slides/bounces. Opacity de-emphasis (inactive rows 0.8) and
   hover fills only.

---

## Palette (Arbor "One Dark" + cross-theme status colors)

Defined in `crates/rmux-app/src/ui/theme.rs` as `Palette`. New canonical fields:

| Field | Hex | Usage |
|---|---|---|
| `app_bg` | `#282c33` | window root, gaps between panes |
| `terminal_bg` | `#282c34` | center pane / terminal background |
| `sidebar_bg` | `#2f343e` | left sidebar AND right notification panel fill |
| `panel_bg` | `#2e343e` | cards (workspace rows, notification rows), buttons, inputs, badges |
| `panel_active_bg` | `#363c46` | hover + selected background everywhere |
| `chrome_bg` | `#3b414d` | top bar, status bar, overlays (zoom indicator, find bar) |
| `chrome_border` | `#464b57` | 1px line under top bar & above status bar |
| `tab_active_bg` | `#282c33` | active tab fill (matches content bg) |
| `border` | `#363c46` | ALL other 1px borders/separators/dividers |
| `text_primary` | `#c8ccd4` | primary text, active labels |
| `text_muted` | `#838994` | secondary text, inactive labels, icons |
| `text_disabled` | `#696b77` | timestamps, placeholders, hints |
| `accent` | `#74ade8` | selection borders, focus, caret, badges, progress, spinners |
| `accent_fg` | `#1d2127` | text on accent-filled elements |
| `success` | `#72d69c` | additions, "Serving", success checks |
| `danger` | `#eb6f92` | deletions, errors, exited processes, destructive actions |
| `warning` | `#e5c07b` | "Working" status, pending |
| `info` | `#61afef` | "Waiting"/attention (cmux notification-ring blue) |
| `terminal_cursor` | `#ebdbb2` | terminal cursor overlay |
| `terminal_selection_bg` | `#3e4451` | terminal selection |

Legacy shim fields (`background`, `card`, `muted_foreground`, `ring`, `destructive`, …)
remain temporarily so unmigrated modules compile; they alias the new values.
**New/rewritten code MUST use the new field names.** Shims deleted after all modules migrate.

## Typography

egui text styles (set in `Theme::apply`):

| Style | Size | Family |
|---|---|---|
| Small | 10.0 | Proportional |
| Body | 12.0 | Proportional |
| Button | 12.0 | Proportional |
| Monospace | 12.0 | Monospace |
| Heading | 14.0 | Proportional |

Component-level sizes (explicit `FontId` at call sites):
- 10px mono: sidebar metadata lines (pane count, status), timestamps, hints
- 11px: status bar segments, top-bar button labels
- 12px: dominant size — row titles, notification titles, buttons, find bar
- 12.5px semibold-ish (use `FontId` 12.5 + `text_primary`): sidebar workspace title
- 14px: top-bar centered window title (strong/semibold)
- 9px mono: count badge text

## Metrics

| Token | Value |
|---|---|
| `radius_sm` | 2.0 — rows, buttons, inputs, cards, tabs |
| `radius_md` | 6.0 — popovers, zoom indicator, overlays |
| Top bar height | 34.0 |
| Status bar height | 26.0 |
| Sidebar default width | 240 (min 200, max 320), resizable |
| Notification panel width | 280 default (min 240, max 340) |
| Sidebar row padding | 8px horizontal, 6px vertical; 2px gap between rows |
| Control height | buttons 24, inputs 28, sidebar header rows 32 |
| Border width | 1.0 everywhere; focus/selection also 1.0 in `accent` |
| Split divider | 1px drawn line in `border` color (not empty gap) |
| item_spacing | (4, 4); button_padding (8, 4); interact_size.y 24 |

---

## Window Layout

```
┌──────────────────────────────────────────────────────────────────┐
│ TOP BAR h=34 chrome_bg                                           │
│ [☰]        centered: "workspace-name" 14px strong        [🔔 n]  │
├──────────────── 1px chrome_border ───────────────────────────────┤
│ SIDEBAR w=240   │ CENTER terminal_bg          │ NOTIF PANEL w=280│
│ sidebar_bg      │ pane tree, 1px drawn        │ sidebar_bg       │
│ workspace cards │ dividers, accent focus ring │ notif cards      │
│ + footer button │                             │ (toggle Cmd+I)   │
├──────────────── 1px chrome_border ───────────────────────────────┤
│ STATUS BAR h=26 chrome_bg  ● workspace • N panes    M unread•ready│
└──────────────────────────────────────────────────────────────────┘
```

egui composition order in `RmuxApp::update` (order matters):
1. `TopBottomPanel::top("rmux_top_bar")` — 34px
2. `TopBottomPanel::bottom("rmux_status_bar")` — 26px
3. `SidePanel::left("rmux_sidebar")`
4. `SidePanel::right("rmux_notification_panel")` (if visible)
5. `CentralPanel` — pane tree

---

## Module Specs & Ownership

Each module below is owned by EXACTLY ONE builder agent. Do not edit files outside your
ownership. `theme.rs`, `app.rs`, `ui/mod.rs` are frozen after the foundation commit
(exception: chrome agent may adjust the top/status bar call sites in `app.rs` if signatures
must change).

### A. `ui/sidebar.rs` — workspace list (agent: sidebar)

Arbor worktree-card + cmux vertical-tab hybrid.

- Panel: `sidebar_bg` fill, 1px `border` right edge, inner margin 8, default 240 min 200
  max 320, resizable.
- Header row: `"Workspaces"` 11px `text_muted`, with workspace count pill on the right —
  pill: `panel_bg` fill, 1px `border`, fully rounded, h=14, min-w 14, 9px mono text.
  No separator line below (density; spacing suffices).
- Workspace card rows (2px gap, in ScrollArea):
  - Card: `panel_bg` fill, 1px border — `accent` border + `panel_active_bg` fill when
    active; plain `border` otherwise. Radius 2. Padding: 8px h, 6px v. Height ~42
    (2 lines) or ~52 with status line.
  - Inactive rows painted at 0.8 opacity (use `gamma_multiply(0.8)` on text/border colors).
  - Hover (inactive): `panel_active_bg` fill.
  - Line 1: workspace name 12.5px `text_primary` (ellipsize), right-aligned unread badge:
    `accent`-filled circle r=8, count 9px `accent_fg`.
  - Line 2 (mono 10px `text_muted`): `"N panes"` + `" · "` + status text if present
    (status in `warning` color when present — cmux "Working" pattern).
  - Progress (if `Some`): 3px capsule along card bottom, `accent` fill on
    `border`-color track, width = card width × progress.
  - NO left accent stripe (old design) — selection is border + fill now.
- Inline rename: same card rect, input style — `panel_bg` fill, 1px `accent` border when
  focused (else `border`), mono 12px text.
- Footer (bottom-up): 1px `border` hline, then full-width `"+ New Workspace"` button:
  h=24, radius 2, `panel_bg` + 1px `border`, hover → `panel_active_bg` + `accent` border;
  label 12px: `+` in `accent` + text in `text_primary`. Triggers same path as Cmd+N —
  emit via return value or callback (see signature note below). Below it the toggle hint
  10px `text_disabled` ("⌘B to toggle").
- REMOVE the bell button from the sidebar (bell moves to top bar). Keep the
  `notification_panel_visible: &mut bool` parameter (underscore-rename) so `app.rs` stays
  untouched.
- New-workspace button: sidebar cannot call `create_workspace_with_terminal` (that's on
  `RmuxApp`). Have `show()` return a small `SidebarAction` enum
  (`None | CreateWorkspace | SwitchTo(usize)…` — minimal: just `create_requested: bool`
  return) — pick the least invasive shape; `app.rs` may NOT be edited, so if a return
  value can't be consumed without editing `app.rs`, instead spawn the workspace via
  `WorkspaceManager::create_workspace` + document limitation (no terminal attach) — in
  that case SKIP the button action and render it disabled with tooltip "use ⌘N".
  Prefer the simplest compiling option.

### B. `ui/terminal_pane.rs` + `ui/workspace_view.rs` + `rmux-terminal/src/renderer.rs` (agent: terminal)

- Workspace view background: `app_bg`.
- Split dividers: keep 1px gap but DRAW a 1px line in `border` color in the gap
  (`painter.line_segment` or 1px rect) — arbor's visible hairline.
- Focus indication: active pane gets 1px `accent` `rect_stroke` (StrokeKind::Inside,
  CornerRadius::ZERO). Replace old 2px gray ring.
- Attention ring (cmux): if pane has unread notifications, draw 2.5px `info` stroke,
  radius 6, inset 2 inside pane edge — ONLY if pane-level unread state is already
  reachable from existing params; do not add new cross-module plumbing. If not reachable,
  skip.
- Loading placeholder: `panel_bg` fill, 1px `border`, "Spawning terminal…" mono 12
  `text_muted`.
- Zoom indicator: top-right pill — `chrome_bg` fill, 1px `chrome_border`, radius 6,
  text 10px `text_muted`.
- Find bar: h=28 strip at pane bottom — `chrome_bg` fill, 1px `chrome_border` top edge.
  Input: `panel_bg`, 1px border → `accent` when focused, mono 12, radius 2, width 200.
  Match count mono 10 `text_muted`. Buttons `‹ › ✕`: 20×20, radius 2, `panel_bg` +
  1px `border`, hover `panel_active_bg`.
- Find highlights: `warning.gamma_multiply(0.35)` matches, `accent.gamma_multiply(0.45)`
  active match.
- Exit state: centered `"Process exited (code: N)"` mono 13 `danger`.
- Renderer (`rmux-terminal/src/renderer.rs`): replace hardcoded white cursor colors with
  Arbor values — block/hollow `#ebdbb2` at alpha 128, underline/beam `#ebdbb2` alpha 200.
  Keep the renderer decoupled (no theme import — literal Color32 constants are fine).

### C. `ui/notification_panel.rs` (agent: notifications)

- Panel: `sidebar_bg` fill, 1px `border` left edge, inner margin 8.
- Header: `"Notifications"` 12px `text_primary` strong + unread count pill (same pill
  spec as sidebar). Action row: `"Mark all read"` / `"Clear"` buttons h=22, radius 2,
  `panel_bg` + 1px `border`, hover `panel_active_bg`, 11px labels. Thin `border` hline
  below header block.
- Rows (2px gap): card `panel_bg`, radius 2, 1px `border`, hover `panel_active_bg`.
  - Unread: 2px `accent` stripe on card's left edge (inside border) + title in
    `text_primary`; read: no stripe, title `text_muted`, card at 0.85 opacity.
  - Line 1: title 12px + right-aligned relative time 10px `text_disabled`.
  - Line 2: body 10.5px `text_muted`, single line ellipsized.
- Empty state: centered "No notifications" 12px `text_muted` + sub-hint 10px
  `text_disabled`.

### D. `ui/top_bar.rs` + `ui/status_bar.rs` (agent: chrome)

Foundation lands functional stubs; this agent brings them to spec.

Top bar (34px, `chrome_bg`, 1px `chrome_border` bottom):
- Left (x offset ~76 to clear macOS traffic lights; use 12 on non-mac):
  sidebar-toggle button `☰` 20×20, radius 2, no fill, icon 12px `text_muted` →
  `text_primary` hover; when sidebar hidden, icon in `accent`.
- Center (absolute center of bar): `"{active workspace name}"` 14px strong
  `text_primary`; if >1 pane append `" · N panes"` 11px `text_muted`.
- Right (right inset 12, gap 6): bell button — h=22 px=6 radius 2, 1px `border`,
  `chrome_bg` fill, hover `panel_bg`; `🔔` + unread count 11px (count in `accent` when
  >0). Click toggles notification panel.
- The whole bar area does NOT need window-drag support (egui limitation acceptable).

Status bar (26px, `chrome_bg`, 1px `chrome_border` top, px=8):
- All text 11px `text_muted`, segments joined with `" • "` literal separators.
- Left: `●` in `success` + `" {workspace name}"` + `" • {N} panes"`.
- Right: `"{M} workspaces"` + `" • {K} unread"` (in `accent` if K>0) + `" • ready"`.

### Frozen after foundation: `theme.rs`, `app.rs`, `ui/mod.rs`

Foundation commit contains: new `Palette` + legacy shims, `Theme::apply` visuals
(panel_fill = `app_bg`, widget styling per metrics above), top/status bar stubs wired
into `app.rs`, this document.

---

## Verification bar (every agent)

- `cargo check --workspace` green.
- `cargo clippy --workspace --all-targets` no new warnings.
- `cargo fmt --all` applied.
- Existing tests pass: `cargo test --workspace`.
- Commit all work on your branch with a descriptive message.

---

## Keyboard Shortcuts

macOS uses Cmd where Linux and Windows use Ctrl.

| Action | macOS | Linux/Windows |
|---|---|---|
| Quit | Cmd+Q | Ctrl+Q |
| Find | Cmd+F | Ctrl+F |
| Find Next | Cmd+G | Ctrl+G |
| Find Next, when find is visible | Enter | Enter |
| Find Previous, when find is visible | Cmd+Option+G | Ctrl+Alt+G |
| Close Find Bar | Escape | Escape |
| Use Selection for Find | Cmd+E | Ctrl+E |
| Clear Scrollback | Cmd+K | Ctrl+K |
| Clear Screen | Cmd+Shift+K | Ctrl+Shift+K |
| Toggle Sidebar | Cmd+B | Ctrl+B |
| Toggle Notifications | Cmd+I | Ctrl+I |
| New Workspace | Cmd+N | Ctrl+N |
| Split Right | Cmd+D | Ctrl+D |
| Split Down | Cmd+Shift+D | Ctrl+Shift+D |
| Close Pane | Cmd+W | Ctrl+W |
| Close Workspace | Cmd+Shift+W | Ctrl+Shift+W |
| Rename Workspace | Cmd+Shift+R | Ctrl+Shift+R |
| Toggle Zoom | Cmd+Shift+Enter | Ctrl+Shift+Enter |
| Equalize Splits | Cmd+Shift+= | Ctrl+Shift+= |
| Previous Workspace | Cmd+Shift+[ | Ctrl+Shift+[ |
| Next Workspace | Cmd+Shift+] | Ctrl+Shift+] |
| Switch to Workspace 1 through 9 | Cmd+1 through Cmd+9 | Ctrl+1 through Ctrl+9 |
| Focus Left | Cmd+Left Arrow | Ctrl+Left Arrow |
| Focus Up | Cmd+Up Arrow | Ctrl+Up Arrow |
| Focus Right | Cmd+Option+Right Arrow | Ctrl+Alt+Right Arrow |
| Focus Down | Cmd+Option+Down Arrow | Ctrl+Alt+Down Arrow |
| Open Browser Split | Cmd+Shift+L | Ctrl+Shift+L |
| Focus Browser URL Bar | Cmd+L | Ctrl+L |
| Reload Browser | Cmd+R | Ctrl+R |
| Increase Font Size | Cmd++ or Cmd+= | Ctrl++ or Ctrl+= |
| Decrease Font Size | Cmd+- | Ctrl+- |
| Reset Font Size | Cmd+0 | Ctrl+0 |
| Copy, when text is selected | Cmd+C | Ctrl+C |
