//! Cross-platform browser engine abstraction.
//!
//! - **Default:** Chromium (CEF OSR) via `browser-chromium` — see
//!   `docs/CHROMIUM_BROWSER_PLAN.md` and `docs/BROWSER_ENGINE.md`.
//! - **Optional:** OS webview via `wry` (`browser-os-webview`) for light builds.
//!
//! Implemented as an **enum** (not `dyn Trait`) so `wry::WebView` can live
//! inside without `Send`/`Sync` requirements that fail on macOS WKWebView.

use anyhow::Result;
use egui::Rect;
use raw_window_handle::HasWindowHandle;

/// Optional navigation callbacks for engines that support them (wry / CEF).
#[derive(Default)]
#[allow(dead_code)] // fields consumed by OS backend; Chromium uses later (E1.7)
pub struct EngineNavHooks {
    pub url_tx: Option<std::sync::mpsc::Sender<String>>,
    pub title_tx: Option<std::sync::mpsc::Sender<String>>,
    pub loading_tx: Option<std::sync::mpsc::Sender<bool>>,
}

/// Which embedded engine is compiled in / active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    /// Platform webview (`wry`).
    #[allow(dead_code)] // unused when only `browser-chromium` is enabled
    OsWebview,
    /// Chromium Embedded Framework (off-screen → egui texture).
    /// Constructed when building with `--features browser-chromium`.
    #[allow(dead_code)]
    Chromium,
}

impl EngineKind {
    /// Human-readable label for status / about UI.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OsWebview => "os-webview",
            Self::Chromium => "chromium",
        }
    }

    /// Engine selected at compile time.
    #[must_use]
    pub const fn compiled() -> Self {
        #[cfg(feature = "browser-chromium")]
        {
            Self::Chromium
        }
        #[cfg(not(feature = "browser-chromium"))]
        {
            Self::OsWebview
        }
    }
}

/// Compile-time selected backend — sole attach/navigate path for [`super::BrowserPane`].
pub enum EngineBackend {
    #[cfg(not(feature = "browser-chromium"))]
    Os(crate::browser::os_webview::OsWebviewEngine),
    #[cfg(feature = "browser-chromium")]
    Chromium(crate::browser::chromium::ChromiumEngine),
}

impl EngineBackend {
    #[must_use]
    pub fn create() -> Self {
        #[cfg(feature = "browser-chromium")]
        {
            Self::Chromium(crate::browser::chromium::ChromiumEngine::new())
        }
        #[cfg(not(feature = "browser-chromium"))]
        {
            Self::Os(crate::browser::os_webview::OsWebviewEngine::new())
        }
    }

    #[must_use]
    pub fn kind(&self) -> EngineKind {
        match self {
            #[cfg(not(feature = "browser-chromium"))]
            Self::Os(_) => EngineKind::OsWebview,
            #[cfg(feature = "browser-chromium")]
            Self::Chromium(_) => EngineKind::Chromium,
        }
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        match self {
            #[cfg(not(feature = "browser-chromium"))]
            Self::Os(e) => e.is_ready(),
            #[cfg(feature = "browser-chromium")]
            Self::Chromium(e) => e.is_ready(),
        }
    }

    pub fn navigate(&mut self, url: &str) -> Result<()> {
        match self {
            #[cfg(not(feature = "browser-chromium"))]
            Self::Os(e) => e.navigate(url),
            #[cfg(feature = "browser-chromium")]
            Self::Chromium(e) => e.navigate(url),
        }
    }

    pub fn reload(&mut self) -> Result<()> {
        match self {
            #[cfg(not(feature = "browser-chromium"))]
            Self::Os(e) => e.reload(),
            #[cfg(feature = "browser-chromium")]
            Self::Chromium(e) => e.reload(),
        }
    }

