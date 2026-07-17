#!/usr/bin/env bash
# bench-shells.sh — Compare shells on real terminal tasks (compilation, file search).
#
# Matches the evaluation request:
#   "comparing shells to show the performance gains on tasks like
#    compilation or file search etc on the terminal"
#
# Usage:
#   ./scripts/bench-shells.sh
#   RUNS=7 ./scripts/bench-shells.sh
#   SHELLS="bash zsh" ./scripts/bench-shells.sh
#
# Output:
#   benches/shell_eval/results/latest.md
#   benches/shell_eval/results/latest.json
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BENCH_DIR="$ROOT/benches/shell_eval"
FIX_DIR="$BENCH_DIR/fixtures"
TINY_CRATE="$FIX_DIR/tiny-crate"
SEARCH_TREE="$FIX_DIR/search-tree"
RESULTS_DIR="$BENCH_DIR/results"
RUNS="${RUNS:-5}"
WARMUP="${WARMUP:-1}"
# Default shells: common macOS/Linux interactive shells
SHELLS="${SHELLS:-bash zsh}"

mkdir -p "$RESULTS_DIR" "$SEARCH_TREE"

have_cmd() { command -v "$1" >/dev/null 2>&1; }

resolve_shell() {
  local name="$1"
  case "$name" in
    bash) command -v bash ;;
    zsh)  command -v zsh ;;
    fish) command -v fish 2>/dev/null || true ;;
    sh)   command -v sh ;;
    *)    command -v "$name" 2>/dev/null || true ;;
  esac
}

