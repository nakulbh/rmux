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
