# fix-keybindings — Worker Notepad

Shared notebook for todos in `feat/fix-keybindings`. Append-only.

---

## W1.3 — Escape/Enter event leak fix

**Worker:** Atlas (Rust Core Specialist) · **Date:** 2026-07-13

### What landed

- New pure helper `should_dispatch_when_text_focused(action) -> bool` in
  `crates/rmux-app/src/shortcut_handler.rs`. Returns `true` for the 7
  always-active actions (Quit, Copy, FontSize{Up,Down,Reset}, ClearScreen,
  ClearScrollback) and `false` for everything else.
- `handle_keyboard_shortcuts` collapsed from a dual-pass loop into a single
  pass. The guard
  `if ctx.wants_keyboard_input() && !should_dispatch_when_text_focused(action) { continue; }`
  lives at the top of the loop body, after the registry lookup. This
  eliminates the duplicated modifier-normalization block and the early
  `return` on `wants_keyboard_input`.
- 15 unit tests at the bottom of `shortcut_handler.rs` covering all 7
  always-active + 8 focus-dependent variants.

### Why it fixes the leak

`Escape` and `Enter` are registered with `Modifiers::NONE` and bound to
`Find` / `FindNext`. The old first pass iterated all events without
checking `wants_keyboard_input()`, so even when the action was a no-op
(find bar not visible), the event was *consumed* by the shortcut handler
and never reached the terminal. Now the guard skips `Find`/`FindNext` (and
all other focus-dependent actions) whenever a text widget reports
`wants_keyboard_input() == true` — the terminal's keyboard claim is honored.

### Verification

- `cargo test -p rmux-app shortcut_handler` → 15 passed, 0 failed
- `cargo build -p rmux-app 2>&1 | grep -c "error\[E"` → `0`
- Diff: +117 / -46 in `shortcut_handler.rs`

### Gotchas hit

- The `ShortcutAction` enum already includes cmux variants on this branch
  (Todo 1 merged), so the existing `dispatch_shortcut_action` match needs a
  `_ => {}` catch-all to stay exhaustive. Added with a comment pointing at
  Todo 14 for real handlers. Without it, `cargo build` fails.
- Tests intentionally cover only EXISTING variants per task instructions
  (no `NewSurface` etc.). Todo 14 dispatch tests can extend coverage once
  the helper's `false` branch is verified for cmux actions.

### Follow-ups (not done here)

- Todo 14: add dispatch tests that exercise `handle_keyboard_shortcuts`
  through the egui event pipeline to verify the guard actually skips
  events for focus-dependent actions when `wants_keyboard_input` is true.
- Todo 16 (dispatch tests) can add cmux-variant coverage for
  `should_dispatch_when_text_focused` (e.g. `NewSurface=false`) once
  explicit cmux arms land.

---

## W1.4 — Focus modifier fix (cmd_ctrl_alt for all 4 directions)

**Worker:** Atlas (Rust Core Specialist) · **Date:** 2026-07-13

### What landed

- All four focus registrations in `ShortcutRegistry::default` now use
  `cmd_ctrl_alt()`:
  - `FocusLeft`  → `cmd_ctrl_alt()` + `Key::ArrowLeft`
  - `FocusUp`    → `cmd_ctrl_alt()` + `Key::ArrowUp`
  - `FocusRight` → `cmd_ctrl_alt()` + `Key::ArrowRight` (unchanged)
  - `FocusDown`  → `cmd_ctrl_alt()` + `Key::ArrowDown` (unchanged)
- 3 new tests in `crates/rmux-app/src/shortcuts.rs::tests`:
  - `test_focus_modifiers_all_match_cmd_ctrl_alt` — asserts all 4 lookups
    resolve with `cmd_ctrl_alt()`.
  - `test_focus_left_not_registered_for_bare_cmd_ctrl` — asserts the bare
    `cmd_ctrl()` + `ArrowLeft` is `None` (regression guard).
  - `test_focus_up_not_registered_for_bare_cmd_ctrl` — asserts the bare
    `cmd_ctrl()` + `ArrowUp` is `None` (regression guard).

### Why

- cmux uses `⌥⌘Arrow` for every direction. The pre-fix inconsistency
  (Left/Up = `Cmd+Arrow`, Right/Down = `Cmd+Opt+Arrow`) meant the user
  could not navigate with the same muscle memory in all 4 directions.

### TDD cycle observed

1. Wrote test first — confirmed `FAILED` with
   `left: None, right: Some(FocusLeft)`.
