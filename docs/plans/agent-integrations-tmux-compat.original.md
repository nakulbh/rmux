# Plan: Agent Integrations (tmux-compat + Claude Code Teams)

**Branch:** `feat/phase-6-agent-hooks` only  
**Status:** implementation in progress  
**Depends on:** Phase 6 hooks notify MVP (shipped: Claude + OpenCode lifecycle hooks)

---

## 1. Research (preserve — source of truth)

### 1.1 What Settings → Agent Integrations is

cmux Settings list (Claude Code Teams, oh-my-opencode, oh-my-codex, oh-my-pi, oh-my-claudecode) is **not** the same as lifecycle hooks.

| Layer | Purpose | rmux status |
|---|---|---|
| **Lifecycle hooks** (`hooks setup`) | Notify + sidebar status when agent needs input / finishes | ✅ Claude + OpenCode |
| **Agent integrations** | Multi-agent **teams** spawn **native splits** (agent thinks it talks to **tmux**) | ❌ this plan |

Docs:
- https://cmux.com/docs/agent-integrations/claude-code-teams
- https://cmux.com/docs/agent-integrations/oh-my-opencode
- https://cmux.com/docs/agent-integrations/oh-my-codex
- https://cmux.com/docs/agent-integrations/oh-my-claudecode
- https://cmux.com/docs/agent-integrations/oh-my-pi (hooks-only; no tmux shim)

cmux source (reference):
- `CLI/CMUXCLI+TmuxCompatSupport.swift`
- `CLI/CMUXCLI+TmuxCompatResizePane.swift`
- `CLI/CMUXCLI+TmuxCompatHUDSupport.swift`
- `daemon/remote/cmd/cmuxd-remote/tmux_compat.go`

### 1.2 How cmux does it (architecture)

Agents (Claude Teams, OMO, OMX, OMC) spawn worker panes via **tmux**. cmux does not reimplement orchestration. It **shims tmux**:

```text
Agent CLI
  │  runs: tmux split-window / send-keys / select-pane / …
  ▼
PATH shim:  ~/.cmuxterm/<integration>-bin/tmux
  │  → cmux __tmux-compat …
  ▼
cmux socket API (split, send, focus, create workspace, kill, list)
  ▼
Native cmux splits + hooks notifications in each pane
```

**Per launch:**
1. Create tmux shim dir (e.g. `~/.cmuxterm/claude-teams-bin/tmux` → `cmux __tmux-compat`)
2. Prepend to `PATH`
3. Fake `TMUX` + `TMUX_PANE` (encode current workspace/pane)
4. Set `CMUX_SOCKET_PATH`
5. Integration flags (e.g. `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`)
6. `exec` real agent

**Tmux → cmux map (Claude Teams docs):**

| tmux | cmux |
|---|---|
| `new-session` / `new-window` | new workspace |
| `split-window` | split pane |
| `send-keys` | send text to surface |
| `capture-pane` | read terminal text |
| `select-pane` / `select-window` | focus |
| `kill-pane` / `kill-window` | close |
| `list-panes` / `list-windows` | list |

Store: `~/.cmuxterm/tmux-compat-store.json` (buffers/hooks state).

**Per integration extras:**

| Integration | CLI | Extra |
|---|---|---|
| Claude Code Teams | `cmux claude-teams` | `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`, teammate mode auto |
| oh-my-opencode | `cmux omo` | Shadow `OPENCODE_CONFIG_DIR`, `tmux.enabled=true` in plugin config |
| oh-my-codex | `cmux omx` | PATH shim + exec `omx` |
| oh-my-claudecode | `cmux omc` | Shim + NODE_OPTIONS restore |
| oh-my-pi | hooks only | `cmux hooks setup omp` → `~/.omp/.../cmux-omp-session.ts` |

**Hooks role in teams:** each teammate pane is normal PTY; lifecycle hooks fire Stop/Notification → badges. Without hooks: splits work, no attention signals. Without tmux-compat: hooks only on single pane, no multi-agent grid.

