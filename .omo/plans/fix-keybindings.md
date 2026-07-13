## TL;DR (For humans)
**What you'll get:** Every cmux shortcut from the reference table wired up and tested. New surface (tab) model so panes can hold multiple terminal tabs, plus the registry/dispatch bugs that break keybindings fixed.

**Why this approach:** TDD — failing test first, then minimal code to pass. Registry + dispatch are pure-data Rust (testable without egui), so we test those exhaustively. Tab model is the new data structure; the UI for the in-pane tab bar piggybacks on the existing pane layout.

**What it will NOT do:** Multi-window (separate top-level windows), browser splits (Phase 4 is stubbed), session restore of tabs (Phase 5), cmux AppleScript/iOS companion.

**Effort:** Large — 25+ todos across registry, dispatch, model, UI, tests.
**Risk:** Medium — tab model changes how each leaf node stores panes; regressions in the existing split tree are possible.
**Decisions to sanity-check:** (1) In-leaf tab list lives behind `Arc<Mutex<Vec<TerminalPane>>>` or `RefCell` — picking `RefCell` for single-thread GUI thread, matches the existing pattern in `splits.rs`. (2) Closed-tabs stack is bounded to 16 entries (memory guard). (3) `Cmd+W` keeps current "close pane" behavior; new "close tab" maps to closing the active tab when the pane has 2+ tabs, otherwise falls through to close pane.

Your next move: approve (or request changes) and I begin Wave 1.

---
> TL;DR (machine): Large/Medium. 25+ TDD todos. Adds full tab model, fixes 3 known registry/dispatch bugs, registers all 28 cmux shortcuts, comprehensive unit tests. Multi-window + browser splits + session restore explicitly out of scope.

## Scope
### Must have
- TDD: failing test FIRST for every shortcut action
- Register ALL cmux shortcuts from the reference (28 actions: 10 surface, 6 split, 8 focus, 4 workspace)
- Fix `shortcut_handler.rs` dual-pass event leak: `Escape` and `Enter` registered with `Modifiers::NONE` must not steal from text-focused widgets
- Fix focus direction modifier inconsistency: `FocusLeft/Right/Up/Down` all use `⌥⌘Arrow` (matches cmux)
- New `Surface` (tab) model: each `PaneNode::Leaf` holds a `Vec<TerminalPane>` with an active index
- Tab bar UI within each pane (small header showing tab title + close button when ≥2 tabs)
- Tab operations: new surface, next/prev surface, select surface 1-9, close tab, rename tab, close other tabs, reopen last closed
- Reopen last closed: bounded closed-tabs stack (max 16) per workspace
- Toggle copy mode: dedicated flag on `TerminalPane`; ⌘⇧M toggles
- Toggle right sidebar: separate visibility flag from left sidebar
- Equalize splits: add `Ctrl+Cmd+=` as alias for `Cmd+Shift+=` (cmux uses `Ctrl+Cmd+=`)
- Workspace prev/next via `Ctrl+Cmd+[ / Ctrl+Cmd+]`
- 1 unit test per shortcut action (registry lookup) + 1 dispatch test (handler called) per action
- 1 test per new `WorkspaceManager` method (tab/surface operations)

