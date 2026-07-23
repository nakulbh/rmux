//! CEF App / Client / RenderHandler / Display / Load / LifeSpan wrappers for OSR.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

// wrap_* macros and CEF method calls need the full prelude + Rc trait.
use cef::rc::Rc;
use cef::*;

/// Shared OSR frame buffer: (width, height, rgba bytes).
pub type FrameBuffer = Arc<Mutex<Option<(u32, u32, Vec<u8>)>>>;

/// Navigation / chrome events pushed to the UI thread via channels.
pub struct NavChannels {
    pub url: Option<std::sync::mpsc::Sender<String>>,
    pub title: Option<std::sync::mpsc::Sender<String>>,
    pub loading: Option<std::sync::mpsc::Sender<bool>>,
    /// Console lines starting with `RMUX_EVAL:` for automation.
    pub eval: Option<std::sync::mpsc::Sender<String>>,
}

#[derive(Clone)]
pub struct OsrApp {}

impl OsrApp {
    pub fn new() -> Self {
        Self {}
    }
}

wrap_app! {
    pub(crate) struct AppBuilder {
        app: OsrApp,
    }

    impl App {
        fn on_before_command_line_processing(
            &self,
            _process_type: Option<&cef::CefStringUtf16>,
            command_line: Option<&mut cef::CommandLine>,
        ) {
            let Some(command_line) = command_line else {
                return;
            };
            command_line.append_switch(Some(&"no-startup-window".into()));
            command_line.append_switch(Some(&"noerrdialogs".into()));
            command_line.append_switch(Some(&"hide-crash-restore-bubble".into()));
            command_line.append_switch(Some(&"use-mock-keychain".into()));
            // OSR software path — avoid multi-process GPU (macOS helpers are
            // not packaged under `cargo run`; GPU process death kills the app).
            command_line.append_switch(Some(&"disable-gpu".into()));
            command_line.append_switch(Some(&"disable-gpu-compositing".into()));
            command_line.append_switch(Some(&"disable-gpu-sandbox".into()));
            command_line.append_switch(Some(&"in-process-gpu".into()));
            // Single-process is required for bare cargo binaries without CEF
            // Helper.app bundles. Production packaging (E4) can drop this.
            command_line.append_switch(Some(&"single-process".into()));
            command_line.append_switch(Some(&"no-sandbox".into()));
            command_line.append_switch(Some(&"enable-begin-frame-scheduling".into()));
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(BrowserProcessHandlerBuilder::build(OsrBrowserProcessHandler::new()))
        }
    }
}

impl AppBuilder {
    pub(crate) fn build(app: OsrApp) -> cef::App {
        Self::new(app)
    }
}

#[derive(Clone)]
pub struct OsrBrowserProcessHandler {
    is_cef_ready: RefCell<bool>,
}

impl OsrBrowserProcessHandler {
    pub fn new() -> Self {
        Self { is_cef_ready: RefCell::new(false) }
    }
}

wrap_browser_process_handler! {
    pub(crate) struct BrowserProcessHandlerBuilder {
        handler: OsrBrowserProcessHandler,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            *self.handler.is_cef_ready.borrow_mut() = true;
        }

        fn on_before_child_process_launch(&self, command_line: Option<&mut cef::CommandLine>) {
            let Some(command_line) = command_line else {
                return;
            };
            command_line.append_switch(Some(&"disable-gpu".into()));
            command_line.append_switch(Some(&"disable-gpu-compositing".into()));
            command_line.append_switch(Some(&"disable-gpu-sandbox".into()));
            command_line.append_switch(Some(&"in-process-gpu".into()));
            command_line.append_switch(Some(&"no-sandbox".into()));
        }
    }
}

impl BrowserProcessHandlerBuilder {
    pub(crate) fn build(handler: OsrBrowserProcessHandler) -> BrowserProcessHandler {
        Self::new(handler)
    }
}

/// View size for OSR (logical CSS pixels as reported to CEF).
#[derive(Clone)]
pub struct ViewSize {
    pub width: f32,
    pub height: f32,
    pub device_scale_factor: f32,
}

#[derive(Clone)]
pub struct OsrRenderHandler {
    size: Arc<Mutex<ViewSize>>,
    frame: FrameBuffer,
}

impl OsrRenderHandler {
    pub fn new(size: Arc<Mutex<ViewSize>>, frame: FrameBuffer) -> Self {
        Self { size, frame }
    }
}

wrap_render_handler! {
    pub struct RenderHandlerBuilder {
        handler: OsrRenderHandler,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(rect) = rect {
                let size = self.handler.size.lock().unwrap_or_else(|e| e.into_inner());
                if size.width > 0.0 && size.height > 0.0 {
                    rect.width = size.width as i32;
                    rect.height = size.height as i32;
                } else {
                    // CEF requires non-zero view_rect.
                    rect.width = 1;
                    rect.height = 1;
                }
            }
        }

        fn screen_info(
            &self,
            _browser: Option<&mut Browser>,
            screen_info: Option<&mut ScreenInfo>,
        ) -> ::std::os::raw::c_int {
            if let Some(screen_info) = screen_info {
                let size = self.handler.size.lock().unwrap_or_else(|e| e.into_inner());
                screen_info.device_scale_factor = size.device_scale_factor;
                return true as _;
            }
            false as _
        }

        fn screen_point(
            &self,
            _browser: Option<&mut Browser>,
            _view_x: ::std::os::raw::c_int,
            _view_y: ::std::os::raw::c_int,
            _screen_x: Option<&mut ::std::os::raw::c_int>,
            _screen_y: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            false as _
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            type_: PaintElementType,
            _dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            // Only the main view; popups ignored for now.
            if type_ != PaintElementType::default() {
                return;
            }
            if buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            let w = width as u32;
            let h = height as u32;
            let src_len = (w as usize).saturating_mul(h as usize).saturating_mul(4);
            // SAFETY: CEF guarantees `buffer` points to `width*height*4` BGRA bytes for this call.
            let bgra = unsafe { std::slice::from_raw_parts(buffer, src_len) };

            // Convert BGRA → RGBA for egui ColorImage.
            let mut rgba = Vec::with_capacity(src_len);
            for chunk in bgra.chunks_exact(4) {
                rgba.push(chunk[2]); // R
                rgba.push(chunk[1]); // G
                rgba.push(chunk[0]); // B
                rgba.push(chunk[3]); // A
            }

            if let Ok(mut slot) = self.handler.frame.lock() {
                *slot = Some((w, h, rgba));
            }
        }
    }
}

