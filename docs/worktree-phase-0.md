# Phase 0: Foundation — Worktree Checklist

> Track progress through all Phase 0 tasks.

| # | Task | Status |
|---|------|--------|
| 0.1 | Replace GPUI with `eframe` + `egui` in root Cargo.toml | [x] |
| 0.2 | Add dependencies: `alacritty_terminal`, `portable-pty`, `tokio`, `serde`, `serde_json` | [x] |
| 0.3 | Create workspace crate structure (rmux-app, rmux-terminal, rmux-cli, rmux-api, rmux-config) | [x] |
| 0.4 | Build single `egui` window with placeholder terminal grid | [x] |
| 0.5 | Add `tracing` + `tracing-subscriber` for structured logging | [x] |
| 0.6 | Add `clap` for CLI argument parsing in `rmux-app` | [x] |
| 0.7 | Add `rustfmt.toml` and `clippy.toml` config files | [x] |
| 0.8 | Add `justfile` with common tasks (fmt, lint, test, check, doc) | [x] |