**Hard lesson from cmux (issue #6447):** teammate start commands are shell expressions (`cd … && env … claude …`). Must wrap as `/bin/sh -c '…'` when spawning — cannot exec `cd` as binary.

### 1.3 rmux inventory (current)

| Capability | Status |
|---|---|
| `notification.create` + workspace/pane routing | ✅ |
| `hooks setup` Claude / OpenCode | ✅ |
| PTY env `RMUX_WORKSPACE_ID`, `RMUX_PANE_ID`, `RMUX_SOCKET_PATH` | ✅ |
| `workspace.create` / `list` / `select` / `close` | ✅ |
| `surface.split` (right/down) + spawn shell | ✅ |
| `surface.focus` by pane_id | ✅ |
| `surface.send_text` / `send_key` | ✅ **active pane only** — need target pane |
| `surface.list` | ✅ |
| `surface.close` / kill pane via API | ❌ need or use internal close |
| `capture-pane` (read grid) | ❌ later |
| split with start command | ❌ send after split for MVP |
| `__tmux-compat` + launchers | ❌ this plan |

---

## 2. Target architecture for rmux

```text
rmux-cli claude-teams [args…]
  ├─ write ~/.rmuxterm/claude-teams-bin/tmux → rmux-cli __tmux-compat "$@"
  ├─ export TMUX, TMUX_PANE, RMUX_SOCKET_PATH, CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1
  ├─ prepend shim dir to PATH
  └─ exec claude [args…]  (teammate mode auto)

rmux-cli __tmux-compat <tmux-args…>
  ├─ parse fake TMUX / store ~/.rmuxterm/tmux-compat-store.json
  ├─ map %pane ↔ (workspace_id, pane_id)
  └─ socket: surface.split | send_text | focus | list | close | workspace.create
```

New PTYs already get `RMUX_*` → hooks keep working in teammate panes.

---

## 3. Phased implementation

### Phase A — MVP (this branch now) — Claude Code Teams

1. **API:** optional `pane_id` on `surface.send_text` (and prefer write to that pane without requiring prior focus if easy).
2. **API:** `surface.close` `{ pane_id }` if missing (or map kill-pane to existing close path).
3. **`rmux-cli __tmux-compat`:** implement subset:
   - `split-window` (-h/-v, `-t target`, `-c cwd`, shell command)
   - `send-keys` (-t target)
   - `select-pane` (-t)
   - `list-panes` / `list-windows` (minimal format)
   - `kill-pane` (-t)
   - `new-window` / `new-session` → workspace.create (best-effort)
   - Unknown commands: exit 0 or 1 without crashing agent (match tmux fail soft where needed)
4. **Store:** `~/.rmuxterm/tmux-compat-store.json` — pane map, session id, counter for fake pane ids (`%0`, `%1`, …).
5. **`rmux-cli claude-teams`:** create shim bin, env, exec `claude`.
6. **Tests:** unit parse of tmux argv; store roundtrip; split mapping -h→right, -v→down.
7. **Docs:** extend `docs/guide/agent-hooks.md` or add `docs/guide/agent-integrations.md`.

**Out of Phase A:** capture-pane, HUD, resize-pane geometry, OMO/OMX/OMC launchers, Settings UI.

### Phase B — Generic launchers (later)

- Shared integration runner
- `omo` / `omx` / `omc` thin configs
- OMO shadow config under `~/.rmuxterm/omo-config/`

### Phase C — Hooks on every integration

- Document that `hooks setup` required for team notifications
- Optional first-run auto setup

### Phase D — Settings UI

- Agent Integrations list like cmux screenshot

### Phase E — oh-my-pi style

- More `hooks setup` agents only

---

## 4. Phase A file layout

```
crates/rmux-cli/src/
  tmux_compat/
    mod.rs          # entry: run(argv) 
    parse.rs        # parse tmux-like argv
    store.rs        # ~/.rmuxterm/tmux-compat-store.json
    map.rs          # fake pane id ↔ rmux ids
  launchers/
    mod.rs
    claude_teams.rs # create shim + exec
  main.rs           # __tmux-compat, claude-teams subcommands
  commands.rs       # wire socket helpers

crates/rmux-api/src/methods.rs   # send_text pane_id?; surface.close
crates/rmux-app/src/api_dispatch.rs

docs/plans/agent-integrations-tmux-compat.md  # compact plan
docs/plans/agent-integrations-tmux-compat.original.md  # this file
docs/guide/agent-integrations.md
```

---

## 5. Acceptance (Phase A)

- [ ] `rmux-cli claude-teams` starts Claude with agent teams env
- [ ] Fake `tmux` on PATH calls `rmux-cli __tmux-compat`
- [ ] `split-window -h` creates right split in running rmux
- [ ] `send-keys -t %N` reaches correct pane
- [ ] `select-pane -t %N` focuses
- [ ] `list-panes` prints something agent can parse
- [ ] Hooks still fire in new panes (existing install)
- [ ] `cargo test` / clippy / fmt green

---

## 6. Risks

| Risk | Mitigation |
|---|---|
| Agent needs full tmux format strings | Start with minimal formats Claude Teams uses; expand on failure |
| send-keys special keys | Support Enter as `\r`; literal strings first |
| Start command is shell expression | After split: `send_text` command + enter, or future spawn-with-command |
| Real tmux on PATH | Shim dir **prepended** so it wins |
| Windows | Unix-first (socket already Unix) |

---

## 7. Implementation order (execute now)

1. API: `surface.send_text` + optional `pane_id`; `surface.close`
2. Store + pane map module
3. `__tmux-compat` command dispatcher
4. `claude-teams` launcher
5. Tests + guide doc
6. Mark checkboxes in this plan when done
