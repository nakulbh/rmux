//! Chromium (CEF) browser backend — off-screen rendering into egui textures.
//!
//! **Default engine** (`browser-chromium` is on by default). Requires CEF:
//!
//! ```bash
//! ./scripts/fetch-cef.sh
//! eval "$(./scripts/fetch-cef.sh --print-env)"
//! cargo run -p rmux-app
//! ```
//!
//! Architecture (see `docs/CHROMIUM_BROWSER_PLAN.md`):
//! - Process-wide CEF runtime with **external message pump** (`do_message_loop_work` from egui)
//! - Windowless (OSR) browsers per pane
//! - `on_paint` → BGRA buffer → RGBA → egui `ColorImage`

mod engine;
mod handlers;
mod runtime;

pub use engine::{ChromiumEngine, MouseBtn};
pub use runtime::{pump_message_loop, try_run_cef_subprocess};