impl RenderHandlerBuilder {
    pub fn build(handler: OsrRenderHandler) -> RenderHandler {
        Self::new(handler)
    }
}

#[derive(Clone)]
pub struct OsrDisplayHandler {
    channels: Arc<NavChannels>,
}

impl OsrDisplayHandler {
    pub fn new(channels: Arc<NavChannels>) -> Self {
        Self { channels }
    }
}

wrap_display_handler! {
    pub struct DisplayHandlerBuilder {
        handler: OsrDisplayHandler,
    }

    impl DisplayHandler {
        fn on_address_change(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut cef::Frame>,
            url: Option<&cef::CefStringUtf16>,
        ) {
            if let (Some(url), Some(tx)) = (url, &self.handler.channels.url) {
                let s = cef::CefStringUtf8::from(url).to_string();
                let _ = tx.send(s);
            }
        }

        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&cef::CefStringUtf16>) {
            if let (Some(title), Some(tx)) = (title, &self.handler.channels.title) {
                let s = cef::CefStringUtf8::from(title).to_string();
                let _ = tx.send(s);
            }
        }

        fn on_console_message(
            &self,
            _browser: Option<&mut Browser>,
            _level: cef::LogSeverity,
            message: Option<&cef::CefStringUtf16>,
            _source: Option<&cef::CefStringUtf16>,
            _line: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            if let (Some(message), Some(tx)) = (message, &self.handler.channels.eval) {
                let s = cef::CefStringUtf8::from(message).to_string();
                if let Some(rest) = s.strip_prefix("RMUX_EVAL:") {
                    let _ = tx.send(rest.to_string());
                    return true as _; // suppress console noise
                }
            }
            false as _
        }
    }
}

impl DisplayHandlerBuilder {
    pub fn build(handler: OsrDisplayHandler) -> DisplayHandler {
        Self::new(handler)
    }
}

#[derive(Clone)]
pub struct OsrLoadHandler {
    channels: Arc<NavChannels>,
}

impl OsrLoadHandler {
    pub fn new(channels: Arc<NavChannels>) -> Self {
        Self { channels }
    }
}

wrap_load_handler! {
    pub struct LoadHandlerBuilder {
        handler: OsrLoadHandler,
    }

    impl LoadHandler {
        fn on_loading_state_change(
            &self,
            _browser: Option<&mut Browser>,
            is_loading: ::std::os::raw::c_int,
            _can_go_back: ::std::os::raw::c_int,
            _can_go_forward: ::std::os::raw::c_int,
        ) {
            if let Some(tx) = &self.handler.channels.loading {
                let _ = tx.send(is_loading != 0);
            }
        }
    }
}

impl LoadHandlerBuilder {
    pub fn build(handler: OsrLoadHandler) -> LoadHandler {
        Self::new(handler)
    }
}

#[derive(Clone)]
pub struct OsrLifeSpanHandler {
    browser_slot: Arc<Mutex<Option<Browser>>>,
}

impl OsrLifeSpanHandler {
    pub fn new(browser_slot: Arc<Mutex<Option<Browser>>>) -> Self {
        Self { browser_slot }
    }
}

wrap_life_span_handler! {
    pub struct LifeSpanHandlerBuilder {
        handler: OsrLifeSpanHandler,
    }

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            if let Some(browser) = browser
                && let Ok(mut slot) = self.handler.browser_slot.lock()
            {
                *slot = Some(browser.clone());
            }
        }

        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            if let Ok(mut slot) = self.handler.browser_slot.lock() {
                *slot = None;
            }
        }

        fn do_close(&self, _browser: Option<&mut Browser>) -> ::std::os::raw::c_int {
            false as _
        }
    }
}

impl LifeSpanHandlerBuilder {
    pub fn build(handler: OsrLifeSpanHandler) -> LifeSpanHandler {
        Self::new(handler)
    }
}

#[derive(Clone)]
pub struct OsrClient {
    render: RenderHandler,
    display: DisplayHandler,
    load: LoadHandler,
    life_span: LifeSpanHandler,
}

impl OsrClient {
    pub fn new(
        render: OsrRenderHandler,
        channels: Arc<NavChannels>,
        browser_slot: Arc<Mutex<Option<Browser>>>,
    ) -> Self {
        Self {
            render: RenderHandlerBuilder::build(render),
            display: DisplayHandlerBuilder::build(OsrDisplayHandler::new(channels.clone())),
            load: LoadHandlerBuilder::build(OsrLoadHandler::new(channels)),
            life_span: LifeSpanHandlerBuilder::build(OsrLifeSpanHandler::new(browser_slot)),
        }
    }
}

wrap_client! {
    pub(crate) struct ClientBuilder {
        client: OsrClient,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(self.client.render.clone())
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(self.client.display.clone())
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(self.client.load.clone())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(self.client.life_span.clone())
        }
    }
}

impl ClientBuilder {
    pub(crate) fn build(client: OsrClient) -> Client {
        Self::new(client)
    }
}
