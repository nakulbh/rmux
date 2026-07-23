# Browser Engine Strategy — Chromium Cross-Platform

> **Full implementation plan:** [`CHROMIUM_BROWSER_PLAN.md`](CHROMIUM_BROWSER_PLAN.md)  
> (phases E0–E4, tasks, acceptance criteria, packaging, QA)

## Goal

A **single Chromium engine** for the in-app browser on **macOS, Windows, and Linux**, so layout, JS, DevTools, cookies, and automation behave the same everywhere (cmux-class agent browser).

## Why not “just wry”?

| Platform | `wry` backend | Engine |
|---|---|---|
| macOS | WKWebView | **WebKit** (not Chromium) |
| Windows | WebView2 | Chromium (Edge) |
| Linux | webkit2gtk | **WebKit** |

That is fine for a lightweight “show a page” pane, but it is **not** one browser. Agents and users hit WebKit vs Chromium differences (CSS, JS APIs, extensions, automation quirks).

## Target architecture

```
┌─────────────────────────────────────────────────────┐
│  BrowserPane (UI: chrome, URL bar, tabs)            │
│         │                                           │
│         ▼                                           │
│  BrowserEngine trait                                │
│    • navigate / back / forward / reload             │
│    • eval / automation hooks                        │
│    • set_bounds / set_visible                       │
│    • paint path: native child OR egui texture       │
└─────────┬───────────────────────────┬───────────────┘
          │                           │
   ┌──────▼──────┐             ┌──────▼──────────────┐
   │ OsWebview   │  (default)  │ Chromium (CEF OSR)  │  (goal)
   │ wry         │             │ wew / wef / libcef  │
   └─────────────┘             └─────────────────────┘
```

### Chromium path (chosen): CEF off-screen rendering (OSR)

1. **CEF** (Chromium Embedded Framework) — real Chromium on all three OSes.
2. **Off-screen rendering** — CEF paints into a pixel buffer / shared texture each frame.
3. **egui texture** — upload buffer → `egui::ColorImage` / `TextureHandle` and draw in the pane (no native z-order fight with the URL bar).
4. **Input** — forward mouse/keyboard from egui into CEF.

Rust options evaluated:

| Crate | Notes |
|---|---|
| [`wew`](https://crates.io/crates/wew) | CEF wrapper, OSR + native window modes, winit-oriented |
| [`wef`](https://github.com/longbridge/wef) | CEF3 + OSR |
| raw CEF / `cef-rs` | Maximum control, maximum work |

**Not chosen for the pane:** driving system Chrome via CDP only (separate process/window unless we stream screenshots — poor UX).

## Packaging reality

Shipping Chromium means:

- CEF binary blobs per platform (~100–200+ MB in release artifacts)
- Multi-process helper (`rmux` + CEF subprocesses)
- Cache dir under the user config path
- CI matrix that downloads CEF or uses a prebuilt cache
- License attribution (BSD/Chromium)

Default builds stay on **OS webview** until the Chromium pipeline is green on all three platforms.

## Cargo features

```toml
# crates/rmux-app/Cargo.toml
[features]
default = ["browser-chromium"]     # CEF OSR (default)
browser-chromium = ["dep:cef"]
browser-os-webview = ["dep:wry"]   # optional light fallback
```

Default build uses Chromium:

```bash
./scripts/fetch-cef.sh
eval "$(./scripts/fetch-cef.sh --print-env)"
cargo run -p rmux-app
```

Optional OS webview (no CEF binaries):

```bash
cargo run -p rmux-app --no-default-features --features browser-os-webview
```

## Implementation phases

| Phase | Deliverable |
|---|---|
| **E0** | ✅ Engine backend scaffold; OS + Chromium modules; features |
| **E1** | ✅ CEF Runtime (`cef` crate), external message pump, subprocess gate, `fetch-cef` |
| **E2** | ✅ OSR `on_paint` → RGBA → egui texture; navigate/reload; mouse/key input; console-bridge `eval` |
| **E3** | 🟡 Profile dir wired; cookies/session/screenshot/DevTools still open |
| **E4** | 🟡 Chromium is default; CI packaging / Helper.app bundles remaining |

**Chosen CEF binding (E1.1):** [tauri-apps/cef-rs](https://github.com/tauri-apps/cef-rs) (`cef` crate) — multi-arch, maintained.

### Run (default = Chromium)

```bash
# 1) Tools: cmake + ninja (cef-dll-sys builds libcef_dll_wrapper)
# 2) CEF binaries:
./scripts/fetch-cef.sh
export CEF_PATH="$(pwd)/third_party/cef/current/150.0.14/cef_macos_aarch64"  # adjust for platform
export DYLD_FALLBACK_LIBRARY_PATH="${DYLD_FALLBACK_LIBRARY_PATH:-}:$CEF_PATH:$CEF_PATH/Chromium Embedded Framework.framework/Libraries"

# 3) Build/run (Chromium is default — no feature flags required)
cargo run -p rmux-app
```

Then open a browser pane (`Cmd/Ctrl+Shift+L` or globe icon). Content paints **below** the egui URL bar (OSR — no z-order fight).

## Decision record

- **Wanted:** Chromium everywhere for parity with agent-browser / cmux-class automation.
- **Default now:** Chromium CEF OSR (`browser-chromium`).
- **Optional:** `browser-os-webview` for builds without CEF.
