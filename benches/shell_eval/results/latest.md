# Shell terminal benchmarks

Generated: 2026-07-17T15:55:41Z

Compares shells on terminal tasks: **compilation**, **file search**, and related
pipelines — the evaluation suggested for showing shell performance gains.

## Environment

| Item | Value |
|---|---|
| Host | Darwin arm64 |
| Runs per cell | 3 (warmup 1) |
| Compile fixture | `benches/shell_eval/fixtures/tiny-crate` |
| Search fixture | 2000 `.txt` files under `fixtures/search-tree` |
| Modes | `-c` non-login · `-lc` login (loads shell rc / PATH) |

### Shells

| Shell | Path | Version |
|---|---|---|
| bash | `/bin/bash` | GNU bash, version 3.2.57(1)-release (arm64-apple-darwin25) |
| zsh | `/bin/zsh` | zsh 5.9 (arm64-apple-darwin25.0) |

## Results (mean wall seconds)

| Workload | Mode | Shell | Mean (s) | Stdev | Min | Max |
|---|---|---|---:|---:|---:|---:|
| `shell_startup` | non-login (-c) | bash | 0.0024 | 0.0001 | 0.0022 | 0.0025 |
| `shell_startup` | non-login (-c) | zsh | 0.0036 | 0.0001 | 0.0035 | 0.0037 |
| `shell_startup` | login (-lc) | bash | 0.0098 | 0.0002 | 0.0095 | 0.0100 |
| `shell_startup` | login (-lc) | zsh | 0.0232 | 0.0002 | 0.0230 | 0.0235 |
| `compilation_cold` | non-login (-c) | bash | 0.1084 | 0.0036 | 0.1048 | 0.1120 |
| `compilation_cold` | non-login (-c) | zsh | 0.1062 | 0.0013 | 0.1051 | 0.1076 |
| `compilation_cold` | login (-lc) | bash | 0.1131 | 0.0031 | 0.1099 | 0.1161 |
| `compilation_cold` | login (-lc) | zsh | 0.1247 | 0.0030 | 0.1226 | 0.1282 |
| `compilation_warm` | non-login (-c) | bash | 0.0178 | 0.0010 | 0.0172 | 0.0190 |
| `compilation_warm` | non-login (-c) | zsh | 0.0174 | 0.0004 | 0.0170 | 0.0177 |
| `compilation_warm` | login (-lc) | bash | 0.0245 | 0.0010 | 0.0236 | 0.0255 |
| `compilation_warm` | login (-lc) | zsh | 0.0374 | 0.0013 | 0.0366 | 0.0389 |
| `file_search_grep` | non-login (-c) | bash | 0.0504 | 0.0010 | 0.0496 | 0.0515 |
| `file_search_grep` | non-login (-c) | zsh | 0.0514 | 0.0006 | 0.0507 | 0.0518 |
| `file_search_grep` | login (-lc) | bash | 0.0589 | 0.0021 | 0.0572 | 0.0613 |
| `file_search_grep` | login (-lc) | zsh | 0.0704 | 0.0007 | 0.0699 | 0.0711 |
| `file_search_find` | non-login (-c) | bash | 0.0094 | 0.0001 | 0.0093 | 0.0095 |
| `file_search_find` | non-login (-c) | zsh | 0.0105 | 0.0003 | 0.0102 | 0.0109 |
| `file_search_find` | login (-lc) | bash | 0.0165 | 0.0001 | 0.0164 | 0.0166 |
| `file_search_find` | login (-lc) | zsh | 0.0298 | 0.0014 | 0.0288 | 0.0314 |
| `file_count_lines` | non-login (-c) | bash | 0.0386 | 0.0011 | 0.0379 | 0.0398 |
| `file_count_lines` | non-login (-c) | zsh | 0.0401 | 0.0007 | 0.0395 | 0.0408 |
| `file_count_lines` | login (-lc) | bash | 0.0454 | 0.0010 | 0.0447 | 0.0466 |
| `file_count_lines` | login (-lc) | zsh | 0.0575 | 0.0007 | 0.0569 | 0.0582 |

## How to read this

- **compilation_***: real `cargo build` of a tiny crate (cold = clean+build, warm = incremental).
- **file_search_grep**: recursive content search (`grep -R`) for a known needle.
- **file_search_find**: filename walk (`find … -name`).
- **file_count_lines**: find + `wc -l` pipeline (extra “etc” terminal task).
- **shell_startup**: `true` only — pure shell spawn overhead.
- Login mode (`-lc`) includes rc/profile cost; that often dominates shell differences
  more than the workload itself once `rustc` / `grep` are running.

## Reproduce

```bash
./scripts/bench-shells.sh
RUNS=10 SHELLS="bash zsh" ./scripts/bench-shells.sh
```

