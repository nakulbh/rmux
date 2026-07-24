//! Per-pane Chromium OSR browser.

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail};
use cef::{ImplBrowser, ImplBrowserHost, ImplFrame};
use egui::Rect;
use tracing::{debug, info};

use super::handlers::{
    ClientBuilder, FrameBuffer, NavChannels, OsrClient, OsrRenderHandler, ViewSize,
};
use super::runtime::{self, cef_path, profile_dir};
use crate::browser::engine::EngineNavHooks;

/// Chromium engine (one OSR browser per pane).
pub struct ChromiumEngine {
    bounds: Rect,
    pixels_per_point: f32,
    ready: bool,
    browser: Arc<Mutex<Option<cef::Browser>>>,
    view_size: Arc<Mutex<ViewSize>>,
    frame: FrameBuffer,
    /// Console `RMUX_EVAL:` payloads from DisplayHandler.
    eval_rx: Option<std::sync::mpsc::Receiver<String>>,
    /// Completes the active `evaluate_script_async` waiter.
    pending_eval_tx: Option<std::sync::mpsc::Sender<String>>,
    /// Last mouse position for drag continuity (view coords).
    last_mouse: Option<(i32, i32)>,
}

impl ChromiumEngine {
    #[must_use]
    pub fn new() -> Self {
        Self {
            bounds: Rect::ZERO,
            pixels_per_point: 1.0,
            ready: false,
            browser: Arc::new(Mutex::new(None)),
            view_size: Arc::new(Mutex::new(ViewSize {
                width: 1.0,
                height: 1.0,
                device_scale_factor: 1.0,
            })),
            frame: Arc::new(Mutex::new(super::handlers::OsrFrameStore::default())),
            eval_rx: None,
            pending_eval_tx: None,
            last_mouse: None,
        }
    }

    #[must_use]
    #[allow(dead_code)]
    pub fn cef_path() -> Option<std::path::PathBuf> {
        cef_path()
    }

    #[must_use]
    #[allow(dead_code)]
    pub fn profile_dir() -> std::path::PathBuf {
        profile_dir()
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready && self.browser.lock().map(|b| b.is_some()).unwrap_or(false)
    }

    pub fn navigate(&mut self, url: &str) -> Result<()> {
        self.require_ready()?;
        let browser = self.browser.lock().unwrap_or_else(|e| e.into_inner());
        let Some(browser) = browser.as_ref() else {
            bail!("Chromium browser handle missing");
        };
        let Some(frame) = browser.main_frame() else {
            bail!("Chromium main frame missing");
        };
        // Keep CefString alive for the FFI call (do not pass a temporary).
        let cef_url = cef::CefString::from(url);
        frame.load_url(Some(&cef_url));
        info!(%url, "Chromium load_url issued");
        Ok(())
    }

    pub fn reload(&mut self) -> Result<()> {
        self.require_ready()?;
        let browser = self.browser.lock().unwrap_or_else(|e| e.into_inner());
        let Some(browser) = browser.as_ref() else {
            bail!("Chromium browser handle missing");
        };
        browser.reload();
        Ok(())
    }

