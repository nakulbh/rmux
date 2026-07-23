//! In-app browser pane.
//!
//! Engine backends:
//! - **Chromium** (`browser-chromium`, **default**) — CEF OSR; see `docs/BROWSER_ENGINE.md`
//! - **OS webview** (`browser-os-webview`, optional) — `wry` light fallback

pub(crate) mod automation;
pub(crate) mod engine;
pub(crate) mod webview;

#[cfg(feature = "browser-chromium")]
pub(crate) mod chromium;

#[cfg(not(feature = "browser-chromium"))]
pub(crate) mod os_webview;

pub use engine::EngineKind;
pub use webview::BrowserPane;

/// CEF helper process gate — call from `main` before eframe when Chromium is enabled.
#[must_use]
pub fn try_run_cef_subprocess() -> bool {
    #[cfg(feature = "browser-chromium")]
    {
        chromium::try_run_cef_subprocess()
    }
    #[cfg(not(feature = "browser-chromium"))]
    {
        false
    }
}

/// Pump CEF message loop (Chromium builds only). Safe no-op elsewhere.
pub fn pump_browser_runtime() {
    #[cfg(feature = "browser-chromium")]
    {
        chromium::pump_message_loop();
    }
}
