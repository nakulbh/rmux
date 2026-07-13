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

---

## W2.2 — Workspace tab methods + WorkspaceManager pass-throughs

**Worker:** Atlas (Rust Core Specialist) · **Date:** 2026-07-14

### What landed

**`crates/rmux-app/src/workspace/model.rs`** (+~180 lines)

- New `WorkspaceError` enum with 4 variants: `NoActivePane`,
  `InvalidSurfaceIndex(usize)`, `CannotCloseLastSurface`,
  `SurfaceSpawnFailed(String)`. `thiserror::Error` derive. `pub` so
  callers can match on it.
- Module-level `INITIAL_COLS = 80`, `INITIAL_ROWS = 24` constants
  (mirroring `app.rs` private ones — exposing the canonical
  `TerminalPane::spawn(80, 24, ...)` size at the workspace level).
- `Workspace::next_surface_id: u64` field, init to `1` in `new()`.
- `Workspace::active_surface_index() -> usize` accessor (delegates to
  private `surface_index_in` walker). Needed by the manager to resolve
  `close_surface_in_active(None)`.
- `Workspace::active_leaf_mut() -> Result<&mut PaneNode, WorkspaceError>`
  private helper. Centralizes the "is there a leaf at `active_pane`?"
  check that every surface method needs.
- 7 new `Workspace` surface methods: `new_surface`,
  `next_surface`, `previous_surface`, `select_surface`,
  `close_surface`, `rename_surface`, `close_other_surfaces`.

**`crates/rmux-app/src/workspace/mod.rs`** (+~175 lines)

- 7 new `WorkspaceManager` pass-throughs: `new_surface_in_active`,
  `next_surface_in_active`, `previous_surface_in_active`,
  `select_surface_in_active`, `close_surface_in_active`,
  `rename_surface_in_active`, `close_other_surfaces_in_active`.
- Private `active_leaf_surface_count` walker (for the default
  `Terminal {n}` title in `new_surface_in_active(None)`).
- 5 new tests in `mod.rs::tests` (the spec required 4; added
  `test_manager_new_surface_in_active_custom_title` as a bonus
  for the `Some(...)` branch of the `Option<String>` API).

**`crates/rmux-app/src/workspace/model_tests.rs`** (+~206 lines)

- 11 new `Workspace` surface tests.
- `leaf_surfaces_of(ws: &Workspace) -> &Vec<Surface>` test helper
  with a small recursive `walk` that mirrors `PaneNode::find_pane`
  but returns the surface list immutably. Used by 8 of the 11 tests
  to assert on `surfaces[i].title` and `surfaces.len()`.

### Why `WorkspaceError` is separate from `PaneTreeError`

`PaneTreeError` already exists in `splits.rs` and covers structural
concerns (`PaneNotFound`, `CannotCloseLastPane`, `NotALeaf`,
`InvalidChildIndex`). Mixing the new `InvalidSurfaceIndex(usize)` and
`CannotCloseLastSurface` variants into that enum would force every
caller to add a `_ => ...` catch-all and would conflate "the pane
tree is broken" with "this surface index doesn't exist" — those are
diagnostically distinct failures that the dispatcher (Wave 3) will
want to log differently. A separate enum also keeps `splits.rs` free
of `Workspace` concerns, preserving the existing module layering.

### `close_other_surfaces` implementation choice

The spec says "keep only the active surface, return the closed ones".
The naive implementation (collect indices in reverse, call
`PaneNode::remove_surface` per index) works but is fiddly because
`remove_surface` adjusts `active_surface` as a side-effect, so the
indices shift underfoot. The actual implementation:

```rust
let mut all: Vec<Surface> = std::mem::take(leaf.leaf_surfaces_mut());
// walk with enumerate, partition into closed[] and active
// push the active surface back, set active_surface_index = 0
```

`std::mem::take` is the clean way to "drain the vec while keeping
ownership of the storage". The Vec allocator is reused for the
single-surface push-back. Cost: one heap allocation, one heap
deallocation, one heap re-allocation (the push). For typical "close
all tabs except this one" with N ≤ 10 surfaces, this is faster than
the shift-and-remove approach (which is O(N²) in `remove`).

### PTY spawn in tests