2. Swapped `cmd_ctrl()` → `cmd_ctrl_alt()` for `FocusLeft` and `FocusUp`.
3. Re-ran — all 6 tests pass.
4. `cargo build -p rmux-app 2>&1 | grep -c "error\[E"` → `0`.

### Reusable patterns

- The registry is an exact-match `HashMap<(Modifiers, Key), ShortcutAction>`,
  so an absent chord returns `None` cleanly. That makes "should NOT be
  registered" assertions (`assert_eq!(lookup(mods, key), None)`) a good
  regression lock — no internal state inspection needed.
- `cmd_ctrl()`, `cmd_ctrl_shift()`, `cmd_ctrl_alt()` are `pub(crate)` and
  re-exported into the test module via `use super::*`, so tests can reuse
  the platform-correct helpers without re-implementing the
  `cfg!(target_os = "macos")` dance.
- The exact-match model means adding a `cmd_ctrl_alt()` registration does
  NOT make a `cmd_ctrl()` lookup also match — they are independent keys.
  The negative regression tests are therefore valid and necessary.

### Gotchas

- The pre-existing `CloseWindow` dead-code warning + 2 unused-helper
  warnings (`cmd_alt_shift`, `ctrl_only`) are NOT caused by this change.
  They predate W1.4 and are out of scope.
- The dispatcher in `RmuxApp::handle_keyboard_shortcuts` (separate
  function) is what actually forwards the matched action; this change only
  touches the *registration* keys. Todo 15 (comprehensive registry tests)
  should verify the dispatcher still wires `FocusLeft/Up` to the correct
  handler now that the chord has changed.
- The `keyboard-shortcuts` table in `README.md` will need its
  `Focus Left` / `Focus Up` rows updated to `Cmd+Opt+Arrow` / `Ctrl+Alt+Arrow`
  to match the new registry. That doc change is OUT of scope here.

---

## W1.2 — Registry registrations for cmux shortcuts

**Worker:** Atlas (Rust Core Specialist) · **Date:** 2026-07-13

### What landed

- 14 new `reg.register(...)` lines in `ShortcutRegistry::default()` (22 actual
  chord mappings — the `⌃1..9` loop expands to 9 unique mappings).
- 2 new `pub(crate)` helper functions in `crates/rmux-app/src/shortcuts.rs`:
  - `cmd_alt_shift()` — `cmd_alt() | Modifiers::SHIFT` (matches the
    `cmd_ctrl_shift` / `cmd_ctrl_alt` style of one-line docstrings).
  - `ctrl_only()` — `Modifiers::CTRL` always. Distinct from `cmd_ctrl()`
    because on macOS `cmd_ctrl()` returns `Modifiers::COMMAND`, but cmux
    uses ⌃1..9 (plain physical Control) for surface selection.
- TODO comment added above the existing
  `cmd_ctrl() + Key::R → ReloadBrowser` registration documenting the
  Cmd+R / RenameTab disambiguation that the dispatcher (Todo 14) must
  resolve. `RenameTab` is NOT bound to Cmd+R in this registry because the
  HashMap would silently overwrite `ReloadBrowser`. A second `NOTE:`
  block in the cmux section explicitly calls out the omission so a future
  maintainer does not assume `RenameTab` was forgotten.
- Same pattern for `cmd_ctrl() + Key::W`: `CloseTab` is registered AFTER
  the existing `ClosePane` on the same chord, intentionally overwriting
  it. A comment explains that the dispatcher decides
  `ClosePane` (single-pane) vs `CloseTab` (multi-surface) based on tab
  count. Without the comment, the duplicate registration looks like a
  bug.

### Why the helpers are necessary

- `cmd_alt_shift()` — needed for `⌥⌘⇧D → SplitBrowserDown`. Mirrors the
  existing `cmd_ctrl_shift()` / `cmd_ctrl_alt()` helpers. The W1.4
  notepad noted these as pre-existing dead-helper warnings that were
  "out of scope" — this W1.2 PR resolves them by giving them callers.
