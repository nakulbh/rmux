# rmux-cli — Application Control Plane

`rmux-cli` controls a running `rmux` instance over the Unix socket JSON-RPC API.
Commands are hierarchical by domain so every major part of the app is scriptable.

## Global flags

| Flag | Description |
|---|---|
| `--socket <PATH>` | Socket path (overrides `$RMUX_SOCKET_PATH`) |
| `--json` | Machine-readable JSON output instead of tables |
| `-h` / `--help` | Help |
| `-V` / `--version` | Version |

Default socket: `/tmp/rmux-debug.sock` (debug builds) or `/tmp/rmux.sock` (release).

## Command tree

```text
rmux-cli
  system
    ping | capabilities | identify
  workspace
    list | create [NAME] | select <INDEX> | close <ID> | rename <ID> <NAME>
  surface
    list | split <right|down> | focus <PANE_ID> | close [PANE_ID]
    new [--title TITLE] | send <TEXT> | key <KEY>
  notification
    create --title T [--subtitle S] [--body B] | list | clear
  sidebar
    status set [--workspace ID] <TEXT>
    status clear [--workspace ID]
    progress <0.0-1.0>
  browser
    open [URL] | navigate <URL> | back | forward | reload | url
  app
    font-size [DELTA] [--reset] | theme <NAME>
  events
    stream
  call <METHOD> [PARAMS_JSON]
```

Phase 3 flat aliases still work: `ping`, `capabilities`, `notify`,
`new-workspace`, `list-workspaces`, `new-split`, `send`.

## Examples

```bash
# Health
rmux-cli system ping
rmux-cli system identify

# Workspaces
rmux-cli workspace list
rmux-cli workspace create dev
rmux-cli workspace select 0
rmux-cli workspace rename 1 "main"

# Panes
rmux-cli surface split right
rmux-cli surface list --json
rmux-cli surface send 'ls\n'
rmux-cli surface key enter
rmux-cli surface new --title shell
rmux-cli surface close

# Notifications & sidebar
rmux-cli notification create --title Build --body done
rmux-cli sidebar status set "compiling…"
rmux-cli sidebar progress 0.5

# Browser
rmux-cli browser open https://example.com
rmux-cli browser navigate https://x.ai
rmux-cli browser url

# App settings
rmux-cli app theme dracula
rmux-cli app font-size 1.0
rmux-cli app font-size --reset

# Escape hatch + events
rmux-cli call workspace.create '{"name":"tmp"}'
rmux-cli events stream
```

## Themes

`app theme` accepts: `onedark` / `dark`, `dracula`, `solarized-dark`,
`solarized-light`, `gruvbox-dark`, `catppuccin-mocha`, `tokyo-night`.

## Keys

`surface key` accepts: `enter`, `tab`, `escape`, `ctrl+c`, `ctrl+d`.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Server or local error |
| 2 | Cannot connect (is rmux running?) |

## Extending

1. Add method constant + param types in `rmux-api::methods`.
2. Implement handler in `rmux-app::api_dispatch`.
3. Add clap subcommand + request builder in `rmux-cli/src/commands/<domain>.rs`.

Or call the new method immediately via `rmux-cli call <method> '<json>'`.
