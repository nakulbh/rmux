# Agent integrations (Claude Code Teams)

Multi-agent tools that spawn **tmux** panes can open **native rmux splits** via a PATH `tmux` shim.

> Lifecycle notifications still come from [agent hooks](./agent-hooks.md). Integrations handle **layout**; hooks handle **attention**.

## Claude Code Teams

```bash
# rmux must be running; run this inside an rmux pane
rmux-cli claude-teams
rmux-cli claude-teams --model sonnet
# extra args are forwarded to `claude`
```

What it does:

1. Writes `~/.rmuxterm/claude-teams-bin/tmux` → `rmux-cli __tmux-compat "$@"`
2. Prepends that dir to `PATH`
3. Sets `TMUX`, `TMUX_PANE`, `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`
4. `exec`s real `claude`

When Claude spawns a teammate via `tmux split-window`, the shim creates an rmux split and can type the start command into the new pane.

### Requirements

- `claude` on `PATH`
- rmux app running (socket reachable)
- Recommended: `rmux-cli hooks setup --agent claude` so teammate panes notify on stop/need-input

### Env

| Variable | Role |
|---|---|
| `RMUX_SOCKET_PATH` | Control socket (also used by hooks) |
| `RMUX_WORKSPACE_ID` / `RMUX_PANE_ID` | Set by rmux PTYs; seed the tmux-compat map |
| `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` | Enables Claude agent teams |

### Store

`~/.rmuxterm/tmux-compat-store.json` maps fake `%N` pane ids to rmux `(workspace_id, pane_id)`.

## Supported tmux subset (`__tmux-compat`)

| Command | Behavior |
|---|---|
| `split-window -h/-v` | `surface.split` right/down |
| `send-keys -t %N` | `surface.send_text` to pane |
| `select-pane -t %N` | `surface.focus` |
| `list-panes` | print mapped fake ids |
| `list-windows` | list workspaces |
| `kill-pane -t %N` | `surface.close` |
| `new-window` / `new-session` | `workspace.create` (best-effort) |
| other | ignored (exit 0) so agents don't crash |

## Later (not yet)

- `omo` / `omx` / `omc` launchers (same shim pattern)
- `capture-pane`
- Settings UI “Agent Integrations”

## Research / plan

- Compact plan: `docs/plans/agent-integrations-tmux-compat.md`
- Full research backup: `docs/plans/agent-integrations-tmux-compat.original.md`
- cmux docs: https://cmux.com/docs/agent-integrations/claude-code-teams