- `ctrl_only()` — the only helper with a multi-line docstring, because
  the macOS-vs-Linux/Windows semantic distinction is genuinely
  non-obvious. Without the docstring, a future maintainer would likely
  "simplify" `ctrl_only()` to `cmd_ctrl()`, which would silently break
  ⌃1..9 surface selection on macOS (lookup would store
  `Modifiers::COMMAND` but the user's actual keypress resolves to
  `Modifiers::CTRL` after the handler's `m.ctrl = false` normalization).

### Verification

- `grep -c "reg.register" crates/rmux-app/src/shortcuts.rs` → `48`
  (line count; 22 actual new mappings including 9 from the loop)
- `cargo test -p rmux-app shortcuts::tests` → 6 passed, 0 failed
- `cargo build -p rmux-app 2>&1 | grep -c "error\[E"` → `0`
- Only remaining warnings: 3 `dead_code` on `RenameTab`, `NewWindow`,
  `CloseWindow` enum variants (expected — dispatcher in Todo 14 will
  construct them).

### HashMap overwrite semantics — important for future maintainers

The registry is `HashMap<(Modifiers, Key), ShortcutAction>` with last-write-wins.
This means the *order* of `reg.register` calls matters when two registrations
target the same chord. In this PR:
- `cmd_ctrl() + Key::W`: first registered as `ClosePane` (line 209),
  then re-registered as `CloseTab` (line 313, new cmux section).
  **Effective binding: `CloseTab`.** Dispatcher must disambiguate.
- `cmd_ctrl() + Key::R`: only registered as `ReloadBrowser` (line 226).
  `RenameTab` is intentionally NOT registered. Dispatcher must surface a
  rename-tab action when no browser is focused.
- `cmd_ctrl_shift() + Key::R`: `RenameWorkspace` (line 247, unchanged).
  No conflict.

### Follow-ups (not done here)

- **Todo 14 (dispatch handlers):** the dispatcher in
  `shortcut_handler.rs::dispatch_shortcut_action` must be extended to
  handle the 14 new `ShortcutAction` variants. It already has a `_ => {}`
  catch-all (per W1.3), so the build is clean, but no cmux behavior
  fires until real arms land. Specifically:
  - `CloseTab` arm should call `ClosePane` logic when the active workspace
    has exactly one surface, and close-tab logic otherwise.
  - `ReloadBrowser` arm should detect whether the active surface is a
    browser and dispatch accordingly for Cmd+R.
- **Todo 15 (comprehensive tests):** the 6 existing tests in
  `shortcuts.rs::tests` only cover `Quit`, `SwitchWorkspace`, and the
  focus-modifier regression cases. Tests for the new cmux registrations
  should follow the same pattern as `test_focus_left_not_registered_for_bare_cmd_ctrl`:
  assert both the positive lookup (correct modifiers) and negative
  lookups (no other modifier combo hits the same action).
- **README.md keyboard-shortcuts table:** needs 14+ new rows for the cmux
  chords. The current table is up to date as of `c1f26fb feat(ui): port
  cmux UI/UX polish into rmux` but does not yet cover the W1.2 set.
  Consider whether to also note the platform-specific behavior of
  `⌃1..9` (Ctrl on macOS, Ctrl on Linux/Windows — same keys, different
  muscle memory because macOS users map ⌃1..9 to app selection, while
  rmux here maps it to surface selection, which may collide with
  Mission Control shortcuts system-wide).

---

## W2.1 — Surface struct + Leaf with `Vec<Surface>`

**Worker:** Atlas (Rust Core Specialist) · **Date:** 2026-07-13

### Refactor path taken

Struct variant (NOT the tuple-variant refactor). The spec offered a
choice between keeping `PaneNode::Leaf { pane: TerminalPane, active_surface, surfaces }`
or going fully tuple-y with `PaneNode::Leaf(Vec<Surface>)`. Struct
variant won because:

1. The 17 existing pattern matches in `splits.rs` and `workspace_view.rs`
   only need `..` added to each arm — the variant shape is preserved.
   The tuple variant would have required rewriting all 17 matches plus
   the `Debug` impl, the `find_terminal*` family, `process_pty_outputs`,
   `find_leaf`, `leaf_panes{,_mut}`, `collect_exited_panes`, etc.
2. `TerminalPane` is non-`Clone` (PTY handles aren't cloneable), so the
   tuple variant forces every `Leaf` to have at least one `Surface`,
   which would have required either a default `TerminalPane` impl or
   breaking the pre-existing `new_leaf(id)` constructor that builds
   a leaf with no terminal yet (used by 8+ tests in `splits.rs`).

The legacy `terminal: Box<Option<TerminalPane>>` slot is preserved
alongside the new `surfaces: Vec<Surface>` field. A leaf with empty
`surfaces` represents the "uninitialized" state where the old
`set_terminal` flow still owns the terminal slot. Once
`Workspace::set_terminal` is migrated (future wave) to call
`add_surface` instead, the legacy slot can be deleted.

### What landed

**New module:** `crates/rmux-app/src/workspace/surface.rs` (~134 lines).

- `SurfaceId(pub u64)` derives `Copy, Clone, Eq, PartialEq, Debug, Hash`.
- `Surface { id: SurfaceId, title: String, terminal: TerminalPane }`
  with `Surface::new(id, title, terminal)` and
  `Surface::display_title() -> &str` (truncates to 24 chars via
  `char_indices` to avoid mid-codepoint slicing).
- 4 tests: `test_surface_creation`,
  `test_surface_display_title_truncates_long_titles`,
  `test_surface_display_title_returns_empty_for_empty_title`,
  `test_surface_id_uniqueness`.

**`PaneNode::Leaf` now has 4 fields instead of 2:**

```rust
Leaf {
    id: PaneId,
    terminal: Box<Option<TerminalPane>>,  // legacy
    active_surface: usize,                 // new
    surfaces: Vec<Surface>,                // new
}
```

**10 new accessor methods on `PaneNode`:**
`leaf_surfaces`, `leaf_surfaces_mut`, `active_surface_index`,
`set_active_surface_index`, `add_surface`, `remove_surface`,
`active_surface`, `active_surface_mut`, `active_terminal`,
`active_terminal_mut`, `terminal_count` (that's 11; the spec required
10+).

**5 new tests in `splits::tests`:**
`test_leaf_holds_multiple_surfaces`,
`test_active_surface_default_is_zero`,
`test_remove_surface_clamps_active_index`,
`test_active_terminal_returns_active_surface_terminal`,
`test_add_surface_does_not_change_active`.

**Pattern-match migrations:** 7 existing match arms in `splits.rs`
needed `..` added to compile (`find_terminal_mut`, `find_leaf`,
`process_pty_outputs`, `leaf_panes`, `leaf_panes_mut`,
`collect_exited_panes`, the `Debug` impl, plus the one in
`workspace_view.rs:79`). 5 pattern matches in `split_at`/`close_pane_impl`
already had `..` or only matched on `id`, so they were untouched.

### `remove_surface` clamp logic

Three cases tested:
1. `active == removed_idx` and a surface remains at the same index →
   focus stays at `min(idx, new_len-1)`. (Surface at idx+1 slides in.)
2. `active` was past the new end (e.g. `set_active_surface_index(99)`
   then remove) → clamp to `new_len - 1` via `saturating_sub`.
3. Last surface removed → vec is empty, `saturating_sub(0) = 0` keeps
   `active_surface` at 0 (still in-bounds, since empty `surfaces.get(0)`
   returns `None` — `active_surface()` correctly returns `None`).

The `active_terminal` and `active_terminal_mut` methods fall back to
the legacy `terminal: Box<Option<TerminalPane>>` slot when the
surfaces list is empty, so uninitialized leaves still expose a
terminal to `Workspace::set_terminal` and `workspace_view::render_leaf`
during the W2 transition. Future waves will remove this fallback.

### Gotchas hit

- `super::surface` is a wrong import path from inside the `tests`
  submodule of `splits.rs` (super = `splits`, not `workspace`). The
  correct path is `super::super::surface::...` or
  `crate::workspace::surface::...`. Wasted one compile cycle.
- The first test for `active_terminal` did
  `let s1_name = s1.terminal.name(); leaf.add_surface(s1);` — borrow
  checker rightly refused: `s1.terminal.name()` borrows the terminal
  for the lifetime of the comparison, but we then move `s1` into the
  leaf. Fix: extract the name AFTER moving, by indexing into
  `leaf.leaf_surfaces()[0].terminal.name()`. Two terminals spawned
  via `TerminalPane::spawn` return the same shell name (e.g. "zsh"),
  so the test asserts the name matches what the leaf reports, not
  what a separately-held terminal reports.
- `leaf_surfaces_mut` for non-leaf nodes: the spec mandates
  `&mut Vec<Surface>` (not `Option<&mut Vec<Surface>>`). Returning a
  reference to a temporary `Vec::new()` doesn't compile (lifetime
  error), and `static mut EMPTY` is deprecated. The cleanest fix is
  to `panic!` on non-leaf nodes and document the precondition in the
  doc comment. Callers gate on `is_leaf()` first.
- `active_terminal` return-type mismatch: `terminal.as_ref()` on
  `Box<Option<TerminalPane>>` returns `&Option<TerminalPane>`, not
  `Option<&TerminalPane>`. The compiler suggested
  `terminal.as_ref().as_ref()` — one `.as_ref()` for the Box, one
  for the inner Option. Same trick for the `mut` variant.
- `Display` impl for `PaneNode::Leaf` now shows `surfaces: N` and
  `active_surface: idx` so debug logs are useful before the new
  fields actually get populated.

### TDD cycle observed

1. Wrote `surface.rs` with only the 4 tests → `cargo test` →
   FAILED with `unresolved import workspace::surface` (module not
   registered in `mod.rs`).
2. Added `pub mod surface;` to `workspace/mod.rs` → tests compiled
   but FAILED with `TerminalPane` not constructable (the early
   placeholder used `unimplemented!()`-style dummy). Replaced the
   dummy with a real `TerminalPane::spawn(1, 1, 14.0)`.
3. All 4 Surface tests passed (green).
4. Wrote 5 splits::tests for the new accessors → FAILED with
   `no method named 'add_surface' found` (and 47 other E0599s for
   each missing accessor).
5. Added fields + accessors → all 15 splits tests + 50 workspace
   tests + 79 full app tests passed.

### Verification

- `cargo test -p rmux-app --bin rmux workspace::surface::tests` →
  4 passed, 0 failed
- `cargo test -p rmux-app --bin rmux workspace::splits::tests` →
  15 passed, 0 failed (10 pre-existing + 5 new)
- `cargo test -p rmux-app --bin rmux workspace::` → 50 passed, 0 failed
- `cargo test -p rmux-app --bin rmux` → 79 passed, 0 failed
- `cargo build -p rmux-app 2>&1 | grep -c "error\[E"` → `0`
- `cargo build -p rmux-app` → 1 pre-existing warning
  (`RenameTab`/`NewWindow`/`CloseWindow` never constructed, W1.2
  notepad). The new surface items are `#![allow(dead_code)]` like
  the rest of the workspace module.

### Reusable patterns

- `char_indices().nth(N).map(|(idx, _)| &s[..idx])` is the
  safe-bytes-boundary way to truncate a UTF-8 string to N chars.
  `s.chars().take(N).collect::<String>()` allocates; the
  `char_indices` version is zero-alloc and returns a borrow.
- For pattern matches on enum variants when new fields are added,
  the `..` is the minimum-disruption update. Five of the existing
  `splits.rs` matches already used `..` (because the variant had
  more fields than the match cared about), so this refactor only
  needed `..` added to 7 explicit-match arms.
- A leaf with both a legacy `Box<Option<TerminalPane>>` slot AND a
  new `Vec<Surface>` of tabs lets you migrate callers incrementally
  — old code that does `find_terminal_mut` keeps working on the
  legacy slot, new code that does `active_terminal` reads from the
  surface list with a fallback to the slot. The fallback is the
  explicit signal that migration is incomplete.
- `saturating_sub` on a fresh `0usize` returns `0`, which is the
  right clamp for an empty vec (`Vec::get(0) == None`, so any
  `active_surface` value is out-of-bounds on an empty vec).

### Follow-ups (not done here)

- **Todo 7 (surface creation in Workspace):** migrate
  `Workspace::set_terminal` to use `add_surface` instead of writing
  to the legacy `terminal` slot. The fallback in `active_terminal`
  can then be deleted and the legacy field removed.
- **Todo 8 (`WorkspaceManager` surface API):** expose
  `new_surface(workspace_id, pane_id) -> SurfaceId` and
  `select_surface(workspace_id, pane_id, surface_id)` on
  `WorkspaceManager` so the dispatcher (Todo 14) can wire
  `NewSurface` and `Ctrl+1..9` to them.
- **Future cleanup:** when the legacy `terminal` field is removed,
  `find_terminal_mut` and `find_leaf` should return
  `Option<&mut TerminalPane>` / `Option<&TerminalPane>` instead of
  `Option<&mut Option<TerminalPane>>`. `Workspace::active_terminal`
  and `workspace_view::render_leaf` will need corresponding updates.
- **Possible future test:** `test_active_terminal_falls_back_to_legacy`
  on a leaf with `terminal = Some(...)` and empty `surfaces`. The
  fallback path is currently only exercised via `Workspace::set_terminal`
  in the integration tests, not at the unit level.
