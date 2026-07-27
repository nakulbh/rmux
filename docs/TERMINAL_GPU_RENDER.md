# Terminal GPU rendering — plan & fallbacks

> **Status:** Active on `feat/terminal-damage-redraw`  
> **G0:** eframe wgpu + pane paint callback  
> **G1/G2:** glyph atlas + full-grid path — **ON by default** after glyph UV fix  
>   (escape hatch: `RMUX_GPU_GRID=0` forces egui CPU paint)  
> **Bug fixed:** vertex shader set glyph UV to `-1` at cell corners (outside the  
>   ink sub-rect), so interpolation never sampled the atlas — backgrounds only.  
> **Next:** G3 damage uploads  
> **Goal:** Kill felt keyboard / LazyVim `j`/`k` lag by drawing terminal cells on the GPU, not via thousands of egui galleys per frame.

If this approach fails, use the **fallback ladder** at the bottom — do not throw away VT work.

---

## Context (what we already know)

### What rmux uses today

| Layer | Tech | Role |
|---|---|---|
| VT / grid | `alacritty_terminal` | Parse PTY bytes, grid, scrollback, selection |
| Paint | custom `TerminalRenderer` + **egui** | Full-grid snapshot + 2-pass bg/glyphs every frame |
| Window | **eframe** (historically glow; GPU path switches to **wgpu**) | UI shell, multipane chrome |

### What we do **not** use

1. **No Alacritty OpenGL terminal renderer** — that lives in the Alacritty *app*, not in `alacritty_terminal`.
2. **No Alacritty damage-only redraw** — we copy the whole grid every frame.
3. **No Alacritty GPU glyph atlas** — we use egui galleys (CPU text layout).

`alacritty_terminal` is a **library for terminal state**. GPU acceleration is **our** job.

### Latency already fixed (PR #38)

- Blocking PTY read + wake UI on data  
- `request_repaint` after key / paste / output  
- Skip slow title probes while focused  

Remaining mush is largely **paint cost**: full `snapshot()` + full cell walks + many egui shapes every frame.

---

## Target architecture

```text
PTY → alacritty_terminal (CPU) → cell buffer + damage (CPU)
    → glyph atlas + instanced quads (GPU)
    → composite into pane rect (egui PaintCallback / wgpu)
    → egui still owns sidebar, tabs, splits, overlays
```

```text
┌──────────────────────────────────────────┐
│ egui: sidebar, tabs, split chrome        │
│   ┌────────────────────────────────────┐ │
│   │ Terminal pane rect                 │ │
│   │   wgpu callback (atlas + quads)    │ │  ← GPU cell renderer
│   └────────────────────────────────────┘ │
└──────────────────────────────────────────┘
```

### Keep on CPU

- `portable-pty` I/O  
- `alacritty_terminal::Term`  
- Input encoding  
- Resize / modes / OSC  
- Damage bookkeeping  

### Move to GPU

| CPU (cheap) | GPU |
|---|---|
| Build `CellInstance { glyph_id, fg, bg, flags, row, col }` for dirty cells | Draw quads |
| Rasterize missing glyphs into atlas once | Sample atlas |
| Cursor / selection as instances | Same pipeline or overlay |

### Crate layout

```text
rmux-terminal/       # VT + snapshot/damage (CPU) — stays
rmux-terminal-gpu/   # atlas, buffers, WGSL, paint callback  (NEW)
rmux-app/            # wires pane rect → GPU; chrome stays egui
```

---

## Why not “just use Alacritty GPU”?

| Idea | Use Alacritty’s code? | Use the idea? |
|---|---|---|
| OpenGL renderer | **No** | Only if we abandon egui for that pane |
| Damage-only redraw | No need | **Yes** (required even with GPU) |
| GPU glyph atlas | **No** | **Yes** — our atlas / path |

Alacritty’s GPU stack is “one window, one terminal, one GL pipeline.”  
rmux is “many panes inside a GUI framework.” Different product.

---

## Implementation phases

### Phase G0 — Foundation (this branch, first commit series)

- Enable **eframe `wgpu`** (default renderer becomes wgpu when both glow+wgpu are enabled).
- Spike: solid-color (or test pattern) fill of the terminal pane via `egui_wgpu::Callback` / `CallbackTrait`.
- Prove multipane + resize + HiDPI (viewport set by egui to callback rect).
- Keep existing egui glyph path so the app stays usable while the spike is verified.

**Success:** log “GPU terminal surface ready”; pane base fill drawn by wgpu without crash.

### Phase G1 — Glyph atlas

- Rasterize monospace face (`fontdue` / `ab_glyph` / `swash` — pick one).
- Atlas texture: `(char, bold, …) → UV`.
- Cell size from font metrics (match current egui metrics as closely as possible).

### Phase G2 — Full-grid GPU paint

- Upload full `cols*rows` instance buffer each frame (or each dirty frame).
- Shader: cell quad → sample atlas → fg/bg.
- Feature flag or env fallback to egui renderer if GPU init fails.
- Cursor as extra instance or invert pass.

**Success:** LazyVim navigable with GPU path; CPU time in galley layout collapses.

### Phase G3 — Damage uploads (where lag really dies)

- Dirty row bitset after `feed_bytes` / scroll / resize.
- Only re-upload dirty row ranges.
- Cursor-only: patch 2 cells.
- Skip GPU work when nothing dirty and no blink.

