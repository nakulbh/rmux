# Agent hooks (Claude Code + OpenCode)

rmux can show **desktop notifications**, **sidebar badges**, and **status pills** when Claude Code or OpenCode needs attention or finishes a turn — the same idea as cmux’s agent hooks.

## Quick start

1. Build and ensure `rmux-cli` is on your `PATH` (or use the absolute path printed by cargo).
2. Start rmux.
3. Install hooks once:

```bash
rmux-cli hooks setup
# or only one agent:
rmux-cli hooks setup --agent claude
rmux-cli hooks setup --agent opencode
# install even if the binary is not on PATH:
rmux-cli hooks setup --force
```

4. Restart Claude Code / OpenCode sessions (so they load the new config).
5. Run an agent **inside an rmux pane**. When it stops or asks for permission, you should see a notification and a sidebar status.

Uninstall:

```bash
rmux-cli hooks uninstall
rmux-cli hooks uninstall --agent claude
```

## What gets installed

### Claude Code

Merges hooks into `~/.claude/settings.json` that call:

| Event | Command |
|---|---|
| `SessionStart` | `rmux-cli hooks claude session-start` |
| `UserPromptSubmit` | `rmux-cli hooks claude prompt-submit` |
| `Stop` | `rmux-cli hooks claude stop` |
| `Notification` | `rmux-cli hooks claude notification` |
| `SessionEnd` | `rmux-cli hooks claude session-end` |
| `PostToolUse` / `PushNotification` | `rmux-cli hooks claude push-notification` |

User hooks are preserved. Re-running setup upgrades only rmux-owned entries.

If `preferredNotifChannel` is unset, install sets it to `notifications_disabled` so Claude does not also fire its own terminal OSC notifications.

### OpenCode

Writes `~/.config/opencode/plugins/rmux-notify.js` (or `$OPENCODE_CONFIG_DIR/plugins/…`) and registers `./plugins/rmux-notify.js` in `opencode.json`.

| Plugin event | rmux handler |
|---|---|
| `session.created` / `session.updated` | `session-start` → Running |
| `session.idle` | `stop` → Completed + Idle |
| `session.error` | `notification` → Error |
| `permission.asked` | `notification` → Needs input |
| `session.status` / tool before | `status` → Running / Idle |

## Routing (which tab gets the badge)

Every rmux PTY exports:

| Variable | Meaning |
|---|---|
| `RMUX_SOCKET_PATH` | Control socket path |
| `RMUX_WORKSPACE_ID` | Workspace id |
| `RMUX_PANE_ID` | Pane id |

`rmux-cli notify` and hook handlers read these (or `--workspace` / `--pane`) and pass them to `notification.create`, so the **originating** sidebar tab gets the unread badge.

## Fail-open behavior

- If rmux is not running, hook commands exit `0` (agents never hang).
- Disable for one process: `RMUX_HOOKS_DISABLED=1`, or per agent:
  - `RMUX_CLAUDE_HOOKS_DISABLED=1`
  - `RMUX_OPENCODE_HOOKS_DISABLED=1`

## Manual test without agents

```bash
# With rmux running, inside a pane:
echo $RMUX_PANE_ID $RMUX_WORKSPACE_ID
rmux-cli notify --title "Test" --body "Hello from hooks"
echo '{}' | rmux-cli hooks claude stop
```

## Out of scope (later)

- Feed / permission reply UI (cmux Feed)
- Session resume / hibernation
- Other agents (Codex, Grok, …)
- Claude PATH wrapper (cmux-style); MVP uses global `settings.json` merge

## Related

- Phase 6 plan: `docs/PLAN.md`
- Socket API: `notification.create`, `sidebar.set_status`
