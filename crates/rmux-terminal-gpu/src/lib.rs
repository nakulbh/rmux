//! GPU terminal surface for rmux (wgpu + egui paint callbacks).
//!
//! # Phases
//!
//! * **G0:** solid pane fill (see `pane_fill`)
//! * **G1:** fontdue glyph atlas (`atlas`)
//! * **G2:** full-grid instanced paint (`grid`) — primary path when ready
//! * **G3+:** damage uploads — see `docs/TERMINAL_GPU_RENDER.md`
//!
//! VT / grid stay in `rmux-terminal` (`alacritty_terminal`). This crate only paints.

#![forbid(unsafe_code)]

mod atlas;
mod grid;
mod pane_fill;

use std::sync::atomic::{AtomicBool, Ordering};

use egui::{Color32, Rect, Ui};
use rmux_terminal::GridSnapshot;

pub use grid::{alloc_pane_id, begin_frame, grid_is_ready, paint_grid};

/// Set once at app startup when any GPU surface (G0+) was installed.
static GPU_READY: AtomicBool = AtomicBool::new(false);

/// Whether a GPU surface was initialized (G0 fill and/or G2 grid).
#[inline]
pub fn is_ready() -> bool {
    GPU_READY.load(Ordering::Relaxed)
}

/// Font bytes + size used to build the atlas at init.
pub struct FontSetup<'a> {
    pub regular: &'a [u8],
    pub bold: &'a [u8],
    pub size: f32,
}

/// Install shared wgpu resources (G0 fill + G1/G2 grid).
///
/// Call once from [`eframe::App`] construction. Returns `false` if eframe is
/// not on the wgpu backend.
pub fn init(cc: &eframe::CreationContext<'_>, fonts: FontSetup<'_>) -> bool {
    grid::set_grid_ready(false);

    let Some(wgpu_render_state) = cc.wgpu_render_state.as_ref() else {
        tracing::warn!("rmux-terminal-gpu: no wgpu_render_state — terminal GPU surface disabled");
        GPU_READY.store(false, Ordering::Relaxed);
        return false;
    };

    if let Err(err) = pane_fill::install_resources(wgpu_render_state) {
        tracing::error!(error = %err, "rmux-terminal-gpu: G0 pane-fill install failed");
        GPU_READY.store(false, Ordering::Relaxed);
        return false;
    }
    GPU_READY.store(true, Ordering::Relaxed);

    match grid::GridGpu::install(wgpu_render_state, fonts.regular, fonts.bold, fonts.size) {
        Ok(()) => {
            tracing::info!("rmux-terminal-gpu: G0–G2 surface ready (wgpu + glyph atlas)");
            true
        }
        Err(err) => {
            tracing::error!(error = %err, "rmux-terminal-gpu: G1/G2 grid install failed");
            tracing::warn!("rmux-terminal-gpu: G0 only — panes will use egui glyphs");
            grid::set_grid_ready(false);
            true
        }
    }
}

/// Convert egui [`Color32`] to premultiplied RGBA for the G0 fill shader.
#[inline]
pub fn color32_to_rgba(c: Color32) -> [f32; 4] {
    let a = f32::from(c.a()) / 255.0;
    [f32::from(c.r()) / 255.0 * a, f32::from(c.g()) / 255.0 * a, f32::from(c.b()) / 255.0 * a, a]
}

/// Paint a solid GPU fill over `rect` if G0 is ready.
pub fn try_paint_pane_fill(ui: &mut Ui, rect: Rect, color: Color32) -> bool {
    if !is_ready() {
        return false;
    }
    pane_fill::paint_pane_fill(ui, rect, color32_to_rgba(color));
    true
}

/// Paint the full terminal grid on the GPU.
///
/// Returns `false` unless the grid pipeline is ready (caller uses egui).
/// Disable with `RMUX_GPU_GRID=0` if you need the pure egui path.
pub fn try_paint_grid(
    ui: &mut Ui,
    rect: Rect,
    snapshot: &GridSnapshot,
    cursor_visible: bool,
    opacity: f32,
    font_size: f32,
    pane_id: u64,
) -> bool {
    if !grid_is_ready() {
        return false;
    }
    // Escape hatch: force CPU egui paint.
    static GPU_GRID_OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let disabled = *GPU_GRID_OFF.get_or_init(|| {
        std::env::var_os("RMUX_GPU_GRID").is_some_and(|v| v == "0" || v == "false")
    });
    if disabled {
        return false;
    }
    paint_grid(ui, rect, snapshot, cursor_visible, opacity, font_size, pane_id)
}

pub use pane_fill::paint_pane_fill;