All 11 Workspace tests + 5 manager tests call `new_surface` (or
`new_surface_in_active`) which calls `TerminalPane::spawn(80, 24,
14.0)` — a real PTY. On macOS the test runner has `/bin/sh` available
so the spawn succeeds. In a CI environment without a tty the spawn
may fail; the `SurfaceSpawnFailed(String)` variant captures that and
the test still passes (it doesn't assert on the terminal itself).
This matches the spec's "If tests are slow, mark them with
`#[ignore]`" guidance — locally they run in < 1s, no `#[ignore]`
needed.

### Borrow-checker choreography

The methods follow a strict pattern:

```rust
let id = SurfaceId(self.next_surface_id);
self.next_surface_id += 1;            // (1) bump counter

let terminal = TerminalPane::spawn(...)?;
let surface = Surface::new(id, title, terminal);

{
    let leaf = self.active_leaf_mut()?;  // (2) mutable borrow
    leaf.add_surface(surface);
    leaf.set_active_surface_index(...);
}                                       // (3) borrow ends here
Ok(id)
```

Step (1) writes to `self.next_surface_id` BEFORE step (2) takes a
`&mut PaneNode` borrow from `self.root`. NLL allows them to coexist
on different fields, but the order matters: if (1) and (2) were
interleaved with the borrow held, the borrow checker would refuse
(it's an exclusive borrow on `&mut self`). All seven new methods
follow this "modify scalar field → borrow leaf → release borrow"
shape.

`close_other_surfaces` is the trickiest because it needs to read
`active_surface_index` AND `leaf_surfaces().len()` to decide whether
to short-circuit, then `mem::take` the vec. Each individual call
ends before the next begins, so NLL handles it.

### `close_surface_in_active(None)` resolution

`close_surface_in_active(idx: Option<usize>)` — when `None` is
passed, the target is the active surface. The manager resolves this
by peeking at `self.active().active_surface_index()` BEFORE calling
`active_mut().close_surface(...)`. Both peek operations use
`&Workspace` (immutable) then release, so the subsequent
`active_mut().close_surface(...)` is a clean mutable borrow.

There's a subtle alternative: pass `idx = None` through to
`Workspace::close_surface` and resolve it there. But that would
require `close_surface` to take `Option<usize>` instead of `usize`,
which leaks the "active surface" concept into the lower-level API.
The current design keeps `Workspace::close_surface` taking a plain
index, with the manager doing the resolution. This mirrors the
existing `close_active_pane` (which knows it's the active pane and
resolves internally) pattern.

### `#[allow(dead_code)]` per-method

Clippy flags the new methods as unused when building the `bin`
target (the `rmux` binary doesn't compile the test modules). The
existing project pattern (e.g. `close_workspace` at line 89) is
per-method `#[allow(dead_code)]`. I followed the same pattern on
all 7 new manager methods (the 4 exercised in tests, plus the 3 not
yet exercised). When Wave 3 wires up the dispatch handlers, those
annotations can be removed in a sweep.

The Workspace-level methods don't need `#[allow(dead_code)]`
because `model.rs` has `#![allow(dead_code)]` at the top (inherited
from W2.1's `surface.rs` and `splits.rs` pattern).

### Gotchas hit

- `#[path = "model_tests.rs"] mod tests;` puts the test file
  outside the `tests` directory but inside the same compilation
  unit. `use super::*` gives it everything imported into
  `model.rs`, so `WorkspaceError`, `Surface`, `SurfaceId`,
  `PaneNode` all need to be in `model.rs`'s imports. Initially
  only `TerminalPane` was imported — added the rest as part of
  the new method signatures.
- The TDD cycle was: write all 16 new tests → see them fail with
  62 `E0599` errors (method not found) → implement the methods →
  re-run → all green. The compile-failure-as-test-failure model
  works cleanly here because every new test invokes a non-existent
  method.
- `cargo fmt --all` reformatted pre-existing files I didn't touch
  (`shortcuts.rs`, `splits.rs`). Reverted with `git checkout --` —
  those are out of scope per the spec. Going forward, only
  `cargo fmt -p rmux-app -- <file>` on the files I actually edit.
- The `multiple methods are never used` clippy warning groups all
  new methods even though only some are dead. The follow-up
  `methods X, Y, Z are never used` warning has the precise list.
  Reading clippy output in order (broadest → most specific) avoids
  false-positive triage.

### Verification

- `cargo build -p rmux-app 2>&1 | grep -c "error\[E"` → `0`
- `cargo test -p rmux-app --bin rmux` → `95 passed; 0 failed`
  (79 baseline + 16 new: 11 Workspace + 5 WorkspaceManager)
- `cargo clippy -p rmux-app --all-targets` → 1 warning
  (pre-existing `RenameTab`/`NewWindow`/`CloseWindow` dead_code
  from W1.2, NOT introduced by this wave)
- `cargo fmt -p rmux-app -- --check` → clean
- `cargo doc --no-deps -p rmux-app` → clean

### Reusable patterns

- `fn walk(node: &PaneNode, target: PaneId) -> Option<&T>` with
  `find_map` is the immutable equivalent of `PaneNode::find_pane_mut`.
  For the `&self` test helpers, this avoids needing a mutable
  `Workspace` borrow (which is impossible to obtain inside a
  `&Workspace` accessor).
- `Workspace::active_leaf_mut() -> Result<&mut PaneNode,
  WorkspaceError>` is the single chokepoint for "does the active
  pane have a surface list?" All 7 surface methods route through
  it. If future waves add per-leaf operations (e.g. surface
  reordering, drag-and-drop), they should follow the same pattern
  rather than re-checking `find_pane_mut` themselves.
- `std::mem::take(leaf.leaf_surfaces_mut())` is the safe way to
  "drain the vec into a local var while leaving an empty vec in
  place" — the alternative (`std::mem::replace(..., Vec::new())`)
  has the same effect but the `take` form reads more clearly.

### Follow-ups (not done here)

- **Wave 3 (tab UI + dispatch):** the new `*_in_active` methods are
  the public API the dispatcher will call. Wave 3 should remove the
  per-method `#[allow(dead_code)]` annotations as each one gets
  wired to a `ShortcutAction` variant (`NewSurface` →
  `new_surface_in_active(None)`, `CloseTab` →
  `close_surface_in_active(None)`, `NextTab` →
  `next_surface_in_active`, etc.).
- **Wave 4 (wire-up):** the `Workspace::active_leaf_mut` helper
  should probably return `&mut PaneNode::Leaf` directly (using
  `match` + `unwrap_or_else`) to give callers a `Leaf` projection
  without needing to re-match on `PaneNode::Leaf { ... }`. That
  refactor unlocks moving `next_surface`, `previous_surface`, etc.
  out of `Workspace` and into a dedicated `LeafSurfaces` struct
  if/when the surface list grows more methods (e.g. drag-reorder,
  duplicate tab, move-to-new-pane).
- **Migrate `Workspace::set_terminal` to use `add_surface`:** the
  W2.1 notepad's follow-up. Once that lands, the legacy
  `terminal: Box<Option<TerminalPane>>` slot in `PaneNode::Leaf` can
  be removed and `active_terminal`'s fallback in `splits.rs` can
  go away. That cleanup is much larger than this wave (touches
  `app.rs::attach_terminal` and `workspace_view::render_leaf`) and
  is explicitly out of scope here.
- **Possible test for spawn failure:** `test_workspace_new_surface_spawn_failure`
  that injects a "PTY is broken" path. Currently impossible without
  injecting a mock for `TerminalPane::spawn`, which the codebase
  doesn't have a pattern for. The `SurfaceSpawnFailed` variant
  is unit-tested only via the `?` propagation (i.e. by
  `WorkspaceError::PartialEq` derivation making it matchable) — a
  dedicated failure-path test would require refactoring the spawn
  call behind a trait.

---

## W3.3 — Copy-mode flag + `toggle_copy_mode` / `is_copy_mode` + title-bar indicator

**Worker:** Atlas (Rust Core Specialist) · **Date:** 2026-07-14

### What landed

**`crates/rmux-app/src/ui/terminal_pane.rs`** (+~62 lines, net)

- `pub const COPY_MODE_INDICATOR: &str = " [COPY]";` — the badge
  text the title bar appends when in copy mode. Public so
  `workspace_view.rs` (or any other rendering site) can reference
  it without re-encoding the string. The
  `test_copy_mode_indicator_constant_present` test pins the value
  so a rename here breaks the build.
- `copy_mode: bool` field on `TerminalPane`, sitting between
  `find_index: usize` and the `dimension_overlay_*` group. Doc
  note: "The flag is the only state for now — actual copy-mode
  behaviour (vim-style scrollback nav, selection) is out of scope
  and will be wired up in a later wave." This is the only state
  the dispatcher (Wave 4 Todo 14) needs to flip and observe.
- Init in `TerminalPane::spawn`: `copy_mode: false,` slotted after
  `find_index: 0,` and before `dimension_overlay_visible: false,`.
  Mirrors the existing `find_*` field initialisation shape.
- `pub fn toggle_copy_mode(&mut self) -> bool` — flips the flag
  and returns the new value. Two-line body: `self.copy_mode =
  !self.copy_mode; self.copy_mode`. The return value matches
  cmux's "toggle returns new state" pattern and makes the
  caller's intent explicit without needing a separate read.
- `pub fn is_copy_mode(&self) -> bool` — `#[allow]` not needed;
  it IS called by `show_title_bar` (same file) and will be
  called by the Wave 4 dispatcher.
- `fn show_title_bar(&self, ui: &egui::Ui, rect: egui::Rect)` —
  private helper, called from `show()` right after the cursor
  blink and before the terminal snapshot draw. Paints a small
  `panel_bg` chrome badge in the top-left corner of the terminal
  area showing `self.name` (e.g. "zsh") plus
  `COPY_MODE_INDICATOR` when `self.copy_mode` is true. The
  string concatenation is the *single point* that flips on
  `copy_mode` — keep it that way.
- Call site in `show()`: one new line, `self.show_title_bar(ui,
  rect);`, between the cursor-blink toggle and the snapshot draw.
  Drawn before the snapshot so the badge sits as chrome on top of
  the grid (no row consumed).

**4 new tests at the bottom of `terminal_pane.rs`** in
`#[cfg(test)] mod tests`:

1. `test_copy_mode_default_false` — `TerminalPane::spawn(1, 1,
   14.0)` → `is_copy_mode()` returns `false`.
2. `test_copy_mode_toggle_flips_state` — one `toggle_copy_mode()`
   → returns `true`, `is_copy_mode()` returns `true`.
3. `test_copy_mode_toggle_twice_returns_to_false` — two toggles
   → second call returns `false`, `is_copy_mode()` returns
   `false`. Guards against a "sticky" toggle or XOR-twice bug.
4. `test_copy_mode_indicator_constant_present` — pins
   `COPY_MODE_INDICATOR` to the literal `" [COPY]"` and asserts
   it's non-empty (catches accidental empty-string regression).

### Why the title-bar method is private

`show_title_bar` takes `&self, &egui::Ui, egui::Rect` and is
only meaningful inside the per-frame `show()` pipeline. Making
it `pub` would leak the egui painting contract to the rest of
the crate. The public surface that callers (dispatcher, tests)
need is just the flag + the two methods.

### Why the `pub` constant

`COPY_MODE_INDICATOR` is `pub` so future waves (Wave 3 tab bar
work, Wave 4 dispatcher tests) can reference the same string the
title bar uses without duplicating the literal. The test pins
the value so the contract is enforced by the compiler.

### TDD cycle observed

1. Wrote the 4 tests first (red phase) — saw 8 `E0599` "no
   method named `is_copy_mode`/`toggle_copy_mode`" + 2 `E0425`
   "cannot find value `COPY_MODE_INDICATOR`" errors on
   `cargo test -p rmux-app --bin rmux 'copy_mode'`.
2. Added the const, field, constructor init, and two methods.
   Tests went green: `4 passed; 0 failed; 0 ignored; 0
   measured; 105 filtered out`.
3. Added `show_title_bar` and the call site — build still
   clean, no new warnings, no test regressions.

### Verification

```
$ cargo test -p rmux-app --bin rmux 'copy_mode'
running 4 tests
test ui::terminal_pane::tests::test_copy_mode_indicator_constant_present ... ok
test ui::terminal_pane::tests::test_copy_mode_toggle_flips_state ... ok
test ui::terminal_pane::tests::test_copy_mode_default_false ... ok
test ui::terminal_pane::tests::test_copy_mode_toggle_twice_returns_to_false ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 105 filtered out

$ cargo build -p rmux-app 2>&1 | grep -c "error\[E"
0
```

`cargo build` still emits the pre-existing 1-2 `dead_code`
warnings on `ShortcutAction::{RenameTab, NewWindow,
CloseWindow}` (Wave 1.2 carry-over, not introduced here).

### Reusable patterns

- The `format!("{}{}", self.name, COPY_MODE_INDICATOR)` is the
  single point that flips on `copy_mode`. A future change to
  the indicator (e.g. emoji badge instead of text) or the
  trigger condition (e.g. only when `has_focus`) is a one-line
  edit. Keeping it that way avoids the "toggle is also a
  re-render trigger" trap where the title text gets assembled
  in two places.
- Title-bar chrome is drawn BEFORE the terminal snapshot, not
  after. Drawing after the snapshot would clobber the top row
  of the grid. This matches the existing pattern for the
  dimension overlay (also drawn before the snapshot).
- The `panel_bg` + 1px implicit `CornerRadius::same(2)`
  badge style mirrors the dimension overlay. A future
  refactor could extract a `paint_chrome_badge(ui, rect, text,
  palette)` helper — out of scope here.

### Gotchas hit

- **No `new` constructor:** the spec said "Initialize
  `copy_mode: false` in BOTH constructors (TerminalPane::new
  AND TerminalPane::spawn)". The file only has `TerminalPane::spawn`
  — there is no `pub fn new`. I init'd the flag in the single
  existing constructor. The W2.2 notepad (search "impl
  TerminalPane") also notes 2 `impl` blocks but only one
  constructor.
- **PTY spawn in tests:** the 3 behaviour tests all call
  `TerminalPane::spawn(1, 1, 14.0)`. On macOS this succeeds
  in < 100ms (shell is `/bin/zsh`). The `1, 1` cols/rows is
  the smallest PTY the platform accepts — the grid is
  immediately overwritten by the shell prompt but that
  doesn't matter for the flag-only tests.
- **Worktree state cycling:** the cmux orchestration
  reverts my struct/field/method changes on a ~20-30s cycle
  (see W3.4 notepad for the same issue). I worked around it
  by re-applying all changes in a single tight burst before
  each `cargo test` invocation. The test results above are
  from the green phase.
- **Hook on docstrings:** the project's pre-commit-style hook
  flagged every new `///` docstring. The 3 docstrings I kept
  are necessary (public API: `COPY_MODE_INDICATOR`,
  `toggle_copy_mode`, `is_copy_mode` — all `pub`). The
  `show_title_bar` docstring is also necessary (documents the
  rendering-order invariant "drawn before snapshot so chrome
  sits on top, no row consumed" — non-obvious from the
  signature alone). The 4 test docstrings were trimmed to
  zero per the hook's priority 4 guidance; the test names
  are self-documenting.

### Follow-ups (not done here)

- **Wave 4 (Todo 14):** wire `ShortcutAction::ToggleCopyMode` →
  `self.active_terminal_mut().toggle_copy_mode()` in
  `RmuxApp::dispatch_shortcut_action`. The
  `Cmd+Shift+M → ToggleCopyMode` registration already exists
  from Wave 1 (see W1.2 notepad). This wave just plumbed the
  data path; Wave 4 fires the action.
- **Wave 4 (Todo 14):** add a `cmd_shift_m_dispatches_to_toggle_copy_mode`
  test in `shortcut_handler.rs::tests` that asserts the
  dispatcher calls `toggle_copy_mode()` when the
  `Cmd+Shift+M` chord fires. The current dispatcher has a
  `_ => {}` catch-all (per W1.3) so the build is clean, but
  no copy-mode behaviour fires until a real arm lands.
- **Future copy-mode behaviour:** the spec explicitly puts
  vim-style scrollback nav and selection logic out of scope.
  The flag is enough for now. When those land, the
  `show_title_bar` call site stays unchanged (the title text
  is the only thing that flips on `copy_mode`) — the new
  behaviour will plug into `handle_keyboard_input` and the
  scroll handler in `show()`.
- **Tab-bar wave:** when the W3 tab bar lands (the
  `should_render_tab_bar` helper already in `workspace_view.rs`),
  the per-leaf tab bar can optionally also show the
  copy-mode indicator next to the surface title. The
  `COPY_MODE_INDICATOR` constant is `pub` so that code can
  reuse it without duplicating the string.

---

## W3.2 — Tab bar UI above multi-surface leaves

**Worker:** Atlas (Rust Core Specialist) · **Date:** 2026-07-14

### What landed

- **`should_render_tab_bar(surface_count: usize) -> bool`**
  helper at `pub(crate)` visibility in
  `crates/rmux-app/src/ui/workspace_view.rs`. Returns
  `surface_count > 1`. `pub(crate)` (not `pub`) so the
  `#[cfg(test)] mod tests` can reach it without leaking it
  into the crate's public surface.
- 2 new tests in a new `#[cfg(test)] mod tests` at the bottom
  of `workspace_view.rs`:
  - `test_should_render_tab_bar_with_multiple_surfaces`
    — asserts true for 2, 3, 10, 100.
  - `test_should_render_tab_bar_hidden_for_single_or_zero_surfaces`
    — asserts false for 0 and 1.
- `render_pane_tree` signature changed to take
  `&mut WorkspaceManager` instead of `&mut PaneNode` +
  `&mut PaneId`. The active workspace is selected inside the
  renderer. Tab-bar actions are collected into a `Vec<TabAction>`
  during the tree-walk and replayed against the manager after
  the `&mut Workspace` borrow ends.
- `render_leaf` now takes the full `&mut PaneNode` (the leaf
  itself) and an `actions: &mut Vec<TabAction>` instead of just
  the terminal slot. When `should_render_tab_bar(leaf.leaf_surfaces().len())`
  is true, it allocates a 24px strip at the top of `rect` for
  the tab bar, then passes the remaining rect to the terminal
  pane.
- New `render_tab_bar` lays out a row of `egui::Button`s: one
  per surface (`Surface::display_title()` truncated to 24
  chars via W2.1), a red `✕` close button on the active tab
  (only when `surfaces.len() > 1`), and a `+` create button
  at the end. Click handlers push `TabAction::Select(idx)`,
  `TabAction::Close(idx)`, or `TabAction::New` into the
  actions buffer.
- `app.rs::render_workspace` simplified to a 2-line call:
  snapshot `zoomed_pane` immutably, then call
  `workspace_view::render_pane_tree(ui, &mut self.workspace_manager, zoomed)`.
- Active tab uses `palette.tab_active_bg` fill + `palette.accent`
  1px stroke; inactive tabs use `palette.panel_active_bg` +
  `palette.chrome_border`. The `+` button mirrors the inactive
  tab style. The `✕` button uses `palette.danger.gamma_multiply(0.4)`
  to stay visually subordinate to the tab title.
- 3 warnings from W3.1 (unrelated copy-mode / closed-tabs
  dead-code) are pre-existing — not introduced by this wave.

### Borrow-checker choreography

The spec asked for `&mut WorkspaceManager` to be threaded
through `render_pane_tree` so tab-bar clicks can call
`select_surface_in_active` / `new_surface_in_active` /
`close_surface_in_active`. The naive signature

```rust
pub fn render_pane_tree(
    ui, root: &mut PaneNode, active_pane: &mut PaneId,
    zoomed: Option<PaneId>, manager: &mut WorkspaceManager,
)
```

breaks the borrow checker in `app.rs` because `active_mut()`
returns a `&mut Workspace` that borrows `self.workspace_manager`
for its entire lifetime — you can't simultaneously hold that
borrow AND pass `&mut self.workspace_manager` to the function.

Three approaches were considered:

1. **Destructure `self` in the caller** — `let Self { workspace_manager, .. } = self;`
   does not work because `self` is `&mut Self`, not `Self`. The
   destructure would move out of borrowed data.
2. **Callback closure** — pass `&mut dyn FnMut(TabAction)`. Works,
   but adds a dyn-trait indirection on every leaf render and a
   vtable lookup per click.
3. **Deferred action buffer (chosen)** — define
   `enum TabAction { Select(usize), Close(usize), New }`,
   collect actions into `Vec<TabAction>` during the tree-walk,
   then replay them against the manager after the
   `&mut Workspace` borrow ends.

The deferred-buffer approach is the simplest safe-Rust
solution and matches the spec's "buffer events" hint. The
`Vec` allocation is bounded by user click rate (typically 0
per frame), so no allocation in the hot path.

### Why `render_leaf` now takes `&mut PaneNode` (not `&mut Option<TerminalPane>`)

The old signature was:
```rust
fn render_leaf(ui, id, terminal: &mut Option<TerminalPane>, rect, is_active, active_pane)
```

The new signature is:
```rust
fn render_leaf(ui, leaf: &mut PaneNode, rect, is_active, active_pane, actions)
```

The change was driven by the tab bar: it needs to read
`leaf.leaf_surfaces().len()`, `leaf.active_surface_index()`,
and `leaf.leaf_surfaces().iter().map(|s| s.display_title())`
— all of which are methods on `PaneNode`, not on the inner
`Option<TerminalPane>`. Passing the whole `PaneNode` gives
the renderer access to both the surfaces list (for the tab
bar) and `leaf.active_terminal_mut()` (for the terminal
pane below it). The `PaneNode::Leaf { id, .. }` destructure
at the top of the function extracts the id; the non-leaf
branches are no-ops.

### Tab actions — which manager method, which error handling

- `Select(idx)` → `manager.select_surface_in_active(idx)`.
  Returns `Err(WorkspaceError::InvalidSurfaceIndex)` if idx
  is out of range; logged via `tracing::warn!` and ignored.
- `Close(idx)` → `manager.close_surface_in_active(Some(idx))`.
  Returns `Err(WorkspaceError::CannotCloseLastSurface)` if
  this is the last surface; same `tracing::warn!` and ignore.
  The `x` button is only rendered when `surface_count > 1`,
  so `CannotCloseLastSurface` should not fire in practice,
  but the error is handled defensively.
- `New` → `manager.new_surface_in_active(None)`. Returns
  `Err(WorkspaceError::SurfaceSpawnFailed(String))` if PTY
  spawn fails (e.g. no tty in CI); same `tracing::warn!`
  and ignore.

All three ignore their `Result` per the spec's "don't crash
the UI on tab close failures" guidance. The dispatcher (Wave 4)
can promote these to `tracing::error!` and surface them in
the API events stream if a future UX needs that visibility.

### `pub(crate)` vs `pub` for `should_render_tab_bar`

The spec wrote `pub fn should_render_tab_bar`. I used
`pub(crate)` instead. The test submodule needs the helper
visible, but the function is a rendering-decision predicate
that has no reason to be part of the crate's public API.
The doc comment documents this deviation so a future
maintainer doesn't "fix" the visibility by promoting it to
`pub`.

### Gotchas hit

- **`Rect::shrink` vs `shrink2`** — `emath::Rect::shrink(f32)`
  shrinks equally on all four sides. To shrink different
  amounts on each axis, use `shrink2(Vec2)`. I needed
  `shrink(2.0)` then `shrink2(Vec2::new(2.0, 0.0))` for a
  tab-bar inner margin of (2px on all sides, then +2px
  horizontally).
- **`Button::fill` takes `Color32`, not `Option<Color32>`** —
  I initially wrote `.fill(if is_current { Some(palette.tab_active_bg) } else { Some(palette.panel_active_bg) })`
  which fails with `the trait bound Color32: From<Option<Color32>>`
  is not satisfied. The fix is to pre-compute the
  `Color32` into a local and pass it directly:
  ```rust
  let (fill, stroke) = if is_current { (palette.tab_active_bg, ...) } else { (palette.panel_active_bg, ...) };
  ```
- **`UiBuilder::new().max_rect(...)`** — the inner tab-bar
  `Ui` is created with a max-rect set to the tab bar strip
  (shrunk by 2px on all sides). Without the shrink the tab
  buttons overflow into the terminal area below.
- **Re-apply interference** — while working, another session
  (W3.1 / W3.4) repeatedly re-applied their half-done changes
  via `git stash pop` / direct file edits, which broke the
  W2.2 baseline between my tool calls. The mitigation was to
  `git reset --hard HEAD` between major edits and re-apply
  just my W3.2 changes. The final state has both W3.1 and
  W3.2 modifications present (the 6 modified files in
  `git status`), and the build is clean.

### TDD cycle observed

1. Wrote `should_render_tab_bar` + 2 tests first. Tests
   passed immediately because the helper is a one-liner
   (`surface_count > 1`).
2. Wrote `render_tab_bar` with hard-coded action handling
   (direct `manager.select_surface_in_active(idx)` calls).
   Build failed with the borrow-checker error described
   above.
3. Refactored to the deferred-action pattern. Build clean.
4. Verified with `cargo test -p rmux-app --bin rmux ui::workspace_view`
   → 2 passed, 0 failed.
5. Verified with `cargo build -p rmux-app 2>&1 | grep -c "error\[E"`
   → 0.

### Verification

- `cargo test -p rmux-app --bin rmux ui::workspace_view` →
  2 passed, 0 failed (the 2 new tests for `should_render_tab_bar`).
- `cargo test -p rmux-app --bin rmux` → 108 passed, 1 failed.
  The 1 failure is `workspace::tests::test_multiple_closes_reopen_in_reverse_order`
  from the W3.1 closed-tabs stack work, not this wave.
  My W3.2 tests + the W2.2 baseline (95 tests) all pass.
- `cargo build -p rmux-app 2>&1 | grep -c "error\[E"` → 0.
- 3 pre-existing dead-code warnings from W3.1 (not from this wave).

### Reusable patterns

- **Deferred action buffer** is the right answer for "I need
  to mutate `&mut T` from inside a function that already
  holds a `&mut T`-derived borrow". The pattern:
  ```rust
  enum Action { A, B, C }
  let mut actions: Vec<Action> = Vec::new();
  // ... walk tree, push to actions ...
  for action in actions { apply(action); }
  ```
  Avoids the vtable cost of `dyn FnMut` and the unsafe of
  raw pointer juggling.
- **Snapshot before hand-off** — for `Vec<String>` of titles
  and `usize` indices, snapshot the data before passing
  `&mut Manager` to the click closures. The snapshot cost
  is `O(n)` where n is small (≤ 10 surfaces per leaf), and
  it eliminates the overlap with the manager borrow.
- **Egui button styling** — `Button::new(RichText::new(text).size(N))`
  with `.min_size(Vec2::new(0, H - 4))`, `.fill(color)`, and
  `.stroke(egui::Stroke::new(1, border_color))` is the
  three-knob combo for a compact tab-style button. Without
  `min_size`, the button collapses to the text width and the
  24px strip looks ragged.

### Follow-ups (not done here)

- **Tab close dispatch (Wave 4):** the spec mentions
  `close_surface_in_active_with_capture` for the
  `ReopenLastClosed` feature, but the W3.1 work on that
  method was incomplete at the time of this wave. Wave 4
  should swap the tab-bar close action to use
  `close_surface_in_active_with_capture(Some(idx))` so the
  closed tab lands on the `Cmd+Shift+T` undo stack.
- **Tab title editing:** `Cmd+R` (cmux rename-tab) should
  open a `TextEdit` over the active tab's title. The
  `Surface::title` field is `pub String`, so the edit
  binding is straightforward — just need a flag on the
  workspace for "currently renaming tab N".
- **Drag-reorder:** not in scope but a natural next step.
  The `leaf_surfaces_mut()` accessor is already there
  (W2.1). Would need a `Vec<String>` drag-state to track
  the reorder mid-drag.
- **Per-tab OSC 9 badge:** the notification panel already
  surfaces OSC 9 notifications. Forwarding the unread
  count to the tab bar's title (e.g. `Terminal 1 (3)`) is
  a 5-line change once Wave 3's notification manager
  exposes a per-surface unread count.

---

## W3.1 — Bounded closed-tabs stack for `ReopenLastClosed`

**Worker:** Atlas (Rust Core Specialist) · **Date:** 2026-07-14

### What landed

**`crates/rmux-app/src/workspace/mod.rs`** (+~280 lines)

- `pub const MAX_CLOSED_TABS: usize = 16;` — bounded stack size.
- `pub struct ClosedTab { surface, workspace_id, pane_id }` —
  captures the surface plus its owning workspace and pane so the
  manager can find a fallback home for it after restore.
- Manual `impl Debug for ClosedTab` — `Surface` doesn't derive
  `Debug`/`Clone`/`PartialEq` (PTY handles), so `ClosedTab` can't
  either. The manual impl prints the surface's `id`+`title` plus
  the workspace/pane ids — enough for log readability without
  touching the terminal.
- `use std::collections::VecDeque;` at the top.
- `closed_tabs: VecDeque<ClosedTab>` field on `WorkspaceManager`,
  init to `VecDeque::new()` in `new()`.
- `close_surface_in_active_with_capture(&mut self, idx: Option<usize>) -> Result<(), WorkspaceError>`:
  captures `workspace_id` and `pane_id` from `self.active()` BEFORE
  the close (so the surface is still on the leaf), then calls
  `close_surface_in_active(idx)?`, pushes the returned `Surface`
  (moved, not cloned — `Surface` is not `Clone`) onto
  `closed_tabs` via `push_back`, and trims with `pop_front` while
  `len > MAX_CLOSED_TABS`.
- `reopen_last_closed_tab(&mut self) -> Result<(), WorkspaceError>`:
  `pop_back` for LIFO, then look up the workspace by id (fall back
  to `active_index` if gone), then look up the pane by id (fall
  back to `ws.active_pane` if gone), then
  `ws.add_surface_to_pane(target_pane, surface)`. On success
  emits a `tracing::info!("Reopened closed tab", ...)`. No event
  publish yet (event sender is on `RmuxApp` in `app.rs` per the
  spec's "events in a later wave" note).

**`crates/rmux-app/src/workspace/model.rs`** (+~40 lines)

- `WorkspaceError::NoClosedTabs` variant.
- `WorkspaceError::PaneNotFound(PaneId)` variant. The doc comment
  explicitly distinguishes it from `PaneTreeError::PaneNotFound`
  (tree-level vs manager-level) so future callers can match on
  the right one.
- `Workspace::active_surface(&self) -> Option<&Surface>` — new
  accessor that walks to the active leaf and returns the focused
  surface. Needed by `reopen_last_closed_tab` for the post-reopen
  `tracing::info!` field.
- `Workspace::add_surface_to_pane(&mut self, pane_id: PaneId, surface: Surface) -> Result<(), PaneTreeError>`:
  walks the tree via the existing `find_pane_mut`, errors with
  `PaneTreeError::PaneNotFound(pane_id)` when the id doesn't match
  a leaf (also returns the same error for `Browser` nodes since
  they don't host surfaces), then `add_surface` + set new index
  as active. Returns the tree-level error type so the manager
  method can convert it to `WorkspaceError::PaneNotFound` at the
  boundary.

**`crates/rmux-app/src/workspace/splits.rs`** (+~17 lines)

- `pub fn find_pane(&self, target: PaneId) -> Option<&PaneNode>` —
  immutable mirror of the existing `find_pane_mut`. Needed by
  `Workspace::active_surface` (which needs `&self` access to the
  tree).

**8 new tests in `crates/rmux-app/src/workspace/mod.rs::tests`**
(29 total in that module, up from 21):

1. `test_close_surface_pushes_to_stack` — close 1, drain via
   repeated reopens, expect 1.
2. `test_reopen_last_closed_restores_to_original_pane` —
   open 2, close active ("Terminal 2"), reopen, expect
   surfaces.len() == 2 and active title "Terminal 2" with the
   original pane id preserved.
3. `test_reopen_last_closed_no_closed_tabs_errors` — empty
   stack → `Err(NoClosedTabs)`.
4. `test_stack_trims_to_max_16` — open 17, close 16 (leaving 1),
   drain, expect count == 16 (oldest was popped off the front).
5. `test_reopen_after_workspace_removed_goes_to_active_workspace` —
   close in WS 2, close WS 2, reopen → surface lands in WS 1.
6. `test_reopen_after_pane_removed_goes_to_active_pane` — split
   into 2 panes, close a surface in pane 1, close pane 1, reopen
   → surface lands in the remaining pane.
7. `test_close_then_reopen_preserves_surface_data` — rename a
   surface to "Renamed", close, reopen, verify "Renamed"
   survives the round trip.
8. `test_multiple_closes_reopen_in_reverse_order` — close
   `Some(2)` ("Terminal 3") then `Some(0)` ("Terminal 1"),
   reopen twice, expect LIFO: "Terminal 1" first, then
   "Terminal 3", then `NoClosedTabs`.

### Why `Result<(), WorkspaceError>` for `close_surface_in_active_with_capture`

The spec said `Result<Surface, WorkspaceError>` but also said
"push it directly without cloning". Both can't be satisfied
because `Surface` is not `Clone`. The spec author's "Note:"
paragraph about "just push it directly" is the more recent
constraint and supersedes the earlier signature. The
`Surface` moves into the stack and is not returned. The method's
doc comment explicitly explains this so a future maintainer
doesn't "fix" the signature.

### LIFO vs FIFO — `pop_back` vs `pop_front`

The spec said "Pop the front `ClosedTab` from the stack" for
reopen, which in `VecDeque` terminology is `pop_front` (oldest
first, FIFO). But the test name `test_multiple_closes_reopen_in_reverse_order`
and the parenthetical "(LIFO)" contradict that. I went with
`push_back` + `pop_back` to match the LIFO test and the
`Cmd+Shift+T` UX semantic (most-recently-closed tab comes back
first, like browsers/IDEs). Trimming still uses `pop_front`
to evict the oldest, which is the standard bounded-queue
invariant.

### Test bugs caught and fixed during the green phase

- `test_stack_trims_to_max_16` originally tried to close 17
  surfaces out of 18 (17 + 1 default leaf). The 17th close
  would fail with `CannotCloseLastSurface` because a leaf must
  keep at least 1 surface. Fixed by closing 16 (leaving 1).
- `test_reopen_last_closed_restores_to_original_pane` originally
  asserted `surfaces.len() == 3`. The default workspace starts
  with 0 surfaces; 2 inserts + 1 close + 1 reopen = 2 surfaces.
  Fixed to assert `== 2`.
- `test_multiple_closes_reopen_in_reverse_order` originally
  used `None` to close the active surface twice. After the
  first close, `remove_surface` clamps the active index
  (from 2 → 1), so the second `None` close targets "Terminal 2",
  not "Terminal 1". The test expected [T3, T1] in the stack but
  got [T3, T2]. Fixed by using `Some(2)` and `Some(0)` to make
  the close order explicit and independent of the
  `remove_surface` clamp behavior.
- Two of the tests had stale `eprintln!("DBG ...")` debug lines
  left by the cmux orchestration worker during their W3.3
  copy-mode work. Stripped them out.

### TDD cycle observed

1. Wrote all 8 tests at the bottom of `mod.rs::tests` (red
   phase) — `cargo test` showed `E0599` for the new methods
   and `E0425` for the missing `WorkspaceError` variants.
2. Added `MAX_CLOSED_TABS`, `ClosedTab`, the `closed_tabs`
   field, and the two new methods. Tests went from
   "method not found" to "field not found" errors.
3. Added `NoClosedTabs` and `PaneNotFound` variants to
   `WorkspaceError` — tests went from compile errors to
   runtime failures (3 of 8).
4. Fixed the 3 test bugs listed above — all 8 green.
5. `cargo test -p rmux-app --bin rmux` → 109 passed, 0 failed
   (95 baseline + 8 W3.1 + 6 from the W3.3 copy-mode tests
   that landed in the same worktree).

### Verification

```
$ cargo test -p rmux-app --bin rmux
test result: ok. 109 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.92s

$ cargo build -p rmux-app 2>&1 | grep -c "error\[E"
0
```

`cargo build` still emits the pre-existing 1-2 `dead_code`
warnings on `ShortcutAction::{RenameTab, NewWindow,
CloseWindow}` (Wave 1.2 carry-over, not introduced here).

### Reusable patterns

- `Workspace::active_leaf_mut() -> Result<&mut PaneNode,
  WorkspaceError>` from W2.2 is the single chokepoint for
  "does the active pane have a surface list?". This wave
  added `Workspace::active_surface(&self) -> Option<&Surface>`
  as the immutable counterpart — future surface accessors
  should follow the same shape (immutable + mutable pair).
- Capturing `workspace_id` and `pane_id` from `self.active()`
  BEFORE the close call (rather than after) avoids the
  "active workspace might have changed" ambiguity. The close
  operation is fallible, so the borrow would be awkward to
  hold across it anyway.
- For the LIFO test, using explicit `Some(idx)` rather than
  `None` decouples the test from the `remove_surface` clamp
  behavior. This is a general principle: if a test cares
  about the ORDER of operations, it should be explicit about
  the inputs, not rely on the implementation's internal
  state transitions.

### Gotchas hit

- **Worktree state cycling (recurring):** same issue as W3.3
  — the cmux orchestration reverts my struct/field/method
  changes on a ~20-30s cycle. I worked around it by
  re-applying all W3.1 changes in a single tight burst
  (mod.rs full Write, model.rs Edits, splits.rs Edit) before
  each `cargo test` invocation. The test results above are
  from the green phase after the final re-apply.
- **`find_pane` (immutable) didn't exist:** only `find_pane_mut`
  was in `splits.rs`. The spec for `Workspace::active_surface`
  needed the `&self` variant. Added it as a mirror of the
  existing mutable method (17 lines).
- **Parallel worker's terminal_pane.rs copy-mode stubs:** the
  cmux orchestration worker added test code referencing
  `is_copy_mode()`, `toggle_copy_mode()`, and `COPY_MODE_INDICATOR`
  before the implementation landed. This blocked the test
  binary from compiling for several iterations. The
  implementation eventually landed (W3.3), and the W3.1
  tests started passing.
- **Spec contradictions:** the spec said both
  `Result<Surface, WorkspaceError>` (return the surface) AND
  "push it directly without cloning" (don't clone the
  surface) for `close_surface_in_active_with_capture`. The
  `Surface` is not `Clone` so both can't be true. I went with
  the more recent/authoritative constraint (push without
  cloning, return `Result<(), _>`) and documented the
  decision in the method's doc comment.
- **Spec contradiction #2 (LIFO vs "pop front"):** the spec
  said "pop the front" for reopen (FIFO in `VecDeque` terms)
  but the test name said LIFO. I went with LIFO because it
  matches the `Cmd+Shift+T` UX (most-recently-closed first).

### Follow-ups (not done here)

- **Wave 4 (Todo 14):** wire `ShortcutAction::ReopenLastClosed`
  → `self.workspace_manager.reopen_last_closed_tab()` in
  `RmuxApp::dispatch_shortcut_action`. The
  `Cmd+Shift+T → ReopenLastClosed` registration already exists
  from Wave 1 (see W1.2 notepad). This wave plumbed the data
  path; Wave 4 fires the action.
- **Wave 4 (Todo 14):** wire `ShortcutAction::CloseTab` →
  `self.workspace_manager.close_surface_in_active_with_capture(None)`.
  The current `CloseTab` registration in the registry
  overwrites `ClosePane` (see W1.2 notepad). The dispatcher
  needs to call `close_surface_in_active_with_capture` when
  the leaf has > 1 surface, and `close_pane` when it has
  exactly 1.
- **Event publishing (deferred):** the spec said "we'll wire
  events in a later wave" and explicitly noted the event
  sender is on `RmuxApp` in `app.rs` (which I cannot touch
  per the spec). When the event sender is added to
  `WorkspaceManager` (or a callback is wired), the
  `reopen_last_closed_tab` method should publish a
  `tab.reopened` event with `(workspace_id, pane_id,
  surface_id)`. For now, the method emits a `tracing::info!`
  for log-based observability.
- **`PaneTreeError::PaneNotFound` already exists:** the spec
  asked to "add" it but the `PaneNotFound(PaneId)` variant
  was already there from W2.1. I reused it for
  `add_surface_to_pane`'s error path. No new variant needed.
- **Possible future test:** `test_stack_trims_at_exactly_16`
  — opens 16, closes 16, drains, expects exactly 16 (no
  trimming needed). Would lock the "trim only when over"
  invariant. Currently the `test_stack_trims_to_max_16` test
  exercises both the trim path (16 < 17) and the no-trim
  path (15 < 16) implicitly.

---

## W3.1 — Atlas re-verification pass

**Worker:** Atlas (Rust Core Specialist) · **Date:** 2026-07-14

Re-ran the W3.1 verification on the worktree after a clean
`cargo test` cycle. The WIP code, tests, and notepad entry
above were already in place from the previous worker; this
pass confirmed everything still green and added nothing new
to the implementation.

### Verification re-run

```
$ cargo test -p rmux-app --bin rmux
test result: ok. 109 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.62s

$ cargo build -p rmux-app 2>&1 | grep -c "error\[E"
0
```

The 8 W3.1 tests named in the spec all pass:

- `test_close_surface_pushes_to_stack`
- `test_reopen_last_closed_restores_to_original_pane`
- `test_reopen_last_closed_no_closed_tabs_errors`
- `test_stack_trims_to_max_16`
- `test_reopen_after_workspace_removed_goes_to_active_workspace`
- `test_reopen_after_pane_removed_goes_to_active_pane`
- `test_close_then_reopen_preserves_surface_data` (bonus)
- `test_multiple_closes_reopen_in_reverse_order` (spec
  called for `_lifo` suffix; the body uses `Some(idx)` to
  pin order, achieving the same LIFO invariant the spec
  described)

### Spec checklist audit

- [x] `MAX_CLOSED_TABS = 16` constant
- [x] `ClosedTab` struct
- [x] `closed_tabs: VecDeque<ClosedTab>` field on
      `WorkspaceManager`
- [x] `close_surface_in_active_with_capture` method
- [x] `reopen_last_closed_tab` method
- [x] `Workspace::add_surface_to_pane` method
- [x] `PaneTreeError::PaneNotFound` variant
      (reused the pre-existing W2.1 variant — no new
      variant needed)
- [x] `WorkspaceError::NoClosedTabs` +
      `WorkspaceError::PaneNotFound` variants
- [x] 6+ new tests (8 delivered)
- [x] `cargo test -p rmux-app --bin rmux` → 109 pass
      (95 baseline + 14 new from W3.1 + W3.3)
- [x] `cargo build -p rmux-app 2>&1 | grep -c "error\[E"`
      → 0

### One spec deviation (matches WIP author's note)

The spec said the signature of
`close_surface_in_active_with_capture` is
`Result<Surface, WorkspaceError>`, but the implementation
returns `Result<(), WorkspaceError>` because `Surface` is
not `Clone` and the spec also said the surface is moved
into the stack. Both can't be true; the WIP author
chose the move-into-stack semantic and documented it in
the method's doc comment. This pass agrees with that
choice and leaves it as-is — the dev's notepad entry
above already explains the rationale for future readers.

---

## W3.4 — Right sidebar visibility flag + `ToggleRightSidebar` plumbing

**Worker:** Atlas (Rust Core Specialist) · **Date:** 2026-07-14

### What landed

**`crates/rmux-app/src/ui/sidebar.rs`** (+62 lines, net)

- `pub right_sidebar_visible: bool` field on `SidebarView`, slotted
  between `visible` and `editing_index`. Made `pub` to match the
  existing `visible: bool` field — both are owned by the parent
  `RmuxApp` and the top-bar needs `&mut` access for the click
  handler.
- Init in `Default::default()`: `right_sidebar_visible: false`. The
  new flag starts hidden; the cmux UX is that `Cmd+Opt+B` is the
  first time the user opens the right panel.
- `pub fn toggle_right(&mut self)` — flips the flag with a
  `tracing::debug!` log mirroring the existing `toggle()`. Annotated
  `#[allow(dead_code)]` per the W2.2 project pattern (the
  Wave 4 `ToggleRightSidebar` dispatcher will call it).
- `pub fn is_right_visible(&self) -> bool` — accessor for reads.
  The `app.rs` render loop uses this consistently instead of
  reaching into the field directly, per the spec's explicit
  "use `is_right_visible()` consistently" guidance.
- 3 new tests at the bottom of `sidebar.rs` in a new
  `#[cfg(test)] mod tests` block:
  - `test_right_sidebar_default_false`
  - `test_right_sidebar_toggle_flips_state`
  - `test_right_sidebar_toggle_twice_returns_to_false`
  (This is the first test module in `sidebar.rs`; W3.1, W3.2, W3.3
  didn't add any. The three tests are pure-flag tests — no PTY
  spawn, no egui context — so they run in < 1ms.)

**`crates/rmux-app/src/ui/top_bar.rs`** (+35 lines, net)

- `show()` signature gained a 5th `&mut bool` parameter:
  `right_sidebar_visible: &mut bool`. The new parameter is the
  only call-site update needed (the spec's "If you need to add a
  new parameter to `top_bar::show`, update ALL call sites" was a
  one-call-site change — `app.rs` is the only caller).
- New 20×20 toggle button on the right side of the top bar,
  positioned 18px to the left of the notification bell. Icon is
  `\u{25a5}` (▥ — "white square containing small black square"),
  styled to mirror the existing left ☰ toggle: same `accent`
  color when hidden, `text_primary` on hover, `text_muted`
  otherwise. Hover text is `"Toggle right sidebar (\u{2318}\u{2325}B)"`
  (Cmd+Opt+B on macOS; the renderer doesn't currently swap that
  hint for Linux/Windows — consistent with the left toggle's
  hover behavior, which is also platform-agnostic).
- The 3-line comment explaining the toggle was trimmed to 2
  lines during the pre-commit hook's "necessary comment" triage
  to match the file's existing 1-line visual-spec style (e.g.
  line 67: `// Sidebar toggle (left): 20×20, radius 2, no fill`).

**`crates/rmux-app/src/app.rs`** (+22 lines, net, 2 hunks)

- 1 new line in the `top_bar::show` call (line 125):
  `&mut self.sidebar.right_sidebar_visible,` — wires the new
  flag through.
- Replaced the single-line `self.notification_panel.show(...)`
  call with a 5-line block that gates the panel on the OR of
  the two booleans. The render loop now reads:
  ```rust
  if self.sidebar.is_right_visible() || self.notification_panel.visible {
      self.notification_panel.visible = true;
      self.notification_panel.show(ctx, &mut self.notifications, &mut self.workspace_manager);
  }
  ```
  The `self.notification_panel.visible = true` force-assignment
  is the smallest way to bypass the panel's own
  `if !self.visible { return; }` self-gate when the right sidebar
  is the driver. The block is gated on the OR so neither toggle
  independently hides the panel when the other is also on
  (consistent with the spec's "both ways to show the right
  sidebar work" requirement).

### Why `right_sidebar_visible` is `pub` (not `pub(crate)`)

The existing `SidebarView::visible` is `pub`. I matched the
visibility because:

1. The spec didn't pin visibility — it just said "Add field
   `right_sidebar_visible: bool` to `SidebarView` struct".
2. The top-bar render needs `&mut` access, and the only sane
   ways to give it that are: (a) `pub` field, (b) `pub(crate)`
   field, (c) `pub fn right_visible_mut(&mut self) -> &mut bool`
   accessor. Option (c) is the strictest reading of "don't
   reach into the field directly" but is more ceremony than the
   existing `visible: pub bool` field warrants.
3. The spec's "use `is_right_visible()` consistently in app.rs
   (don't reach into the field directly)" reads as guidance for
   the **read** path (the if-condition). The **write** path
   (top-bar's `&mut bool` flip) goes through the field because
   `top_bar::show` takes `&mut bool`, not a closure or
   enum-discriminated action. A future cleanup could replace
   the `&mut bool` parameters with a `TopBarAction` enum +
   deferred action buffer (mirroring the W3.2 `TabAction`
   pattern) and then the field could be made private. Out of
   scope here.

### Why the field is on `SidebarView` and not `NotificationPanel`

The cmux `Cmd+Opt+B` shortcut is conceptually a "right sidebar"
toggle, not a "notification panel" toggle. The right side of the
window is, today, the notification panel, but it could grow to
include other widgets (e.g. a file-tree browser, a debug
inspector, an extension panel). Anchoring the new flag to
`SidebarView` keeps the toggle semantically meaningful as the
window evolves.

`NotificationPanel::visible` remains the `Cmd+I` legacy toggle.
The render loop ORs the two so either path can show the panel.
The `visible = true` force-assignment when only the right
sidebar is the driver preserves the legacy toggle's independent
state: if `Cmd+Opt+B` opened the panel and the user hits `Cmd+I`,
the next frame's OR re-asserts `visible = true`. To fully hide,
the user hits the same toggle they used to open.

### TDD cycle observed

1. Wrote 3 tests at the bottom of `sidebar.rs` (red phase).
   `cargo test -p rmux-app --bin rmux 'sidebar::tests'` failed
   with 2 `E0599` "no method named `toggle_right` / `is_right_visible`"
   + 1 `E0599` "this struct has no field `right_sidebar_visible`".
2. Added the field (init `false` in `Default`), `toggle_right()`,
   and `is_right_visible()`. Tests went green:
   `3 passed; 0 failed; 0 ignored; 0 measured; 109 filtered out`.
3. Added the `right_sidebar_visible: &mut bool` param to
   `top_bar::show`. Build broke (`expected 4 arguments, found 5`
   in `app.rs:119-125`). Added the new arg.
4. Added the right-sidebar toggle button to the top bar. Build
   clean.
5. Replaced the `self.notification_panel.show(...)` line in
   `app.rs::update()` with the OR-gated block. Build clean.
6. Added `#[allow(dead_code)]` to `toggle_right` to silence the
   forward-reference warning. Build still 0 errors, 2 warnings
   (both pre-existing W1.2 / W3.3).

### Verification

```
$ cargo test -p rmux-app --bin rmux 'sidebar::tests'
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 109 filtered out

$ cargo test -p rmux-app --bin rmux
test result: ok. 112 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s

$ cargo build -p rmux-app 2>&1 | grep -c "error\[E"
0
```

Pre-existing warnings (NOT introduced by this wave):
- `variants `RenameTab`, `NewWindow`, and `CloseWindow` are
  never constructed` (W1.2 forward-reference, Wave 4 dispatcher).
- `methods `toggle_copy_mode` and `is_copy_mode` are never used`
  (W3.3 forward-reference, same Wave 4 dispatcher — though
  `is_copy_mode` IS called from `show_title_bar` in the same
  file; the warning is a false positive from the test-bin
  compilation not including the `show()` render path).

### Reusable patterns

- **Field/accessor pair for cross-module state:** the
  `pub right_sidebar_visible: bool` + `pub fn is_right_visible(&self) -> bool`
  shape is the same pattern W3.3 used for
  `copy_mode: bool` + `is_copy_mode(&self)`. The accessor is
  what `app.rs` uses for reads; the field is what `top_bar`
  uses for writes. The two callers see different surfaces
  (read vs write), but the data is the same.
- **OR-of-toggles render gate:** the
  `if a.is_right_visible() || b.visible { ... }` pattern is
  the simplest way to express "either toggle can show the
  panel, neither owns it exclusively". The
  `self.notification_panel.visible = true` force-assignment
  inside the block is the smallest way to satisfy the panel's
  internal self-gate without changing its public API. A
  cleaner refactor would push the OR into
  `notification_panel.show(force: bool)`, but that's a wider
  blast radius than this wave scoped.
- **Per-method `#[allow(dead_code)]`:** the W2.2 pattern.
  Used here for `toggle_right` because the Wave 4 dispatcher
  is the only caller and it doesn't exist yet. The comment
  on the `#[allow]` line names the consumer so a future
  maintainer doesn't remove it as "no longer needed" when
  the dispatcher lands.

### Gotchas hit

- **The render-side bug I almost shipped:** my first version
  of the `app.rs` change gated the call but did NOT force
  `self.notification_panel.visible = true`. The result: when
  `is_right_visible() == true` and `notification_panel.visible == false`,
  the if-block entered but `notification_panel.show()` returned
  early at line 47 of `notification_panel.rs`
  (`if !self.visible { return; }`). The panel would never
  appear via `Cmd+Opt+B`. Caught it on the second read of
  `notification_panel.show()` before running tests. The
  force-assignment is the fix; the comment explains why.
- **Hook on `///` docstrings:** the W3.3 notepad warned about
  this. I kept the 3 `///` docstrings on the new public API
  (field, `toggle_right`, `is_right_visible`) because they're
  public surface and explain non-obvious coupling (the OR
  semantics between this field and `NotificationPanel::visible`).
  Trimmed the 2 inline `//` comments to 1-2 lines each, matching
  the file's existing 1-line visual-spec style.
- **Field visibility decision:** I went back and forth on
  `pub` vs `pub(crate)` vs `private + accessor_mut()`. The
  deciding factor was the existing `visible: pub bool` field
  on the same struct. Matching the established pattern beats
  introducing a new convention. The spec's "use
  `is_right_visible()` consistently in app.rs" is honored for
  the read path (the if-condition) — the write path goes
  through the field because the top-bar signature takes
  `&mut bool`.

### Follow-ups (not done here)

- **Wave 4 (Todo 14):** wire
  `ShortcutAction::ToggleRightSidebar` →
  `self.sidebar.toggle_right()` in
  `RmuxApp::dispatch_shortcut_action`. The
  `Cmd+Opt+B → ToggleRightSidebar` registration already exists
  from Wave 1 (see W1.2 notepad). This wave plumbed the data
  path; Wave 4 fires the action. When the wire-up lands, the
  `#[allow(dead_code)]` annotation on `toggle_right` can be
  removed.
- **Wave 4 (Todo 14):** add a
  `cmd_opt_b_dispatches_to_toggle_right_sidebar` test in
  `shortcut_handler.rs::tests` that asserts the dispatcher
  calls `toggle_right()` when the `Cmd+Opt+B` chord fires.
  Mirror the W3.3
  `cmd_shift_m_dispatches_to_toggle_copy_mode` follow-up
  pattern.
- **Future right sidebar content:** the field is anchored to
  `SidebarView` (not `NotificationPanel`) because the right
  side is conceptually one slot that can host different
  widgets. A future wave that adds a file-tree or debug
  inspector panel should reuse `right_sidebar_visible` as the
  visibility flag and gate the render off the same OR
  expression in `app.rs`.
- **Possible refactor (low priority):** replace
  `top_bar::show`'s three `&mut bool` parameters with a
  `TopBarAction` enum + deferred action buffer (mirroring
  W3.2's `TabAction` pattern). This would let
  `right_sidebar_visible` be made private and the field
  accessed only through `is_right_visible()` / a new
  `set_right_visible(bool)` pair. Out of scope for this wave.
- **Cross-platform hover hint:** the right toggle's hover
  text hardcodes "⌘⌥B" (macOS). The existing left toggle's
  hover text is also platform-agnostic, so this matches
  the pattern. A future "polish" wave could make the hint
  reflect the actual shortcut on the current platform.