# Wall-clock seconds for a command string run under a shell binary.
# Args: shell_path mode(c|lc) command_string
time_shell_cmd() {
  local shell_bin="$1"
  local mode="$2"
  local cmd="$3"
  python3 - "$shell_bin" "$mode" "$cmd" <<'PY'
import subprocess, sys, time
shell, mode, cmd = sys.argv[1], sys.argv[2], sys.argv[3]
if mode == "lc":
    argv = [shell, "-lc", cmd]
else:
    argv = [shell, "-c", cmd]
start = time.perf_counter()
proc = subprocess.run(argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
elapsed = time.perf_counter() - start
if proc.returncode != 0:
    sys.stderr.write(f"command failed (exit {proc.returncode}): {shell} -{mode} {cmd!r}\n")
    sys.exit(proc.returncode)
print(f"{elapsed:.6f}")
PY
}

stats() {
  # stdin: one float per line → mean std min max
  # Use -c so the program's stdin stays connected to the times file (not a heredoc).
  python3 -c '
import sys, statistics
vals = [float(x) for x in sys.stdin if x.strip()]
if not vals:
    print("0 0 0 0")
    raise SystemExit(0)
mean = statistics.fmean(vals)
std = statistics.stdev(vals) if len(vals) > 1 else 0.0
print(f"{mean:.4f} {std:.4f} {min(vals):.4f} {max(vals):.4f}")
'
}

ensure_search_tree() {
  # Stable tree: 2000 files, ~half contain the needle, nested dirs.
  local marker="$SEARCH_TREE/.rmux_bench_ready"
  if [[ -f "$marker" ]] && [[ "$(cat "$marker")" == "v1-2000" ]]; then
    return 0
  fi
  echo "Generating search-tree fixture (2000 files)…"
  rm -rf "$SEARCH_TREE"
  mkdir -p "$SEARCH_TREE"
  python3 - "$SEARCH_TREE" <<'PY'
import os, sys
root = sys.argv[1]
n = 2000
for i in range(n):
    d = os.path.join(root, f"d{i % 50}", f"s{i % 10}")
    os.makedirs(d, exist_ok=True)
    path = os.path.join(d, f"file_{i}.txt")
    if i % 2 == 0:
        body = f"line one\nrmux_bench_needle_{i}\nline three\n"
    else:
        body = f"noise data {i}\nno match here\n"
    with open(path, "w", encoding="utf-8") as f:
        f.write(body)
with open(os.path.join(root, ".rmux_bench_ready"), "w", encoding="utf-8") as f:
    f.write("v1-2000")
PY
}

ensure_tiny_crate_built_once() {
  # Touch sources so warm builds still do *some* work after first compile.
  (cd "$TINY_CRATE" && cargo build -q 2>/dev/null || cargo build -q)
}

# ── Workloads ──────────────────────────────────────────────────────────────
# Each workload is a shell command string. Same string for every shell.

cmd_shell_startup() {
  echo "true"
}

cmd_compilation_cold() {
  # Full compile path launched by the shell. Caller runs `cargo clean` before each timed run.
  printf 'cd %q && cargo build -q' "$TINY_CRATE"
}

cmd_compilation_warm() {
  # Incremental / mostly-cached build after ensuring target exists.
  printf 'cd %q && cargo build -q' "$TINY_CRATE"
}

cmd_file_search_grep() {
  # Recursive content search (portable; no ripgrep required).
  printf 'grep -R --binary-files=without-match -l "rmux_bench_needle" %q | wc -l | grep -q .' "$SEARCH_TREE"
}

cmd_file_search_find() {
  # Filename walk / extension filter — classic terminal file discovery.
  printf 'find %q -type f -name "*.txt" | wc -l | grep -q .' "$SEARCH_TREE"
}

cmd_file_count_lines() {
  # "etc": another common terminal pipeline (find + wc).
  printf 'find %q -type f -name "*.txt" -print0 | xargs -0 wc -l | tail -1 | grep -q .' "$SEARCH_TREE"
}

run_benchmark() {
  local label="$1"
  local mode="$2"   # c | lc
  local shell_name="$3"
  local shell_bin="$4"
  local cmd="$5"
  local times_file
  times_file="$(mktemp)"

  # Warmup
  local i
  for ((i = 0; i < WARMUP; i++)); do
    time_shell_cmd "$shell_bin" "$mode" "$cmd" >/dev/null || true
  done

  for ((i = 0; i < RUNS; i++)); do
    # Cold compile needs a clean every run (warmup already dirtied target once).
    if [[ "$label" == "compilation_cold" ]]; then
      (cd "$TINY_CRATE" && cargo clean -q) || true
    fi
    time_shell_cmd "$shell_bin" "$mode" "$cmd" >>"$times_file"
  done

  local mean std minv maxv
  read -r mean std minv maxv < <(stats <"$times_file")
  rm -f "$times_file"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$label" "$mode" "$shell_name" "$mean" "$std" "$minv" "$maxv"
}

main() {
  echo "=== rmux shell terminal benchmarks ==="
  echo "root:    $ROOT"
  echo "runs:    $RUNS (warmup $WARMUP)"
  echo "shells:  $SHELLS"
  echo

  ensure_search_tree
  ensure_tiny_crate_built_once

  local -a resolved_names=()
  local -a resolved_bins=()
  local s bin
  for s in $SHELLS; do
    bin="$(resolve_shell "$s" || true)"
    if [[ -z "$bin" ]]; then
      echo "skip: shell not found: $s" >&2
      continue
    fi
    resolved_names+=("$s")
    resolved_bins+=("$bin")
    echo "using $s → $bin ($("$bin" --version 2>/dev/null | head -1 || echo version unknown))"
  done
  if [[ ${#resolved_names[@]} -lt 1 ]]; then
    echo "No shells available to benchmark." >&2
    exit 1
  fi
  echo

  local tsv="$RESULTS_DIR/latest.tsv"
  local md="$RESULTS_DIR/latest.md"
  local json="$RESULTS_DIR/latest.json"
  printf 'workload\tmode\tshell\tmean_s\tstd_s\tmin_s\tmax_s\n' >"$tsv"

  # Workload list: label → command builder function name
  local -a labels=(
    shell_startup
    compilation_cold
    compilation_warm
    file_search_grep
    file_search_find
    file_count_lines
  )

  local label mode idx name bin cmd
  for label in "${labels[@]}"; do
    cmd="$("cmd_${label}")"
    for mode in c lc; do
      echo "── $label  (shell -${mode}) ──"
      for idx in "${!resolved_names[@]}"; do
        name="${resolved_names[$idx]}"
        bin="${resolved_bins[$idx]}"
        echo -n "  $name … "
        # fish uses different -c semantics for login; skip -lc if unsupported later
        if ! line="$(run_benchmark "$label" "$mode" "$name" "$bin" "$cmd")"; then
          echo "FAILED"
          continue
        fi
        echo "$line" | tee -a "$tsv" | awk -F'\t' '{printf "mean %ss (±%s)\n", $4, $5}'
      done
    done
  done

  # Markdown report
  {
    echo "# Shell terminal benchmarks"
    echo
    echo "Generated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    echo
    echo "Compares shells on terminal tasks: **compilation**, **file search**, and related"
    echo "pipelines — the evaluation suggested for showing shell performance gains."
    echo
    echo "## Environment"
    echo
    echo "| Item | Value |"
    echo "|---|---|"
    echo "| Host | $(uname -s) $(uname -m) |"
    echo "| Runs per cell | $RUNS (warmup $WARMUP) |"
    echo "| Compile fixture | \`benches/shell_eval/fixtures/tiny-crate\` |"
    echo "| Search fixture | 2000 \`.txt\` files under \`fixtures/search-tree\` |"
    echo "| Modes | \`-c\` non-login · \`-lc\` login (loads shell rc / PATH) |"
    echo
    echo "### Shells"
    echo
    echo "| Shell | Path | Version |"
    echo "|---|---|---|"
    for idx in "${!resolved_names[@]}"; do
      name="${resolved_names[$idx]}"
      bin="${resolved_bins[$idx]}"
      ver="$("$bin" --version 2>/dev/null | head -1 | tr '|' '/' || echo n/a)"
      echo "| $name | \`$bin\` | $ver |"
    done
    echo
    echo "## Results (mean wall seconds)"
    echo
    echo "| Workload | Mode | Shell | Mean (s) | Stdev | Min | Max |"
    echo "|---|---|---|---:|---:|---:|---:|"
    tail -n +2 "$tsv" | while IFS=$'\t' read -r w m sh mean std minv maxv; do
      mode_label="$m"
      [[ "$m" == "c" ]] && mode_label="non-login (-c)"
      [[ "$m" == "lc" ]] && mode_label="login (-lc)"
      echo "| \`$w\` | $mode_label | $sh | $mean | $std | $minv | $maxv |"
    done
    echo
    echo "## How to read this"
    echo
    echo "- **compilation_***: real \`cargo build\` of a tiny crate (cold = clean+build, warm = incremental)."
    echo "- **file_search_grep**: recursive content search (\`grep -R\`) for a known needle."
    echo "- **file_search_find**: filename walk (\`find … -name\`)."
    echo "- **file_count_lines**: find + \`wc -l\` pipeline (extra “etc” terminal task)."
    echo "- **shell_startup**: \`true\` only — pure shell spawn overhead."
    echo "- Login mode (\`-lc\`) includes rc/profile cost; that often dominates shell differences"
    echo "  more than the workload itself once \`rustc\` / \`grep\` are running."
    echo
    echo "## Reproduce"
    echo
    echo '```bash'
    echo './scripts/bench-shells.sh'
    echo 'RUNS=10 SHELLS="bash zsh" ./scripts/bench-shells.sh'
    echo '```'
    echo
  } >"$md"

  # JSON summary
  python3 - "$tsv" "$json" <<'PY'
import json, sys, pathlib
tsv, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
rows = []
lines = tsv.read_text(encoding="utf-8").splitlines()
for line in lines[1:]:
    if not line.strip():
        continue
    w, m, sh, mean, std, mn, mx = line.split("\t")
    rows.append({
        "workload": w,
        "mode": m,
        "shell": sh,
        "mean_s": float(mean),
        "std_s": float(std),
        "min_s": float(mn),
        "max_s": float(mx),
    })
out.write_text(json.dumps({"results": rows}, indent=2) + "\n", encoding="utf-8")
PY

  echo
  echo "Wrote:"
  echo "  $md"
  echo "  $json"
  echo "  $tsv"
  echo
  echo "──── preview ────"
  cat "$md"
}

main "$@"