### Must NOT have (guardrails, anti-slop, scope boundaries)
- NO multi-window support (separate top-level OS windows) — stub `NewWindow`/`CloseWindow` with `tracing::warn!` "not yet implemented"
- NO browser split shortcuts (`⌥⌘D`, `⌥⌘⇧D`) — Phase 4 not complete; register as `tracing::warn!` stubs
- NO new dependencies — all changes use existing crates (`egui`, `alacritty_terminal`, `portable-pty`, `tokio`)
- NO session restore of tabs (Phase 5)
- NO changes to `Cargo.toml` dependencies or versions
- NO file over 350 lines — split `shortcut_handler.rs` (298→2 files) and `workspace/mod.rs` (559→3 files) if needed
- NO unsafe code
- NO breaking changes to existing socket API methods (the API works; don't touch `api_dispatch.rs`)
- NO changes to AGENTS.md project rules

## Verification strategy
> Zero human intervention - all verification is agent-executed.
- **Test decision: TDD** — every todo writes a failing test first, then implementation
- **Framework:** `cargo test --workspace` (built-in `#[cfg(test)]` + `#[test]`)
- **Lint:** `cargo clippy --workspace --all-targets -- -D warnings` must pass
- **Format:** `cargo fmt --all -- --check` must pass
- **No unsafe:** `grep -r "unsafe" crates/ --include="*.rs" | grep -v "forbid(unsafe_code)"` must return empty
- **Evidence:** each todo commits a passing test log to `.omo/start-work/ledger.jsonl`

### Per-shortcut test matrix (mandatory)
For every registered shortcut, two tests:
1. **Registry test** (`shortcuts.rs`): `lookup(mods, key) == Some(Action::Variant)` using canonical modifier (`Modifiers::COMMAND` on macOS, `Modifiers::CTRL` on others — assert `cfg!` accordingly)
2. **Dispatch test** (`shortcut_handler.rs` or per-action handler): mock `WorkspaceManager` and assert the correct method called, OR pure-logic test on a new helper function (e.g. `tab_index_for_shortcut(1) == 0`)

### Per-method test matrix (mandatory)
For every new `WorkspaceManager` or `Workspace` method:
- Happy path: state changes as expected
- Edge case: empty workspace, single tab, no panes
- Error path: closed-last-tab, reopen stack empty, etc.

## Execution strategy
### Parallel execution waves
> Target 5-8 todos per wave. Fewer than 3 (except the final) means you under-split.

**Wave 1 — Foundation (4 todos, parallel-safe):**
Registry + ShortcutAction expansion + 3 bug fixes. No model changes. Tests are pure data lookups.

**Wave 2 — Tab model (4 todos, sequential within wave):**
`Surface` struct → `PaneNode::Leaf` holds `Vec<TerminalPane>` → `Workspace` accessor methods → `WorkspaceManager` orchestration. Tests for each.

**Wave 3 — Tab UI + dispatch (4 todos, parallel-safe after Wave 2):**
Tab bar renderer in `workspace_view.rs`, dispatch handlers for tab actions, closed-tabs stack, copy mode flag.

**Wave 4 — Wire up remaining shortcuts (4 todos, parallel-safe):**
Right sidebar, equalize alias, workspace prev/next with `Ctrl+Cmd+[ / ]`, browser/window stubs.

**Wave 5 — Integration + final wave:**
Comprehensive registry + dispatch tests run; final verification wave F1-F4.

### Dependency matrix
| Todo | Depends on | Blocks | Can parallelize with |
| --- | --- | --- | --- |
| 1. Expand ShortcutAction enum | — | 2, 4 | 3, 5, 6 |
| 2. Register all cmux shortcuts in registry | 1 | 15, 16 | 3, 5, 6 |
| 3. Fix dual-pass event leak (Escape/Enter) | — | 16 | 1, 2, 5, 6 |
| 4. Fix focus direction modifiers (all ⌥⌘Arrow) | — | 15, 16 | 1, 2, 3, 5, 6 |
| 5. Add Surface struct in `workspace/surface.rs` | — | 6, 7, 8 | 1, 2, 3, 4 |
| 6. Refactor `PaneNode::Leaf` to hold `Vec<TerminalPane>` | 5 | 7, 8, 10 | — |
| 7. Add `Workspace` tab methods (new/next/prev/select/close/rename) | 5, 6 | 8, 10 | — |
| 8. Add `WorkspaceManager` tab orchestration | 5, 6, 7 | 9, 10 | — |
| 9. Add closed-tabs stack + ReopenLastClosed | 8 | 15, 16 | 10, 11 |
| 10. Tab bar UI in `workspace_view.rs` | 6, 7, 8 | 14 | 11, 12, 13 |
| 11. Add ToggleCopyMode on `TerminalPane` | — | 14, 15, 16 | 9, 10, 12, 13 |
| 12. Add ToggleRightSidebar on `SidebarView` | — | 14, 15, 16 | 9, 10, 11, 13 |
| 13. Add workspace prev/next aliases (Ctrl+Cmd+[/]) | — | 14, 15, 16 | 9, 10, 11, 12 |
| 14. Wire all new dispatch handlers | 9, 10, 11, 12, 13 | 15, 16 | — |
| 15. Write registry lookup tests for all 28 actions | 2, 4 | 17 | 16 |
| 16. Write dispatch integration tests for all handlers | 3, 4, 14 | 17 | 15 |
| 17. Final integration test (registry + dispatch cross-check) | 15, 16 | F1-F4 | — |

## Todos
> Implementation + Test = ONE todo. Never separate.
<!-- APPEND TASK BATCHES BELOW THIS LINE WITH edit/apply_patch - never rewrite the headers above. -->

### Wave 1 — Foundation
- [x] 1. Expand `ShortcutAction` enum with 12 new variants for cmux shortcuts
  What to do: Add `NewSurface`, `NextSurface`, `PreviousSurface`, `SelectSurface(usize)`, `RenameTab`, `CloseTab`, `CloseOtherTabs`, `ReopenLastClosed`, `ToggleCopyMode`, `SplitBrowserRight`, `SplitBrowserDown`, `ToggleRightSidebar`, `NewWindow` (stub), `CloseWindow` (stub), `EqualizeSplitsAlt` (alias for `Ctrl+Cmd+=`), `PrevWorkspaceAlt`, `NextWorkspaceAlt`. Use `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`. Keep existing variants untouched.
  Parallelization: Wave 1 | Blocked by: — | Blocks: 2, 4
  References: `crates/rmux-app/src/shortcuts.rs:10-75` (current enum), `:103-234` (default impl)
  Acceptance: `cargo build -p rmux-app` succeeds; `grep "ShortcutAction::" src/shortcuts.rs | wc -l` shows ≥33 variants
  QA scenarios:
    - Happy: `cargo build -p rmux-app 2>&1 | grep -c "error"` == 0
    - Failure: missing derive → build fails; added to verify the enum is still `Copy + Eq`
  Evidence: `.omo/start-work/ledger.jsonl` line with `task: 1, artifact: build.log`
  Commit: N (part of wave commit)

- [x] 2. Register all cmux shortcuts in `ShortcutRegistry::default()`
  What to do: Add 17 new registrations matching cmux: `Cmd+T→NewSurface`, `Cmd+Shift+]→NextSurface`, `Cmd+Shift+[→PreviousSurface`, `Ctrl+1..9→SelectSurface(0..8)` (using `Modifiers::CTRL` not `cmd_ctrl()` — this is the macOS-only Ctrl shortcut per cmux), `Cmd+R→RenameTab`, `Cmd+W→CloseTab` (in addition to existing `ClosePane`), `Opt+Cmd+T→CloseOtherTabs`, `Cmd+Shift+T→ReopenLastClosed`, `Cmd+Shift+M→ToggleCopyMode`, `Opt+Cmd+D→SplitBrowserRight` (stub), `Opt+Cmd+Shift+D→SplitBrowserDown` (stub), `Opt+Cmd+ArrowLeft→FocusLeft`, `Opt+Cmd+ArrowRight→FocusRight`, `Opt+Cmd+ArrowUp→FocusUp`, `Opt+Cmd+ArrowDown→FocusDown`, `Ctrl+Cmd+=→EqualizeSplits` (alias), `Ctrl+Cmd+]→NextWorkspace`, `Ctrl+Cmd+[→PrevWorkspace`, `Cmd+Opt+B→ToggleRightSidebar`. Update `default()` to be the single source of truth. Move existing 35+ registrations into ordered groups with section comments.
  Parallelization: Wave 1 | Blocked by: 1 | Blocks: 15, 16
  References: `crates/rmux-app/src/shortcuts.rs:103-234` (existing default impl), `Cargo.toml:13-14` (egui version for `Key` enum)
  Acceptance: `grep -c "reg.register" src/shortcuts.rs` shows ≥52 registrations; all use canonical `cmd_ctrl()`/`cmd_ctrl_shift()`/`cmd_ctrl_alt()` helpers
  QA scenarios:
    - Happy: build succeeds, existing 3 tests still pass
    - Failure: duplicate `(mods, key)` registration — last write wins, silently broken; verify with `cargo test test_registry_default_includes_quit`
  Evidence: `test log` + count of registrations
  Commit: N (part of wave commit)

- [x] 3. Fix `Escape` and `Enter` event leak in `shortcut_handler.rs`
  What to do: In `handle_keyboard_shortcuts`, the first pass (lines 14-51) processes ALL events including `Modifiers::NONE` Escape/Enter. This steals from text-focused widgets. Fix by adding `if !ctx.wants_keyboard_input()` guard at the TOP of the first pass too, OR — preferred — consolidate the two passes into one with a single `wants_keyboard_input` check that runs before the loop, and only skip the loop for non-always-active shortcuts when focus is held. Define a per-action `is_always_active()` method on `ShortcutAction` that returns true only for `Quit`, `Copy`, `FontSizeUp/Down/Reset`, `ClearScreen`, `ClearScrollback` (these work even in text fields). Other actions skip if `wants_keyboard_input()`. Keep the two-pass code structure but add the guard to pass 1 as well, then refactor to the single-pass version in a follow-up.
  Parallelization: Wave 1 | Blocked by: — | Blocks: 16
  References: `crates/rmux-app/src/shortcut_handler.rs:9-96` (current dual-pass), `crates/rmux-app/src/ui/terminal_pane.rs:334` (`handle_keyboard_input` that consumes Escape/Enter)
  Acceptance: Writing a unit test that constructs a fake `egui::Context` is impractical (egui Context is opaque). Instead, extract the focus-check logic into a pure function `should_dispatch_when_text_focused(action: ShortcutAction) -> bool` and test that directly. Assert `should_dispatch_when_text_focused(ShortcutAction::Find) == false`, `should_dispatch_when_text_focused(ShortcutAction::Quit) == true`, etc.
  QA scenarios:
    - Happy: `should_dispatch_when_text_focused(Quit) == true`, `should_dispatch_when_text_focused(NewSurface) == false`
    - Failure: if we forget `Copy`, `Copy` would be lost during text input — test catches it
  Evidence: test output showing 8+ assertions
  Commit: N

- [x] 4. Fix focus direction modifier inconsistency
  What to do: Change `FocusLeft` and `FocusUp` to require `cmd_ctrl_alt()` (matching Right/Down) and remove the now-redundant `FocusRight`/`FocusDown` plain `cmd_ctrl` registrations. Result: all four focus directions use `⌥⌘Arrow` on macOS / `Ctrl+Alt+Arrow` on Linux/Windows. Keep `FocusLeft`/`FocusUp` plain `cmd_ctrl` for `cmd_ctrl_shift` arrows too if any (no — verify none exist).
  Parallelization: Wave 1 | Blocked by: — | Blocks: 15, 16
  References: `crates/rmux-app/src/shortcuts.rs:220-230` (current focus registrations), `crates/rmux-app/src/workspace/model.rs:175-208` (focus_left/right/up/down methods)
  Acceptance: All 4 directions registered with `cmd_ctrl_alt()`; `cargo test -p rmux-app shortcuts::tests` passes; new test `test_focus_modifiers_all_match_cmd_ctrl_alt` asserts all 4 use `cmd_ctrl_alt()`
  QA scenarios:
    - Happy: `lookup(cmd_ctrl_alt(), Key::ArrowLeft) == Some(FocusLeft)` and same for Right/Up/Down
    - Failure: if we miss one direction, the test fails — that's the bug
  Evidence: passing test
  Commit: N

### Wave 2 — Tab (Surface) model
- [x] 5. Add `Surface` struct and `SurfaceId` in new file `crates/rmux-app/src/workspace/surface.rs`
  What to do: New file. `Surface { id: SurfaceId(u64), title: String, terminal: TerminalPane }`. Add `SurfaceId(u64)` with `Copy, Clone, Eq, PartialEq, Debug, Hash`. Provide `Surface::new(id, title, terminal) -> Self`, `Surface::display_title() -> &str` (truncates to 20 chars). Tests: `test_surface_creation`, `test_surface_display_title_truncates`, `test_surface_id_uniqueness`.
  Parallelization: Wave 2 | Blocked by: — | Blocks: 6, 7, 8
  References: `crates/rmux-app/src/workspace/splits.rs` (for `PaneId` style), `crates/rmux-app/src/ui/terminal_pane.rs` (`TerminalPane` struct)
  Acceptance: `cargo test -p rmux-app workspace::surface::tests` passes 3+ tests
  QA scenarios:
    - Happy: `Surface::new(1, "foo", terminal)` returns struct with id=1, title="foo"
    - Failure: empty title → `display_title()` returns "" (not a panic)
  Evidence: test log
  Commit: Y | `feat(workspace): add Surface struct for tab model`

- [x] 6. Refactor `PaneNode::Leaf` to hold `Vec<Surface>` instead of single `TerminalPane`
  What to do: In `crates/rmux-app/src/workspace/splits.rs`, change `PaneNode::Leaf { pane: TerminalPane, active_surface: usize, surfaces: Vec<Surface> }`. Add accessors: `leaf_surfaces(&self) -> &[Surface]`, `leaf_surfaces_mut(&mut self) -> &mut Vec<Surface>`, `active_surface(&self) -> usize`, `set_active_surface(&mut self, idx: usize)`, `add_surface(&mut self, surface: Surface)`, `remove_surface(&mut self, idx: usize) -> Option<Surface>`, `active_surface_mut(&mut self) -> Option<&mut Surface>`, `active_terminal(&self) -> Option<&TerminalPane>`, `active_terminal_mut(&mut self) -> Option<&mut TerminalPane>`. Update all `find_terminal_*` methods to walk to `surfaces[active_surface].terminal`. Update `find_browser_mut` to skip non-leaf nodes. Update `find_leaf` semantics: a "leaf" is still a single `PaneNode` but now holds multiple surfaces; the `active_pane` in `Workspace` is still a `PaneId` identifying the leaf, and `Workspace` tracks `active_surface: HashMap<PaneId, usize>` so each pane leaf has its own active tab. **Migration:** `active_terminal()` calls elsewhere must continue to work; the `Workspace` model adds `active_surface: HashMap<PaneId, usize>`. Tests: `test_leaf_holds_multiple_surfaces`, `test_active_surface_default_is_zero`, `test_remove_surface_decrements_active_index`, `test_active_terminal_returns_active_surface_terminal`.
  Parallelization: Wave 2 | Blocked by: 5 | Blocks: 7, 8, 10
  References: `crates/rmux-app/src/workspace/splits.rs` (entire file, 559 lines — may need split), `crates/rmux-app/src/workspace/model.rs:175-208` (focus methods on `Workspace`)
  Acceptance: `cargo build -p rmux-app` succeeds; all existing `workspace::mod::tests` still pass (17 tests); 4 new tests pass
  QA scenarios:
    - Happy: leaf with 3 surfaces, active_surface=1 → `active_terminal()` returns surfaces[1].terminal
    - Failure: remove active surface (idx=1) of 3 → active_surface becomes 0 (clamps), not 1 (which would be the now-removed surface)
  Evidence: test output
  Commit: Y | `refactor(workspace): PaneNode::Leaf holds Vec<Surface>`

- [x] 7. Add tab methods to `Workspace`: new_surface, next_surface, prev_surface, select_surface, close_surface, rename_surface, close_other_surfaces
  What to do: In `crates/rmux-app/src/workspace/model.rs`, add methods on `Workspace`. Each operates on the active pane (`self.active_pane`) and its surface list. `new_surface(&mut self, title: String) -> Result<SurfaceId, WorkspaceError>` creates a new surface with a fresh PTY, `next_surface(&mut self)`, `prev_surface(&mut self)`, `select_surface(&mut self, idx: usize) -> Result<(), WorkspaceError>`, `close_surface(&mut self, idx: usize) -> Result<Surface, WorkspaceError>` (returns the closed Surface for reopen stack), `rename_surface(&mut self, idx: usize, title: String)`, `close_other_surfaces(&mut self) -> Vec<Surface>` (returns all closed for reopen). `WorkspaceError` enum: `NoActivePane`, `InvalidSurfaceIndex`, `CannotCloseLastSurface`. Tests for each.
  Parallelization: Wave 2 | Blocked by: 5, 6 | Blocks: 8, 10
  References: `crates/rmux-app/src/workspace/model.rs` (whole file), `crates/rmux-app/src/workspace/splits.rs:478-494` (equalize_splits pattern)
  Acceptance: `cargo test -p rmux-app workspace::model::tests` passes 10+ new tests
  QA scenarios:
    - Happy: workspace with 3 surfaces, `next_surface()` → active=1, `next_surface()` → active=2, `next_surface()` → active=0 (wraps)
    - Failure: `close_surface(99)` → `Err(InvalidSurfaceIndex)`; `close_surface(0)` on single-surface workspace → `Err(CannotCloseLastSurface)`
  Evidence: test log
  Commit: Y | `feat(workspace): add tab methods to Workspace`

- [x] 8. Add tab orchestration to `WorkspaceManager`: new_surface_in_active, next_surface, etc.
  What to do: In `crates/rmux-app/src/workspace/mod.rs`, add thin pass-through methods that call into `active_mut()` and delegate to the `Workspace` tab methods. Also add `closed_tabs_stack: VecDeque<ClosedTab>` (where `ClosedTab { surface: Surface, workspace_id: WorkspaceId, pane_id: PaneId }`) and `reopen_last_closed_tab(&mut self) -> Result<(), WorkspaceError>` that pops the stack and re-inserts the surface into its original pane (or active pane if original gone). Tests for each manager method.
  Parallelization: Wave 2 | Blocked by: 5, 6, 7 | Blocks: 9, 10
  References: `crates/rmux-app/src/workspace/mod.rs:1-350` (current manager), `crates/rmux-app/src/app.rs:200-255` (event publication patterns for create_workspace_with_terminal, close_active_pane_with_event)
  Acceptance: `cargo test -p rmux-app workspace::mod::tests` passes 6+ new tests
  QA scenarios:
    - Happy: close surface in pane A → reopen → surface restored in pane A; close surface then close pane A → reopen → restored in current active pane
    - Failure: reopen stack empty → `Err(WorkspaceError::NoClosedTabs)`
  Evidence: test log
  Commit: Y | `feat(workspace): WorkspaceManager tab orchestration + reopen stack`

### Wave 3 — Tab UI + dispatch
- [x] 9. Add bounded `closed_tabs_stack` with MAX_CLOSED=16, expose on `WorkspaceManager`
  What to do: In `workspace/mod.rs`, add `const MAX_CLOSED_TABS: usize = 16;` and ensure `reopen_last_closed_tab` pops from front, trims to MAX_CLOSED_TABS. Tests: `test_reopen_stack_trims_to_max`, `test_reopen_stack_pops_front`.
  Parallelization: Wave 3 | Blocked by: 8 | Blocks: 15, 16
  References: `crates/rmux-app/src/workspace/mod.rs` (new struct fields), `std::collections::VecDeque`
  Acceptance: 2 new tests pass
  QA scenarios:
    - Happy: push 20 surfaces onto stack, pop returns 20th-closed, stack size is 16
    - Failure: pop from empty stack returns `None`
  Evidence: test log
  Commit: N (rolled into 8 commit)

- [x] 10. Add tab bar UI in `ui/workspace_view.rs` rendered above each leaf pane
  What to do: In `crates/rmux-app/src/ui/workspace_view.rs`, modify `render_pane_tree` to render a tab strip above each `PaneNode::Leaf` when `leaf.surfaces.len() > 1` (hide for single-tab panes to save space). Tab strip shows: each tab title as a button (clicking selects), an `x` button on the active tab to close it, and a `+` button to create a new surface. Use `egui::TopBottomPanel::top` inside a child UI for the tab bar. Update `crates/rmux-app/src/ui/workspace_view.rs` to track the active pane+surface for input routing. Add the `egui::Response::clicked()` handling for tab buttons. Tests: `test_tab_bar_renders_only_when_multiple_surfaces` (use a `egui_kittest` if available, otherwise document as visual QA).
  Parallelization: Wave 3 | Blocked by: 6, 7, 8 | Blocks: 14
  References: `crates/rmux-app/src/ui/workspace_view.rs` (full file), `crates/rmux-app/src/ui/terminal_pane.rs` (rendering pattern)
  Acceptance: `cargo build -p rmux-app` succeeds; new test passes (or documented as visual-only)
  QA scenarios:
    - Visual: render workspace with 3 surfaces in one pane → tab bar visible with 3 buttons + `+`
    - Visual: render with 1 surface → tab bar hidden
  Evidence: screenshot or test result
  Commit: Y | `feat(ui): tab bar for multi-surface panes`

- [x] 11. Add `ToggleCopyMode` flag on `TerminalPane` with getter/setter and copy-mode visual indicator
  What to do: In `crates/rmux-app/src/ui/terminal_pane.rs`, add `copy_mode: bool` field on `TerminalPane`. In copy mode, the terminal pane swallows mouse events for selection instead of forwarding to PTY, and renders a `[COPY]` indicator in the title bar. Add `toggle_copy_mode()`, `is_copy_mode()` methods. Tests: `test_copy_mode_toggle`, `test_copy_mode_default_false`.
  Parallelization: Wave 3 | Blocked by: — | Blocks: 14, 15, 16
  References: `crates/rmux-app/src/ui/terminal_pane.rs` (struct definition)
  Acceptance: 2 new tests pass
  QA scenarios:
    - Happy: `terminal.toggle_copy_mode()` → `is_copy_mode() == true`; toggle again → false
    - Failure: double-toggle returns to original state
  Evidence: test log
  Commit: Y | `feat(terminal): add copy mode flag`

- [x] 12. Add `right_sidebar_visible: bool` to `SidebarView` and `ToggleRightSidebar` action
  What to do: In `crates/rmux-app/src/ui/sidebar.rs`, add `right_sidebar_visible: bool` field with `toggle_right()` method. In `crates/rmux-app/src/shortcuts.rs`, register `Cmd+Opt+B → ToggleRightSidebar` (using `cmd_alt()` helper). In `app.rs`, render the right sidebar panel conditionally on `right_sidebar_visible`. Reuse the existing notification panel as the "right sidebar" content for now. Tests: `test_toggle_right_sidebar_initial_false`, `test_toggle_right_sidebar_flips_state`.
  Parallelization: Wave 3 | Blocked by: — | Blocks: 14, 15, 16
  References: `crates/rmux-app/src/ui/sidebar.rs` (struct), `crates/rmux-app/src/app.rs:119-139` (panel rendering), `crates/rmux-app/src/ui/notification_panel.rs`
  Acceptance: 2 new tests pass; right sidebar hidden by default
  QA scenarios:
    - Happy: `sidebar.toggle_right()` flips `right_sidebar_visible`
    - Failure: initial state must be `false` (not undefined)
  Evidence: test log
  Commit: Y | `feat(ui): add right sidebar toggle`

- [x] 13. Add `Ctrl+Cmd+[` / `Ctrl+Cmd+]` aliases for `PrevWorkspace` / `NextWorkspace` (rolled into W1.2)
  What to do: In `crates/rmux-app/src/shortcuts.rs`, the existing `Cmd+Shift+[` / `Cmd+Shift+]` already work. Add NEW registrations using `Modifiers::CTRL | Modifiers::COMMAND | Modifiers::SHIFT` (or `Modifiers::COMMAND | Modifiers::CTRL` + SHIFT — verify by checking the `lookup_mods` normalization in `shortcut_handler.rs:28-44`). Since the handler normalizes `Ctrl+Cmd` on macOS to pass through unchanged, the registration must use `Modifiers::CTRL | Modifiers::COMMAND | Modifiers::SHIFT`. Tests: `test_ctrl_cmd_bracket_prev_workspace`, `test_ctrl_cmd_bracket_next_workspace`.
  Parallelization: Wave 3 | Blocked by: — | Blocks: 14, 15, 16
  References: `crates/rmux-app/src/shortcuts.rs:214-218` (existing bracket shortcuts), `crates/rmux-app/src/shortcut_handler.rs:28-44` (modifier normalization)
  Acceptance: 2 new tests pass; cmux `⌃⌘[` / `⌃⌘]` work alongside existing `⌘⇧[` / `⌘⇧]`
  QA scenarios:
    - Happy: `lookup(Modifiers::CTRL | Modifiers::COMMAND | Modifiers::SHIFT, Key::OpenBracket)` on macOS returns `Some(PrevWorkspace)`
    - Failure: forgot the shift → matches bare `Cmd+[` (wrong action)
  Evidence: test log
  Commit: N

### Wave 4 — Wire up + integration
- [x] 14. Add all new dispatch handlers in `shortcut_handler.rs::dispatch_shortcut_action`
  What to do: Add match arms for all 16 new actions: `NewSurface` → `self.workspace_manager.new_surface_in_active(...)`, `NextSurface` → `next_surface`, `PreviousSurface` → `prev_surface`, `SelectSurface(idx)` → `select_surface(idx)`, `RenameTab` → start tab rename (use a new field on `Workspace` or pass through to top bar), `CloseTab` → close active surface (fall back to close pane if last), `CloseOtherTabs` → `close_other_surfaces`, `ReopenLastClosed` → `reopen_last_closed_tab`, `ToggleCopyMode` → `active_terminal_mut().toggle_copy_mode()`, `SplitBrowserRight/Down` → `tracing::warn!("browser split not yet implemented")`, `ToggleRightSidebar` → `self.sidebar.toggle_right()`, `NewWindow`/`CloseWindow` → `tracing::warn!("multi-window not yet implemented")`, `EqualizeSplitsAlt` → call same handler as `EqualizeSplits`, `PrevWorkspaceAlt`/`NextWorkspaceAlt` → call same as existing. For `CloseTab` vs `ClosePane`: check if the active leaf has >1 surface; if yes, close the surface; if no, fall through to close pane. This keeps `Cmd+W` as "close the closest thing."
  Parallelization: Wave 4 | Blocked by: 9, 10, 11, 12, 13 | Blocks: 15, 16
  References: `crates/rmux-app/src/shortcut_handler.rs:108-294` (current dispatch), `crates/rmux-app/src/app.rs:200-255` (event publication patterns)
  Acceptance: `cargo build -p rmux-app` succeeds; all match arms have a body
  QA scenarios:
    - Happy: every variant compiles
    - Failure: missing arm → compile error
  Evidence: build log
  Commit: Y | `feat(shortcuts): wire up all cmux dispatch handlers`

- [x] 15. Write registry lookup tests for all 28 cmux actions
  What to do: In `crates/rmux-app/src/shortcuts.rs` tests module, add one test per registered shortcut (28 tests): `test_cmd_t_new_surface`, `test_cmd_shift_bracket_next_surface`, ..., `test_ctrl_cmd_bracket_prev_workspace`, `test_cmd_opt_b_toggle_right_sidebar`, etc. Each test uses the platform-conditional canonical modifier. Tests follow the pattern `assert_eq!(reg.lookup(mods, Key::X), Some(ShortcutAction::Variant))`. Use a helper `fn canonical_mod(without: Modifiers) -> Modifiers` to reduce duplication.
  Parallelization: Wave 4 | Blocked by: 2, 4 | Blocks: 17
  References: `crates/rmux-app/src/shortcuts.rs:258-281` (existing test module)
  Acceptance: `cargo test -p rmux-app shortcuts::tests` runs 31+ tests (3 existing + 28 new), all pass
  QA scenarios:
    - Happy: 28 tests pass
    - Failure: a shortcut was registered with wrong modifier → test fails with clear diff
  Evidence: test log
  Commit: Y | `test(shortcuts): registry lookup tests for all cmux actions`

- [x] 16. Write dispatch handler tests for all 28 actions
  What to do: In a new `crates/rmux-app/src/shortcut_handler_tests.rs` file, write one test per action that verifies the dispatch handler invokes the correct method. For pure handlers (e.g. `ToggleRightSidebar` on `SidebarView`), instantiate the struct and assert state change. For handlers that need `WorkspaceManager`, use the existing `WorkspaceManager::new()` (which spawns a shell — that may need a mock or use the test in an environment where PTY is available). For handlers that hit `tracing::warn!` (browser/window stubs), use `tracing-test` or a similar test infrastructure to capture the log; if not available, document as "manual verification only" and assert the handler compiles.
  Parallelization: Wave 4 | Blocked by: 3, 4, 14 | Blocks: 17
  References: `crates/rmux-app/src/shortcut_handler.rs:102-297` (current dispatch), `crates/rmux-app/src/workspace/mod.rs:359-558` (test patterns)
  Acceptance: 28 dispatch tests pass (or documented as manual-only for stubs)
  QA scenarios:
    - Happy: `NewSurface` handler creates a new surface, active_surface moves to new idx
    - Failure: handler for `ToggleCopyMode` doesn't call the method → test fails
  Evidence: test log
  Commit: Y | `test(shortcuts): dispatch handler tests for all actions`

- [x] 17. Final integration test: every registered shortcut has both a registry test AND a dispatch test
  What to do: In a new `crates/rmux-app/tests/keybindings_integration.rs` integration test, iterate over a static list of `(canonical_mods, key, action)` tuples that includes ALL 28 cmux shortcuts, assert each is registered, then for each non-stub action, assert the corresponding `WorkspaceManager`/`Workspace`/`SidebarView`/`TerminalPane` method exists and is wired. This is a regression guard against future drift. Use `pretty_assertions` if available, otherwise `assert_eq!`. The test must panic with a clear message if a shortcut is missing.
  Parallelization: Wave 5 | Blocked by: 15, 16 | Blocks: F1-F4
  References: `crates/rmux-app/src/shortcuts.rs:103-234` (current registrations), `crates/rmux-app/src/shortcut_handler.rs:108-294` (current dispatch)
  Acceptance: `cargo test -p rmux-app --test keybindings_integration` passes with 28 rows checked
  QA scenarios:
    - Happy: 28 rows × 2 assertions = 56 assertions all pass
    - Failure: missing shortcut → panic with the action name and the assertion that failed
  Evidence: test log
  Commit: Y | `test(shortcuts): integration guard for all keybindings`

## Final verification wave
> Runs in parallel after ALL todos. ALL must APPROVE. Surface results and wait for the user's explicit okay before declaring complete.
- [ ] F1. Plan compliance audit — every top-level todo done; every test referenced exists and passes; scope NOT-haves respected
- [ ] F2. Code quality review — `cargo clippy --workspace --all-targets -- -D warnings` zero warnings; `cargo fmt --all -- --check` clean; no `unsafe`; no new dependencies; files ≤350 lines (split `shortcut_handler.rs` if needed)
- [ ] F3. Real manual QA — `cargo run -p rmux-app` launches; press every registered shortcut in the running app (or document the ones that need an actual GUI session via screenshot+key capture); verify each triggers the expected action
- [ ] F4. Scope fidelity — confirm tab model added, registry bugs fixed, all 28 cmux shortcuts registered; confirm multi-window + browser split + session restore are NOT implemented (per Must NOT have)

## Commit strategy
- **Wave commits** group related changes (e.g. all Wave 2 tab model changes in one commit)
- Conventional commits format: `feat(scope): summary`, `test(scope): summary`, `refactor(scope): summary`, `fix(scope): summary`
- One commit per todo marked `Commit: Y` (15 commits total)
- Final commit after F1-F4 approve: `docs: update PLAN.md keyboard shortcuts table` (add new cmux shortcuts to the table in `README.md`)
- Push to `feat/fix-keybindings` branch at end

## Success criteria
- `cargo build --workspace` exit 0
- `cargo test --workspace` exit 0 with **≥60 new tests** added (28 registry + 28 dispatch + 4 integration)
- `cargo clippy --workspace --all-targets -- -D warnings` exit 0
- `cargo fmt --all -- --check` exit 0
- All 28 cmux shortcuts registered in `ShortcutRegistry::default()`
- All 28 cmux shortcuts have a dispatch handler (or documented `tracing::warn!` stub for browser/window)
- `Escape` and `Enter` no longer leak through to text-focused widgets
- Focus direction shortcuts all use `⌥⌘Arrow` (consistent with cmux)
- `Cmd+W` closes surface when ≥2 surfaces, falls through to close pane when 1
- `Cmd+Shift+T` reopens the last closed surface (bounded to 16)
- `Cmd+Shift+M` toggles copy mode on the active terminal
- `Cmd+Opt+B` toggles the right sidebar
