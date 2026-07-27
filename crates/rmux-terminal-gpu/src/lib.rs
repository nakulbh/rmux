//! GPU terminal surface for rmux (wgpu + egui paint callbacks).
//!
//! # Phases
//!
//! * **G0 (this module):** solid pane fill via [`egui_wgpu::Callback`] to prove the
//!   eframe wgpu path works inside a multipane terminal layout.
//! * **G1+:** glyph atlas, cell instance buffers, damage uploads — see
//!   `docs/TERMINAL_GPU_RENDER.md`.
//!
//! VT / grid stay in `rmux-terminal` (`alacritty_terminal`). This crate only paints.

#![forbid(unsafe_code)]

mod pane_fill;

use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui_wgpu;
use egui::{Color32, Rect, Ui};

pub use pane_fill::paint_pane_fill;

/// Set once at app startup when GPU resources were installed successfully.
static GPU_READY: AtomicBool = AtomicBool::new(false);

/// Whether the G0 GPU surface was initialized for this process.
#[inline]
pub fn is_ready() -> bool {
    GPU_READY.load(Ordering::Relaxed)
}

/// Install shared wgpu resources used by terminal paint callbacks.
///
/// Call once from [`eframe::App`] construction with a wgpu
/// [`eframe::CreationContext`]. Returns `false` if eframe is not on the wgpu
/// backend (e.g. forced glow) so callers can fall back to CPU fills.
pub fn init(cc: &eframe::CreationContext<'_>) -> bool {
    let Some(wgpu_render_state) = cc.wgpu_render_state.as_ref() else {
        tracing::warn!(
            "rmux-terminal-gpu: no wgpu_render_state — terminal GPU surface disabled \
             (is eframe using the wgpu renderer?)"
        );
        GPU_READY.store(false, Ordering::Relaxed);
        return false;
    };

    match pane_fill::install_resources(wgpu_render_state) {
        Ok(()) => {
            GPU_READY.store(true, Ordering::Relaxed);
            tracing::info!("rmux-terminal-gpu: G0 pane-fill surface ready (wgpu)");
            true
        }
        Err(err) => {
            tracing::error!(error = %err, "rmux-terminal-gpu: failed to install G0 resources");
            GPU_READY.store(false, Ordering::Relaxed);
            false
        }
    }
}

/// Convert egui [`Color32`] to premultiplied-ish linear-ish RGBA for the G0 fill shader.
///
/// G0 does not do color-space conversion; we pass unorm channels as `f32`.
#[inline]
pub fn color32_to_rgba(c: Color32) -> [f32; 4] {
    let a = f32::from(c.a()) / 255.0;
    [f32::from(c.r()) / 255.0 * a, f32::from(c.g()) / 255.0 * a, f32::from(c.b()) / 255.0 * a, a]
}

/// Paint a solid GPU fill over `rect` if the surface is ready.
///
/// No-op when [`init`] was not successful — caller should use the CPU path.
pub fn try_paint_pane_fill(ui: &mut Ui, rect: Rect, color: Color32) -> bool {
    if !is_ready() {
        return false;
    }
    paint_pane_fill(ui, rect, color32_to_rgba(color));
    true
}

/// Re-export for callers that already depend on eframe's wgpu types.
pub use egui_wgpu::wgpu;
