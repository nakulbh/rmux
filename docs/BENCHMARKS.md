# Shell terminal benchmarks

Evaluations **comparing shells** on real terminal tasks — **compilation**, **file search**, and related pipelines — as suggested for showing performance gains on the terminal.

## What is compared

| Dimension | Values |
|---|---|
| **Shells** | `bash`, `zsh` (add more via `SHELLS=…`) |
| **Modes** | non-login (`-c`) and login (`-lc`) |
| **Tasks** | shell startup, cold compile, warm compile, content search, filename search, line-count pipeline |

This is a **shell comparison** (how each shell launches and runs those workloads), not a terminal-emulator FPS benchmark and not a full rmux vs cmux memory suite.

## Workloads

| ID | Task | Command shape |
|---|---|---|
| `shell_startup` | Spawn overhead | `true` |
| `compilation_cold` | Compilation (cold) | `cargo clean` then `cargo build` of tiny crate |
| `compilation_warm` | Compilation (warm/incremental) | `cargo build` of tiny crate |
| `file_search_grep` | Content file search | `grep -R` for a known needle over 2000 files |
| `file_search_find` | Filename discovery | `find … -name '*.txt'` |
| `file_count_lines` | Extra pipeline (“etc”) | `find` + `xargs wc -l` |

Fixtures live under `benches/shell_eval/fixtures/`.

## Run

```bash
./scripts/bench-shells.sh

# More samples / extra shells
RUNS=10 SHELLS="bash zsh sh" ./scripts/bench-shells.sh
```

Requires: `bash`, `python3`, `cargo`, `find`, `grep`. Optional shells are skipped if missing.

## Output

| File | Purpose |
|---|---|
| `benches/shell_eval/results/latest.md` | Human-readable table |
| `benches/shell_eval/results/latest.tsv` | Raw means |
| `benches/shell_eval/results/latest.json` | Machine-readable |

## Interpreting results

- Most of **compilation** time is `rustc`, not the shell. Shell deltas are often small in non-login mode.
- **Login** mode (`-lc`) loads `~/.zshrc` / bash profiles — that can dominate and is closer to “open a terminal and run a task.”
- **File search** differences are usually small once `grep`/`find` are running; startup + rc cost still show up in login mode.
- For host comparisons (rmux vs cmux memory with many panes), use a separate harness — not this script.

## Latest results

See [`benches/shell_eval/results/latest.md`](../benches/shell_eval/results/latest.md) after running the script (generated locally; re-run on your machine for numbers that match your PATH/rc).