    #[allow(dead_code)] // available for CEF history vs pane history (Phase E2.7)
    pub fn go_back(&mut self) -> Result<()> {
        self.require_ready()?;
        let browser = self.browser.lock().unwrap_or_else(|e| e.into_inner());
        let Some(browser) = browser.as_ref() else {
            bail!("Chromium browser handle missing");
        };
        if browser.can_go_back() == 1 {
            browser.go_back();
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn go_forward(&mut self) -> Result<()> {
        self.require_ready()?;
        let browser = self.browser.lock().unwrap_or_else(|e| e.into_inner());
        let Some(browser) = browser.as_ref() else {
            bail!("Chromium browser handle missing");
        };
        if browser.can_go_forward() == 1 {
            browser.go_forward();
        }
        Ok(())
    }

    pub fn set_bounds(&mut self, bounds: Rect, pixels_per_point: f32) {
        self.bounds = bounds;
        let ppp = if pixels_per_point.is_finite() && pixels_per_point > 0.0 {
            pixels_per_point
        } else {
            1.0
        };
        self.pixels_per_point = ppp;

        let w = bounds.width().max(1.0);
        let h = bounds.height().max(1.0);
        if let Ok(mut size) = self.view_size.lock() {
            let changed = (size.width - w).abs() > 0.5
                || (size.height - h).abs() > 0.5
                || (size.device_scale_factor - ppp).abs() > 0.01;
            size.width = w;
            size.height = h;
            size.device_scale_factor = ppp;
            if changed && self.ready {
                drop(size);
                if let Ok(browser) = self.browser.lock()
                    && let Some(browser) = browser.as_ref()
                    && let Some(host) = browser.host()
                {
                    host.was_resized();
                    host.notify_screen_info_changed();
                }
            }
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        if !self.ready {
            return;
        }
        if let Ok(browser) = self.browser.lock()
            && let Some(browser) = browser.as_ref()
            && let Some(host) = browser.host()
        {
            host.was_hidden((!visible) as i32);
        }
    }

    pub fn ensure_attached(
        &mut self,
        _window: &impl raw_window_handle::HasWindowHandle,
        initial_url: &str,
        hooks: EngineNavHooks,
    ) -> Result<()> {
        if self.ready {
            return Ok(());
        }
        if self.bounds.width() <= 1.0 || self.bounds.height() <= 1.0 {
            return Ok(());
        }

        runtime::ensure_runtime().context("CEF runtime")?;

        let url = if initial_url.is_empty()
            || initial_url == "about:blank"
            || initial_url.starts_with("data:")
        {
            // Dark blank page so chrome matches app; load via data URL.
            "data:text/html;charset=utf-8,\
             <!DOCTYPE html><html><head><meta charset=utf-8>\
             <style>html,body{margin:0;height:100%;background:#0c0c0c;color:#71717a;\
             font-family:system-ui,sans-serif;display:flex;align-items:center;justify-content:center}\
             .hint{opacity:.55;font-size:13px;text-align:center;line-height:1.5}</style></head>\
             <body><div class=hint>Type a URL in the address bar above<br>and press Enter</div></body></html>"
                .to_string()
        } else {
            initial_url.to_string()
        };

        {
            let mut size = self.view_size.lock().unwrap_or_else(|e| e.into_inner());
            size.width = self.bounds.width().max(1.0);
            size.height = self.bounds.height().max(1.0);
            size.device_scale_factor = self.pixels_per_point;
        }

        let (eval_tx, eval_rx) = std::sync::mpsc::channel();
        self.eval_rx = Some(eval_rx);

        let channels = Arc::new(NavChannels {
            url: hooks.url_tx,
            title: hooks.title_tx,
            loading: hooks.loading_tx,
            eval: Some(eval_tx),
        });

        let render = OsrRenderHandler::new(self.view_size.clone(), self.frame.clone());
        let client = OsrClient::new(render, channels, self.browser.clone());
        let mut client = ClientBuilder::build(client);

        let window_info = cef::WindowInfo {
            windowless_rendering_enabled: true as _,
            shared_texture_enabled: false as _,
            external_begin_frame_enabled: false as _,
            bounds: cef::Rect {
                x: 0,
                y: 0,
                width: self.bounds.width().max(1.0) as i32,
                height: self.bounds.height().max(1.0) as i32,
            },
            ..Default::default()
        };

        let browser_settings = cef::BrowserSettings {
            // 30–45 fps is enough for OSR→egui; 60 burns CPU on conversion/upload.
            windowless_frame_rate: 45,
            background_color: 0xFF_0C_0C_0C, // ARGB dark
            ..Default::default()
        };

        let mut context = cef::request_context_create_context(
            Some(&cef::RequestContextSettings {
                cache_path: cef::CefString::from(
                    profile_dir().join("default").to_string_lossy().as_ref(),
                ),
                persist_session_cookies: true as _,
                ..Default::default()
            }),
            None,
        );

        let browser = cef::browser_host_create_browser_sync(
            Some(&window_info),
            Some(&mut client),
            Some(&url.as_str().into()),
            Some(&browser_settings),
            None,
            context.as_mut(),
        );

        let Some(browser) = browser else {
            bail!("browser_host_create_browser_sync returned None");
        };

        {
            let mut slot = self.browser.lock().unwrap_or_else(|e| e.into_inner());
            *slot = Some(browser);
        }

        self.ready = true;
        info!(
            url = %initial_url,
            cef = ?cef_path().map(|p| p.display().to_string()),
            "Chromium OSR browser attached"
        );
        Ok(())
    }

    pub fn destroy(&mut self) {
        if let Ok(mut slot) = self.browser.lock()
            && let Some(browser) = slot.take()
            && let Some(host) = browser.host()
        {
            host.close_browser(true as i32);
        }
        self.ready = false;
        self.eval_rx = None;
        self.pending_eval_tx = None;
        if let Ok(mut frame) = self.frame.lock() {
            *frame = super::handlers::OsrFrameStore::default();
        }
        debug!("Chromium browser destroyed");
    }

    pub fn evaluate_script(&mut self, script: &str) -> Result<()> {
        self.require_ready()?;
        self.execute_js(script)
    }

    pub fn evaluate_script_async(
        &mut self,
        script: &str,
    ) -> Result<std::sync::mpsc::Receiver<String>> {
        self.require_ready()?;
        // Drain stale console eval payloads so we pick up this invocation.
        if let Some(ref eval_rx) = self.eval_rx {
            while eval_rx.try_recv().is_ok() {}
        }

        let wrapped = format!(
            r#"(function(){{
  try {{
    var __v = (function(){{ return ({script}); }})();
    var __out;
    if (typeof __v === 'undefined') {{ __out = 'null'; }}
    else if (typeof __v === 'string') {{ __out = JSON.stringify(__v); }}
    else {{ __out = JSON.stringify(__v); }}
    console.log('RMUX_EVAL:' + __out);
  }} catch (e) {{
    console.log('RMUX_EVAL:' + JSON.stringify(String(e)));
  }}
}})()"#
        );
        self.execute_js(&wrapped)?;

        // Return a receiver the UI thread will complete by polling `poll_eval_result`.
        // We also expose a channel pair: store pending tx so pump can complete it.
        let (tx, rx) = std::sync::mpsc::channel();
        self.pending_eval_tx = Some(tx);
        Ok(rx)
    }