    pub fn set_bounds(&mut self, bounds: Rect, pixels_per_point: f32) {
        match self {
            #[cfg(not(feature = "browser-chromium"))]
            Self::Os(e) => e.set_bounds(bounds, pixels_per_point),
            #[cfg(feature = "browser-chromium")]
            Self::Chromium(e) => e.set_bounds(bounds, pixels_per_point),
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        match self {
            #[cfg(not(feature = "browser-chromium"))]
            Self::Os(e) => e.set_visible(visible),
            #[cfg(feature = "browser-chromium")]
            Self::Chromium(e) => e.set_visible(visible),
        }
    }

    pub fn ensure_attached(
        &mut self,
        window: &impl HasWindowHandle,
        initial_url: &str,
        hooks: EngineNavHooks,
    ) -> Result<()> {
        match self {
            #[cfg(not(feature = "browser-chromium"))]
            Self::Os(e) => e.ensure_attached(window, initial_url, hooks),
            #[cfg(feature = "browser-chromium")]
            Self::Chromium(e) => e.ensure_attached(window, initial_url, hooks),
        }
    }

    pub fn destroy(&mut self) {
        match self {
            #[cfg(not(feature = "browser-chromium"))]
            Self::Os(e) => e.destroy(),
            #[cfg(feature = "browser-chromium")]
            Self::Chromium(e) => e.destroy(),
        }
    }

    pub fn evaluate_script(&mut self, script: &str) -> Result<()> {
        match self {
            #[cfg(not(feature = "browser-chromium"))]
            Self::Os(e) => e.evaluate_script(script),
            #[cfg(feature = "browser-chromium")]
            Self::Chromium(e) => e.evaluate_script(script),
        }
    }

    pub fn evaluate_script_async(
        &mut self,
        script: &str,
    ) -> Result<std::sync::mpsc::Receiver<String>> {
        match self {
            #[cfg(not(feature = "browser-chromium"))]
            Self::Os(e) => e.evaluate_script_async(script),
            #[cfg(feature = "browser-chromium")]
            Self::Chromium(e) => e.evaluate_script_async(script),
        }
    }

    /// OSR only — RGBA frame for egui texture upload.
    pub fn take_frame_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        match self {
            #[cfg(not(feature = "browser-chromium"))]
            Self::Os(_) => None,
            #[cfg(feature = "browser-chromium")]
            Self::Chromium(e) => e.take_frame_rgba(),
        }
    }

    /// Complete pending `browser.eval` results (Chromium console bridge).
    pub fn poll_eval_result(&mut self) {
        match self {
            #[cfg(not(feature = "browser-chromium"))]
            Self::Os(_) => {}
            #[cfg(feature = "browser-chromium")]
            Self::Chromium(e) => e.poll_eval_result(),
        }
    }

    /// Forward pointer motion into the OSR engine (view-local coords).
    pub fn send_mouse_move(&mut self, x: f32, y: f32, modifiers: u32) {
        match self {
            #[cfg(not(feature = "browser-chromium"))]
            Self::Os(_) => {
                let _ = (x, y, modifiers);
            }
            #[cfg(feature = "browser-chromium")]
            Self::Chromium(e) => e.send_mouse_move(x, y, modifiers),
        }
    }

    pub fn send_mouse_click(&mut self, x: f32, y: f32, button: u8, down: bool, modifiers: u32) {
        match self {
            #[cfg(not(feature = "browser-chromium"))]
            Self::Os(_) => {
                let _ = (x, y, button, down, modifiers);
            }
            #[cfg(feature = "browser-chromium")]
            Self::Chromium(e) => {
                use crate::browser::chromium::MouseBtn;
                let btn = match button {
                    1 => MouseBtn::Right,
                    2 => MouseBtn::Middle,
                    _ => MouseBtn::Left,
                };
                e.send_mouse_click(x, y, btn, down, modifiers);
            }
        }
    }

    pub fn send_mouse_wheel(&mut self, x: f32, y: f32, dx: f32, dy: f32, modifiers: u32) {
        match self {
            #[cfg(not(feature = "browser-chromium"))]
            Self::Os(_) => {
                let _ = (x, y, dx, dy, modifiers);
            }
            #[cfg(feature = "browser-chromium")]
            Self::Chromium(e) => e.send_mouse_wheel(x, y, dx, dy, modifiers),
        }
    }

    pub fn send_key_char(&mut self, ch: char, modifiers: u32) {
        match self {
            #[cfg(not(feature = "browser-chromium"))]
            Self::Os(_) => {
                let _ = (ch, modifiers);
            }
            #[cfg(feature = "browser-chromium")]
            Self::Chromium(e) => e.send_key_char(ch, modifiers),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_engine_is_os_webview_by_default() {
        #[cfg(not(feature = "browser-chromium"))]
        assert_eq!(EngineKind::compiled(), EngineKind::OsWebview);
        #[cfg(feature = "browser-chromium")]
        assert_eq!(EngineKind::compiled(), EngineKind::Chromium);
    }

    #[test]
    fn engine_kind_labels() {
        assert_eq!(EngineKind::OsWebview.as_str(), "os-webview");
        assert_eq!(EngineKind::Chromium.as_str(), "chromium");
    }

    #[test]
    fn create_backend_matches_compiled_kind() {
        let backend = EngineBackend::create();
        assert_eq!(backend.kind(), EngineKind::compiled());
        assert!(!backend.is_ready());
    }
}
