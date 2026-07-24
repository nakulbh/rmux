# Plan: Cross-Platform Chromium Browser in rmux

| | |
|---|---|
| **Status** | Draft — ready to execute |
| **Branch** | `feat/browser-pane` (worktree: `rmux-feat-browser-pane`) |
| **Owner** | rmux maintainers |
| **Related** | [`BROWSER_ENGINE.md`](BROWSER_ENGINE.md), [`PLAN.md`](PLAN.md) § Phase 4.6 |
| **Last updated** | 2026-07-23 (E1 in progress) |

---

## 1. Problem statement

rmux embeds a browser pane inspired by [cmux](https://github.com/manaflow-ai/cmux). Today that pane uses **`wry`**, which picks a **different engine per OS**:

| Platform | Backend | Engine |
|---|---|---|
| macOS | WKWebView | **WebKit** |
| Windows | WebView2 | **Chromium** (Edge) |
| Linux | webkit2gtk | **WebKit** |

### Why this is not good enough

- **Inconsistent rendering** — CSS/JS/layout differ WebKit vs Chromium.
- **Inconsistent automation** — agents (`browser.eval`, snapshot, click/fill) hit different bugs and APIs.
- **Native child z-order** — OS webviews paint *above* egui, so URL bar / chrome can be covered (already hit on macOS Retina).
- **cmux-class goal** — agents should drive a predictable browser next to the terminal on every platform.

### Desired outcome

One **Chromium** engine on **macOS, Windows, and Linux**, painted **inside** the pane (below the address bar), with the same navigation + automation surface we already expose.

---

## 2. Goals and non-goals

### Goals

1. **Same engine everywhere:** Chromium (CEF) on macOS / Windows / Linux.
2. **Pane-native UX:** tab chrome + URL bar stay in **egui**; page content is Chromium.
3. **No URL-bar occlusion:** content never paints over chrome (prefer **off-screen rendering** into an egui texture).
4. **Keep existing API:** socket methods (`browser.*`) and CLI keep working; backend is swappable.
5. **Safe migration:** default builds stay on OS webview until Chromium is green on all three OSes.
6. **Ship path:** documented CEF download, local dev, CI cache, license attribution.

### Non-goals (this plan)

- Full Chrome extension store / Google account sync.
- Replacing the terminal emulator with a browser.
- Embedding Electron or spawning a separate Chrome window as the “pane”.
- Perfect pixel parity with Google Chrome stable (CEF version may lag slightly).

---

## 3. Current state (baseline)

### Already done (as of this plan)

| Item | Location |
|---|---|
| Browser pane + split / zoom / close | `workspace/splits.rs`, `ui/workspace_view.rs` |
| Globe button on terminal chrome | `ui/workspace_view.rs`, `ui/icons.rs` |
| “New tab” browser chrome + URL bar | `browser/webview.rs`, `workspace_view` |
| Navigation + history | `BrowserPane` |
| Automation API + CLI | `rmux-api` methods, `api_dispatch`, `rmux-cli` |
| Engine feature flags | `crates/rmux-app/Cargo.toml` (`browser-os-webview`, `browser-chromium`) |
| `EngineBackend` enum + OS / Chromium modules | `browser/engine.rs`, `os_webview.rs`, `chromium.rs` (stub) |
| Strategy notes | `docs/BROWSER_ENGINE.md` |

### Gaps

- Chromium backend still fails at attach until CEF is linked (E1.6–E1.7).
- ~~Pane still uses inline wry~~ — **migrated** to `EngineBackend` (OS path live).
- CEF download script exists; binary cache not yet in CI.
- No OSR → egui texture path (E2).
- Session persistence (Phase 4.4) still open (should target Chromium profile when ready).

---

## 4. Architecture

### 4.1 Target data flow

```
User types URL in egui address bar
        │
        ▼
BrowserPane (history, title, focus_url_bar)
        │
        ▼
EngineBackend::Chromium  ──navigate / eval / input──►  CEF Browser (OSR)
        │                                                      │
        │                                                      │ paint buffer (RGBA)
        ▼                                                      ▼
egui: draw tab chrome + URL bar          egui TextureHandle (page content)
        │                                                      │
        └────────────────── single pane rect ──────────────────┘
```

### 4.2 Why CEF OSR (not native child Chromium window)

| Approach | Pros | Cons |
|---|---|---|
| **CEF OSR → egui texture** (chosen) | Same z-order as UI; URL bar always clickable; one paint path | More glue (frame upload, input forwarding) |
| CEF native child window | Closer to wry today | Same z-order bugs as WKWebView |
| System Chrome + CDP | Real Chrome | Separate process/window; screenshots = bad UX |
| Keep wry only | Small binary | Engine skew forever |

### 4.3 Module layout (target)

```
crates/rmux-app/src/browser/
├── mod.rs
├── engine.rs          # EngineKind, EngineBackend enum
├── webview.rs         # BrowserPane UI + history (engine-agnostic)
├── automation.rs      # JS helpers for click/fill/snapshot
├── os_webview.rs      # wry backend (optional fallback)
└── chromium/
    ├── mod.rs         # ChromiumEngine
    ├── runtime.rs     # CEF / wew runtime lifecycle
    ├── osr.rs         # frame buffer → egui texture
    └── input.rs       # mouse/keyboard → CEF
```

### 4.4 Process model

```
rmux (main process)
  ├── eframe / egui UI thread
  ├── CEF message pump (main or integrated pump)
  └── CEF subprocesses (GPU / renderer / utility)
        launched via CEF helper binary or same binary with --type=
```

Requirements:

- Detect CEF subprocess entry early in `main` (before eframe).
- User data / cache under platform config dir, e.g.  
  `~/Library/Application Support/rmux/chromium/` (macOS),  
  `%APPDATA%\rmux\chromium\` (Windows),  
  `~/.config/rmux/chromium/` (Linux).

---

## 5. Implementation phases

### Phase E0 — Scaffold ✅ DONE

**Goal:** Feature flags and backend swap point without breaking default builds.

- [x] `browser-os-webview` / `browser-chromium` features  
- [x] `EngineKind` + `EngineBackend`  
- [x] Chromium stub module  
- [x] Strategy doc  

**Exit criteria:** `cargo test -p rmux-app` green with default features.

---

### Phase E1 — CEF toolchain and runtime bootstrap

**Goal:** Developers can download CEF and create a Chromium runtime on at least one platform (prefer **macOS** first, then Linux, then Windows).

#### Tasks

| ID | Task | Notes |
|---|---|---|
| E1.1 | Choose Rust CEF crate (`wew` vs `wef` vs raw CEF) | **Done (ADR):** [tauri-apps/cef-rs](https://github.com/tauri-apps/cef-rs) (`cef` crate) as official multi-arch binding; wew/wef optional later |
| E1.2 | Script `scripts/fetch-cef.sh` (or `.ps1`) | **Done** — `scripts/fetch-cef.sh` + `fetch-cef.ps1` → `third_party/cef/` |
| E1.3 | `.gitignore` third_party CEF blobs | **Done** |
| E1.4 | Document cache path + disk size (~1–2 GB unpacked) | **Done** — profile `~/.config/rmux/chromium`; CEF via `CEF_PATH` / fetch script |
| E1.5 | CEF subprocess entry in `main.rs` | **Done** (stub detect + exit); full helper when `cef` linked |
| E1.6 | Create Runtime on UI thread before/with eframe | **Done** — `cef` dep, `ensure_runtime`, pump in `RmuxApp::update` |
| E1.7 | `ChromiumEngine::ensure_attached` creates a real browser | **Done** — windowless OSR via `browser_host_create_browser_sync` |
| E1.8 | Migrate `BrowserPane` onto `EngineBackend` | **Done** — all navigate/attach/eval via engine enum |

#### Acceptance criteria

- [ ] On macOS: `cargo run -p rmux-app --no-default-features --features browser-chromium` starts without stub error after CEF is fetched.
- [ ] Runtime initializes; process tree shows CEF helpers.
- [ ] Documented one-command setup: `./scripts/fetch-cef.sh && cargo run --features browser-chromium`.

#### Risks

| Risk | Mitigation |
|---|---|
| CEF + eframe fight over main loop | Prefer CEF **external message pump** driven from `App::update` |
| Helper binary packaging on macOS | Use CEF’s app bundle layout early |
| License / redistribution | Add `THIRD_PARTY_NOTICES` for Chromium |

**Estimated effort:** 1–2 weeks (one engineer, one primary OS first).

---

### Phase E2 — OSR paint + input (usable Chromium pane)

**Goal:** Chromium page pixels appear **inside** the browser content rect; mouse/keyboard work; URL bar never covered.

#### Tasks

| ID | Task | Notes |
|---|---|---|
| E2.1 | Enable CEF windowless (OSR) mode | **Done** — `windowless_rendering_enabled` |
| E2.2 | OnPaint → RGBA buffer (double-buffer) | **Done** — BGRA→RGBA into `FrameBuffer` |
| E2.3 | Upload to `egui::TextureHandle` each frame (dirty only) | **Done** — `update_osr_texture` |
| E2.4 | Draw texture in content rect only | **Done** — below URL toolbar |
| E2.5 | Forward pointer events (move, down, up, scroll) | **Done** — `feed_osr_input` |
| E2.6 | Forward keys + IME when address bar not focused | **Done** — text events; IME polish remaining |
| E2.7 | `navigate` / `back` / `forward` / `reload` via CEF | **Done** — load_url / reload; pane history for back/fwd |
| E2.8 | Title + URL change callbacks → `BrowserPane` state | **Done** — DisplayHandler / LoadHandler channels |
| E2.9 | Hide/destroy OSR browser when pane not shown | **Done** — `was_hidden` / `close_browser` |
| E2.10 | Migrate `BrowserPane` fully onto `EngineBackend` | **Done** |

#### Acceptance criteria

- [ ] Globe → browser pane shows Chromium content under URL bar.
- [ ] Address bar clickable and typed URLs load.
- [ ] Resize split updates CEF view size without black bars / stretch bugs.
- [ ] Switching workspace hides Chromium content (no overlay on terminals).
- [ ] Automation `browser.eval` works against Chromium.

#### Risks

| Risk | Mitigation |
|---|---|
| Frame upload CPU cost | Dirty rects; throttle to display refresh; consider GPU texture path later |
| HiDPI wrong scale | Always use logical size for CEF, physical for texture pixels |
| Input focus races with PTY | Reuse existing `text_sink` + active pane rules |

**Estimated effort:** 2–3 weeks after E1 on first OS; +1 week per additional OS.

---

### Phase E3 — Product parity on Chromium

**Goal:** Chromium path matches (and exceeds) current OS webview features.

#### Tasks

| ID | Task | Notes |
|---|---|---|
| E3.1 | Persistent profile / cookies | **Done** — CEF profile dir + `persist_session_cookies` |
| E3.2 | Save/restore last URL + history per pane | **Done** — `browser/session.rs` on exit/startup |
| E3.3 | Screenshot via CEF bitmap API | **Done** — OSR last-frame PNG + `browser.screenshot` |
| E3.4 | DevTools toggle (debug builds) | Optional shortcut |
| E3.5 | Download handling (optional) | Policy: deny or download folder |
| E3.6 | Popup / new window policy | Open as new browser pane or block |
| E3.7 | Performance check | Cap DPR + buffer reuse shipped; formal budget TBD |

#### Acceptance criteria

- [x] Restart restores browser URL (with 4.4).
- [x] `browser.screenshot` returns PNG bytes or file path.
- [ ] Memory target from PLAN still documented (adjust if CEF requires higher baseline).

**Estimated effort:** 1–2 weeks.

---

### Phase E4 — Packaging, CI, default flip

**Goal:** Chromium is reliable enough to become the default browser engine.

#### Tasks

| ID | Task | Notes |
|---|---|---|
| E4.1 | CI: cache CEF per OS matrix | GitHub Actions cache key = CEF version + target |
| E4.2 | Release artifacts: include CEF runtime | Or download-on-first-run with checksum |
| E4.3 | macOS codesign / notarization of helpers | Required for distribution |
| E4.4 | Windows: WebView2 no longer required for browser | Terminal still independent |
| E4.5 | Linux: document glibc / GPU deps | CI on Ubuntu LTS |
| E4.6 | Flip default feature to `browser-chromium` | **Done** — OS webview is optional fallback |
| E4.7 | Update docs: guide, API, KEY_BINDINGS, AGENTS.md | Engine section |
| E4.8 | Deprecate / optional-compile wry browser path | Feature-gate wry if unused |

#### Acceptance criteria

- [ ] CI green on macOS + Windows + Linux with Chromium.
- [ ] Fresh user: install → open browser pane → load https://example.com works without manual CEF setup (download-on-first-run **or** bundled).
- [ ] Fallback feature still builds for minimal embeds.

**Estimated effort:** 2–3 weeks (CI + packaging heavy).

---

## 6. Work sequence (recommended)

```text
E0 scaffold ✅
    │
    ▼
E1.1 spike (wew/wef hello OSR)  ──► decide crate
    │
    ▼
E1.2–E1.7 macOS runtime bootstrap
    │
    ▼
E2 OSR + input on macOS  ──► dogfood daily
    │
    ▼
E2 port Linux ──► E2 port Windows
    │
    ▼
E3 product features (cookies, screenshot, session)
    │
    ▼
E4 CI + packaging + default flip
```

Do **not** flip the default engine before E2 works on all three OSes.

---

## 7. Task checklist (copy into issues)

### E1

- [ ] Spike CEF crate choice; write ADR note in this file §10  
- [ ] `scripts/fetch-cef.sh` + Windows script  
- [ ] Subprocess early-exit in `main`  
- [ ] Runtime init with eframe  
- [ ] Non-stub `ChromiumEngine::ensure_attached` on macOS  

### E2

- [ ] OSR paint buffer  
- [ ] egui texture draw  
- [ ] Pointer + keyboard forwarding  
- [ ] Navigate / history / title / URL sync  
- [ ] Visibility on workspace switch  
- [ ] Full `EngineBackend` migration from inline wry  

### E3

- [ ] Profile / cookies  
- [ ] Session restore hooks (4.4)  
- [ ] Screenshot API  
- [ ] Memory / perf note  

### E4

- [ ] CI CEF cache  
- [ ] Release packaging  
- [ ] Default feature flip  
- [ ] Docs update  

---

## 8. Dependencies and packaging

### New / planned dependencies

| Purpose | Candidate | When |
|---|---|---|
| CEF Rust bindings | `wew` or `wef` (decide in E1.1) | E1 |
| Existing | `wry` | Keep until E4 optional |
| Existing | `eframe` / `egui` | Texture upload |

### Binary layout (illustrative)

```
rmux.app/Contents/          # macOS
  MacOS/rmux
  Frameworks/Chromium Embedded Framework.framework
  Helpers/...

rmux/                       # Linux tarball
  rmux
  libcef.so
  locales/
  ...

rmux/                       # Windows
  rmux.exe
  libcef.dll
  ...
```

### Config / data

| Path purpose | Example (macOS) |
|---|---|
| CEF cache / cookies | `~/Library/Application Support/rmux/chromium/` |
| Optional CEF download | `~/Library/Caches/rmux/cef/<version>/` |

---

## 9. Testing strategy

| Layer | What | How |
|---|---|---|
| Unit | URL normalize, history, engine kind | Existing `cargo test` |
| Integration | navigate + title callback | Headless CEF if possible; else `#[ignore]` manual |
| UI manual | Globe → type URL → load | Checklist below |
| CI | Build default + `browser-chromium` | Matrix 3 OS |
| Perf | RSS with 1 browser pane | Compare to PLAN memory table |

### Manual QA checklist (per OS)

1. Open app; Cmd+D split; click **globe**.  
2. “New tab” chip + **address bar** visible; type `example.com`, Enter.  
3. Page loads; tab title updates.  
4. Back / forward / reload.  
5. Cmd+L focuses address bar.  
6. Switch workspace — browser not covering other UI.  
7. Close browser pane — no leak / crash.  
8. `rmux-cli browser-eval 'document.title'`.  
9. (E3) Quit and relaunch — URL restored.  

---

## 10. Decisions log

| Date | Decision | Rationale |
|---|---|---|
| 2026-07-23 | Target **CEF OSR → egui texture** | Cross-platform Chromium + no z-order fight with URL bar |
| 2026-07-23 | Keep **wry as default** until E4 | Superseded — Chromium is default |
| 2026-07-23 | Flip default to **browser-chromium** | OS webview remains optional fallback |
| 2026-07-23 | Reject CDP-only system Chrome for pane | Not embedded; poor agent UX |
| 2026-07-23 | **E1.1 ADR:** use **tauri-apps/cef-rs** (`cef`) | Official multi-platform (x64+ARM), maintained with Tauri ecosystem; `export-cef-dir` for shared binaries; wew/wef remain optional higher-level wrappers after Runtime boots |
| 2026-07-23 | Route all pane attach/nav through **`EngineBackend`** | One swap point for OS → Chromium without rewriting chrome/automation |

---

## 11. Success metrics

| Metric | Target |
|---|---|
| Engine parity | Same Chromium major on macOS / Win / Linux |
| UX | URL bar always interactive when browser focused |
| API | All `browser.*` methods work on Chromium backend |
| Default build | Chromium after E4; OS webview optional |
| Regression | Existing terminal + split tests remain green |

---

## 12. Open questions

1. **Bundle vs download-on-first-run** for CEF (~100–200MB)?  
   - Bundle: simpler offline UX, large GitHub Releases.  
   - Download: smaller installer, needs network + checksums.  
2. **GPU process** required on all CI runners?  
3. **Minimum OS versions** for chosen CEF build?  
4. Keep OS webview forever as `minimal` feature for embedded/low-RAM?

---

## 13. References

- rmux strategy: [`docs/BROWSER_ENGINE.md`](BROWSER_ENGINE.md)  
- Master plan Phase 4: [`docs/PLAN.md`](PLAN.md)  
- CEF builds: https://cef-builds.spotifycdn.com/  
- `wew` (CEF Rust): https://crates.io/crates/wew  
- `wef` (CEF3 OSR): https://github.com/longbridge/wef  
- cmux (reference UX; WKWebView on Apple): https://github.com/manaflow-ai/cmux  

---

## 14. Next action

**E1.6–E1.7 (Runtime):**

1. Run `./scripts/fetch-cef.sh` (or fix `CEF_VERSION` against Spotify CDN).  
2. Add optional Cargo dependency on `cef` (tauri-apps/cef-rs) behind `browser-chromium`.  
3. Boot CEF Runtime with **external message pump** driven from `RmuxApp::update`.  
4. Create windowless browser in `ChromiumEngine::ensure_attached`.  
5. Proceed to **E2** (OnPaint → egui texture).  

**PR suggestion:** `feat/chromium-e1-runtime` from worktree `rmux-feat-browser-pane`.