    /// Called from the UI thread after CEF pump — completes pending `browser.eval`.
    pub fn poll_eval_result(&mut self) {
        let Some(ref eval_rx) = self.eval_rx else {
            return;
        };
        if let Ok(val) = eval_rx.try_recv()
            && let Some(tx) = self.pending_eval_tx.take()
        {
            let _ = tx.send(val);
        }
    }

    fn execute_js(&mut self, script: &str) -> Result<()> {
        let browser = self.browser.lock().unwrap_or_else(|e| e.into_inner());
        let Some(browser) = browser.as_ref() else {
            bail!("Chromium browser handle missing");
        };
        let Some(frame) = browser.main_frame() else {
            bail!("Chromium main frame missing");
        };
        frame.execute_java_script(Some(&script.into()), Some(&"about:blank".into()), 0);
        Ok(())
    }

    pub fn take_frame_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        self.frame.lock().ok().and_then(|mut g| g.take_if_dirty())
    }

    // ── Input forwarding (egui → CEF) ──────────────────────────────────

    /// Forward a pointer move in **view** coordinates (content rect, top-left).
    ///
    /// Coordinates are scaled to the OSR paint pixel space (capped DPR).
    pub fn send_mouse_move(&mut self, x: f32, y: f32, modifiers: u32) {
        if !self.ready {
            return;
        }
        let (ix, iy) = self.to_paint_coords(x, y);
        // Skip no-op moves — CEF move storms during hover are expensive.
        if self.last_mouse == Some((ix, iy)) {
            return;
        }
        self.last_mouse = Some((ix, iy));
        if let Ok(browser) = self.browser.lock()
            && let Some(browser) = browser.as_ref()
            && let Some(host) = browser.host()
        {
            let event = cef::MouseEvent { x: ix, y: iy, modifiers };
            host.send_mouse_move_event(Some(&event), false as i32);
        }
    }

    fn to_paint_coords(&self, x: f32, y: f32) -> (i32, i32) {
        let scale = {
            let ppp = self.pixels_per_point;
            if !ppp.is_finite() || ppp <= 1.0 {
                1.0
            } else {
                ppp.clamp(1.0, 1.5)
            }
        };
        ((x * scale).round() as i32, (y * scale).round() as i32)
    }

    pub fn send_mouse_click(&mut self, x: f32, y: f32, button: MouseBtn, down: bool, modifiers: u32) {
        if !self.ready {
            return;
        }
        let (ix, iy) = self.to_paint_coords(x, y);
        self.last_mouse = Some((ix, iy));
        if let Ok(browser) = self.browser.lock()
            && let Some(browser) = browser.as_ref()
            && let Some(host) = browser.host()
        {
            let event = cef::MouseEvent { x: ix, y: iy, modifiers };
            let btn = match button {
                MouseBtn::Left => cef::MouseButtonType::from(cef::sys::cef_mouse_button_type_t::MBT_LEFT),
                MouseBtn::Right => {
                    cef::MouseButtonType::from(cef::sys::cef_mouse_button_type_t::MBT_RIGHT)
                }
                MouseBtn::Middle => {
                    cef::MouseButtonType::from(cef::sys::cef_mouse_button_type_t::MBT_MIDDLE)
                }
            };
            host.send_mouse_click_event(Some(&event), btn, (!down) as i32, 1);
        }
    }

    pub fn send_mouse_wheel(&mut self, x: f32, y: f32, delta_x: f32, delta_y: f32, modifiers: u32) {
        if !self.ready {
            return;
        }
        let (ix, iy) = self.to_paint_coords(x, y);
        if let Ok(browser) = self.browser.lock()
            && let Some(browser) = browser.as_ref()
            && let Some(host) = browser.host()
        {
            let event = cef::MouseEvent { x: ix, y: iy, modifiers };
            // egui deltas are in points; CEF expects pixels. Amplify slightly so
            // trackpads feel responsive without flooding paints.
            let scale = self.pixels_per_point.clamp(1.0, 2.0);
            host.send_mouse_wheel_event(
                Some(&event),
                (delta_x * scale).round() as i32,
                (delta_y * scale).round() as i32,
            );
        }
    }

    pub fn send_key_char(&mut self, ch: char, modifiers: u32) {
        if !self.ready || ch == '\0' {
            return;
        }
        if let Ok(browser) = self.browser.lock()
            && let Some(browser) = browser.as_ref()
            && let Some(host) = browser.host()
        {
            let event = cef::KeyEvent {
                size: std::mem::size_of::<cef::KeyEvent>(),
                type_: cef::KeyEventType::from(cef::sys::cef_key_event_type_t::KEYEVENT_CHAR),
                modifiers,
                windows_key_code: ch as i32,
                native_key_code: ch as i32,
                character: ch as u16,
                unmodified_character: ch as u16,
                is_system_key: 0,
                focus_on_editable_field: 0,
            };
            host.send_key_event(Some(&event));
        }
    }

    fn require_ready(&self) -> Result<()> {
        if self.ready {
            Ok(())
        } else {
            bail!(
                "Chromium engine not ready. Install CEF via ./scripts/fetch-cef.sh \
                 and open a browser pane after the runtime boots."
            )
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MouseBtn {
    Left,
    Right,
    Middle,
}

impl Default for ChromiumEngine {
    fn default() -> Self {
        Self::new()
    }
}