### Phase G4 — Parity with current egui renderer

| Feature | Approach |
|---|---|
| Wide chars | span 2 columns |
| Underline / strikethrough | thin quads or flag bits |
| Selection | bg override in cell data |
| Dim / inverse / hidden | resolve on CPU into instance colors |
| Box-drawing | atlas glyphs or procedural |
| Transparency / wallpaper | premultiplied alpha; clear with `terminal_bg * opacity` |
| Scrollback viewport | `display_offset`; upload visible rows only |

### Phase G5 — Multi-pane lifecycle

- Shared atlas (keyed by font size / scale).
- Per-pane cell buffers.
- Drop resources on pane close.
- Resize → recreate buffers + full damage.

---

## Shader mental model

```wgsl
// Instance: col, row, glyph_uv, fg, bg, flags
// Vertex: expand to 4 corners of cell rect in pane space
// Frag:
//   if sample.a == 0 { out = bg; } else { out = mix(bg, fg, sample.a); }
```

GPUs handle ~10k cell quads easily; optional CPU run-length batching of backgrounds is a later optimization.

---

## Dependencies (expected)

| Crate | Role |
|---|---|
| `eframe` feature `wgpu` | Device/queue + egui-wgpu |
| `egui-wgpu` / `wgpu` (via eframe) | Paint callbacks |
| `bytemuck` | POD instance/uniform buffers |
| `fontdue` or `ab_glyph` (G1+) | Glyph raster → atlas |
| optional atlas packer | `etagere` / shelf |

Still **no** Alacritty display/renderer crate.

---

## PR delivery sequence

```text
PR1  G0: wgpu eframe + PaintCallback solid rect in pane   ← current
PR2  G1: Glyph atlas + grid of chars
PR3  G2: Full terminal from alacritty grid (fallback egui)
PR4  G3: Damage uploads + cursor path
PR5  G4: Selection, underline, wide char, opacity parity
PR6  Default-on when stable; keep or drop egui fallback
```

---

## Risks

| Risk | Mitigation |
|---|---|
| Font quality vs egui | Match size/metrics; side-by-side compare |
| CJK / emoji | atlas growth; fallback fonts later |
| HiDPI | use `pixels_per_point` for atlas + cells |
| Lag remains without damage | G3 is mandatory |
| Debug vs release | measure **release** only |
| `#![forbid(unsafe_code)]` in app | keep GPU crate free of unsafe if possible; isolate if needed |
| Time | multi-PR; always keep egui fallback |

---

## Fallback ladder (if GPU path does not work out)

Use this order — each step is a valid product path:

### Fallback A — Damage-only on current egui path (fastest lag win)

**Do not abandon this idea** if G0–G2 stall.

1. Dirty flag after `feed_bytes` / scroll / resize / selection.  
2. Reuse last snapshot when not dirty.  
3. Row damage: paint only changed rows.  
4. Cursor-only path when only cursor moved.  
5. Quieter idle: stop unconditional 16 ms repaint when nothing dirty.

**Keeps:** `alacritty_terminal` + egui.  
**Effort:** days, not weeks.  
**Expected:** most of LazyVim mush gone without graphics rewrite.

Work can live on this same branch or a sibling `feat/terminal-damage-redraw` focused PR if GPU is parked.

### Fallback B — Heavier egui optimizations only

- Run-length text batching for same-style ASCII  
- Mesh reuse / fewer shapes  
- Snapshot buffer pool  

**Not** true GPU terminal; diminishing returns after A.

### Fallback C — Different UI host (Phase C / nuclear)

Only if A + GPU both fail product goals vs cmux:

- Embed **Ghostty / libghostty** surfaces (cmux-like)  
- Or other native terminal surfaces per pane  

**Cost:** architecture fork; platform divergence; multipane/wallpaper/input complexity.

### Fallback D — Stay on glow + egui_glow callbacks

If wgpu eframe integration is broken on a target OS:

- Keep glow backend  
- `egui_glow::CallbackFn` for a GL cell renderer  
- More `unsafe` / platform pain  

Prefer fixing wgpu first.

---

## Decision log

| Date | Decision |
|---|---|
| 2026-07-26 | Pursue **custom wgpu pane** (Option 1), not Alacritty embed. |
| 2026-07-26 | Keep `alacritty_terminal` for VT only. |
| 2026-07-26 | Document fallbacks **before** committing to G0 in this worktree. |
| 2026-07-26 | Branch/worktree: `feat/terminal-damage-redraw` hosts G0 (name historical; scope is GPU + later damage). |

---

## How to verify G0

```bash
cd /path/to/rmux-feat-terminal-damage-redraw
cargo run -p rmux-app --release
```

Expect:

1. App starts on **wgpu** renderer.  
2. Log line that GPU terminal surface initialized.  
3. Terminal panes still usable (egui glyphs until G2).  
4. Base pane fill comes from the paint callback when GPU init succeeded.

```bash
RUST_LOG=info,rmux_terminal_gpu=debug cargo run -p rmux-app --release
```

---

## Related docs

- `docs/ARCHITECTURE.md` — overall app layout  
- `docs/guide/05-terminal-renderer.md` — current CPU renderer  
- PR #38 — I/O / wake / repaint latency work already on `main`  
