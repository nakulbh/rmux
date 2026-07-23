//! Browser pane state + egui chrome. Rendering engine is [`EngineBackend`].

use anyhow::{Context, Result};
use egui::Rect;
use tracing::{debug, info};

use super::engine::{EngineBackend, EngineNavHooks};

/// Empty new-tab page (dark, matches app chrome — avoids white `about:blank`).
const DEFAULT_URL: &str = "about:blank";

pub struct BrowserPane {
    url: String,
    history: Vec<String>,
    history_index: usize,
    /// Active engine (OS webview or Chromium).
    engine: EngineBackend,
    bounds_egui: Rect,
    pixels_per_point: f32,
    is_loading: bool,
    page_title: String,
    is_open: bool,
    /// Set by Cmd/Ctrl+L to request keyboard focus on the URL bar.
    pub focus_url_bar: bool,
    shown_this_frame: bool,
    /// After a failed attach, skip retries until bounds change (avoids log flood).
    attach_failed: bool,
    last_attach_bounds: Rect,
    /// Bridge receivers from engine navigation hooks (set on attach).
    nav_url_rx: Option<std::sync::mpsc::Receiver<String>>,
    nav_title_rx: Option<std::sync::mpsc::Receiver<String>>,
    nav_load_rx: Option<std::sync::mpsc::Receiver<bool>>,
    /// Chromium OSR: last uploaded egui texture (reused when size matches).
    osr_texture: Option<egui::TextureHandle>,
    osr_texture_size: (u32, u32),
}

impl BrowserPane {
    pub fn new() -> Self {
        Self {
            url: DEFAULT_URL.to_string(),
            history: vec![DEFAULT_URL.to_string()],
            history_index: 0,
            engine: EngineBackend::create(),
            bounds_egui: Rect::ZERO,
            pixels_per_point: 1.0,
            is_loading: false,
            page_title: String::new(),
            is_open: false,
            focus_url_bar: false,
            shown_this_frame: false,
            attach_failed: false,
            last_attach_bounds: Rect::ZERO,
            nav_url_rx: None,
            nav_title_rx: None,
            nav_load_rx: None,
            osr_texture: None,
            osr_texture_size: (0, 0),
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn title(&self) -> &str {
        &self.page_title
    }

    pub fn engine_kind(&self) -> crate::browser::engine::EngineKind {
        self.engine.kind()
    }

    /// Tab chrome label: page title, host, or "New tab" for blank pages.
    pub fn tab_title(&self) -> String {
        let title = self.page_title.trim();
        if !title.is_empty() && !title.eq_ignore_ascii_case("about:blank") {
            return title.chars().take(24).collect();
        }
        let url = self.url.trim();
        if url.is_empty() || url == "about:blank" || url.starts_with("data:") {
            return "New tab".to_string();
        }
        if let Some(rest) = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")) {
            let host = rest.split('/').next().unwrap_or(rest);
            if !host.is_empty() {
                return host.chars().take(24).collect();
            }
        }
        url.chars().take(24).collect()
    }

    pub fn is_new_tab(&self) -> bool {
        let url = self.url.trim();
        url.is_empty() || url == "about:blank"
    }

    pub fn is_loading(&self) -> bool {
        self.is_loading
    }

    #[allow(dead_code)] // session restore / automation (Phase 4.3–4.4)
    pub fn history(&self) -> &[String] {
        &self.history
    }

    pub fn has_valid_bounds(&self) -> bool {
        self.bounds_egui.width() > 1.0 && self.bounds_egui.height() > 1.0
    }

    pub fn mark_shown_this_frame(&mut self) {
        self.shown_this_frame = true;
    }

    pub fn take_shown_this_frame(&mut self) -> bool {
        let shown = self.shown_this_frame;
        self.shown_this_frame = false;
        shown
    }

    fn push_history(&mut self, url: String) {
        if self.history.get(self.history_index).is_some_and(|u| u == &url) {
            return;
        }
        self.history.truncate(self.history_index + 1);
        self.history.push(url);
        self.history_index = self.history.len() - 1;
    }

    /// Normalize user-entered URL: bare hosts get `https://`.
    pub fn normalize_url(url: &str) -> String {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return DEFAULT_URL.to_string();
        }
        if Self::has_url_scheme(trimmed) {
            trimmed.to_string()
        } else {
            format!("https://{trimmed}")
        }
    }

    fn has_url_scheme(url: &str) -> bool {
        let Some(colon) = url.find(':') else {
            return false;
        };
        let scheme = &url[..colon];
        !scheme.is_empty()
            && !scheme.contains('/')
            && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    }

    pub fn navigate(&mut self, url: &str) -> Result<()> {
        let url = Self::normalize_url(url);
        if self.engine.is_ready() {
            self.engine.navigate(&url).context("engine navigate")?;
            self.is_loading = true;
        }
        self.url.clone_from(&url);
        self.push_history(url.clone());
        info!(%url, engine = self.engine.kind().as_str(), "Browser navigated");
        Ok(())
    }

    pub fn go_back(&mut self) -> Result<()> {
        if self.history_index > 0 {
            self.history_index -= 1;
            let url = self.history[self.history_index].clone();
            if self.engine.is_ready() {
                self.engine.navigate(&url).context("engine go_back")?;
                self.is_loading = true;
            }
            self.url.clone_from(&url);
            debug!(%url, "Browser went back");
        }
        Ok(())
    }

    pub fn go_forward(&mut self) -> Result<()> {
        if self.history_index < self.history.len() - 1 {
            self.history_index += 1;
            let url = self.history[self.history_index].clone();
            if self.engine.is_ready() {
                self.engine.navigate(&url).context("engine go_forward")?;
                self.is_loading = true;
            }
            self.url.clone_from(&url);
            debug!(%url, "Browser went forward");
        }
        Ok(())
    }

    pub fn reload(&mut self) -> Result<()> {
        if self.engine.is_ready() {
            self.engine.reload().context("engine reload")?;
            self.is_loading = true;
        }
        debug!("Browser reloaded");
        Ok(())
    }

    pub fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.history_index < self.history.len() - 1
    }

    #[allow(dead_code)]
    pub fn evaluate_javascript(&mut self, script: &str) -> Result<()> {
        self.engine.evaluate_script(script)
    }

    pub fn evaluate_javascript_async(
        &mut self,
        script: &str,
    ) -> Result<std::sync::mpsc::Receiver<String>> {
        self.engine.evaluate_script_async(script)
    }

    pub fn run_automation_async(
        &mut self,
        script: &str,
    ) -> Result<std::sync::mpsc::Receiver<String>> {
        self.evaluate_javascript_async(script)
    }

    /// Update content-area bounds (below chrome + URL bar only).
    pub fn set_bounds_scaled(&mut self, bounds: Rect, pixels_per_point: f32) {
        let ppp = if pixels_per_point.is_finite() && pixels_per_point > 0.0 {
            pixels_per_point
        } else {
            1.0
        };
        self.bounds_egui = bounds;
        self.pixels_per_point = ppp;
        self.engine.set_bounds(bounds, ppp);
    }

    /// Attach the compiled engine to the host window (lazy).
    pub fn create_webview(
        &mut self,
        window: &impl raw_window_handle::HasWindowHandle,
    ) -> Result<()> {
        if self.is_open {
            return Ok(());
        }
        if !self.has_valid_bounds() {
            return Ok(());
        }
        // Avoid per-frame attach storms after a hard failure (e.g. Chromium stub).
        if self.attach_failed && self.bounds_egui == self.last_attach_bounds {
            return Ok(());
        }

        self.engine.set_bounds(self.bounds_egui, self.pixels_per_point);
        self.last_attach_bounds = self.bounds_egui;

        let (url_tx, url_rx) = std::sync::mpsc::channel();
        let (title_tx, title_rx) = std::sync::mpsc::channel();
        let (load_tx, load_rx) = std::sync::mpsc::channel();
        let hooks = EngineNavHooks {
            url_tx: Some(url_tx),
            title_tx: Some(title_tx),
            loading_tx: Some(load_tx),
        };

        match self.engine.ensure_attached(window, &self.url, hooks) {
            Ok(()) => {
                self.attach_failed = false;
                self.nav_url_rx = Some(url_rx);
                self.nav_title_rx = Some(title_rx);
                self.nav_load_rx = Some(load_rx);
                self.is_open = self.engine.is_ready();
                if !self.is_open {
                    self.nav_url_rx = None;
                    self.nav_title_rx = None;
                    self.nav_load_rx = None;
                }
                self.is_loading = false;
                info!(
                    url = %self.url,
                    engine = self.engine.kind().as_str(),
                    ready = self.is_open,
                    "Browser engine attach finished"
                );
                Ok(())
            }
            Err(e) => {
                self.attach_failed = true;
                self.is_loading = false;
                self.nav_url_rx = None;
                self.nav_title_rx = None;
                self.nav_load_rx = None;
                Err(e).context("engine ensure_attached")
            }
        }
    }

    #[allow(dead_code)]
    pub fn destroy_webview(&mut self) {
        self.engine.destroy();
        self.is_open = false;
        self.attach_failed = false;
        self.nav_url_rx = None;
        self.nav_title_rx = None;
        self.nav_load_rx = None;
        self.osr_texture = None;
        self.osr_texture_size = (0, 0);
        debug!("Browser engine destroyed");
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.engine.set_visible(visible);
    }

    pub fn reposition_webview(&mut self) {
        // Bounds are pushed on every `set_bounds_scaled`.
    }

    /// Drain engine navigation callbacks into pane state.
    pub fn poll_events(&mut self) {
        self.engine.poll_eval_result();

        let mut urls = Vec::new();
        let mut titles = Vec::new();
        let mut loadings = Vec::new();
        if let Some(rx) = self.nav_url_rx.as_ref() {
            while let Ok(url) = rx.try_recv() {
                urls.push(url);
            }
        }
        if let Some(rx) = self.nav_title_rx.as_ref() {
            while let Ok(title) = rx.try_recv() {
                titles.push(title);
            }
        }
        if let Some(rx) = self.nav_load_rx.as_ref() {
            while let Ok(loading) = rx.try_recv() {
                loadings.push(loading);
            }
        }
        for url in urls {
            // Ignore data: placeholder URLs for history display.
            if url.starts_with("data:") {
                continue;
            }
            if url != self.url {
                self.url.clone_from(&url);
                self.push_history(url);
            }
        }
        for title in titles {
            self.page_title = title;
        }
        for loading in loadings {
            self.is_loading = loading;
        }
    }

    /// OSR engines: latest frame for egui texture (Chromium E2).
    pub fn take_osr_frame(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        self.engine.take_frame_rgba()
    }

    /// Upload latest OSR frame to an egui texture (Chromium path).
    pub fn update_osr_texture(&mut self, ctx: &egui::Context) {
        let Some((w, h, rgba)) = self.take_osr_frame() else {
            return;
        };
        if w == 0 || h == 0 || rgba.len() < (w as usize) * (h as usize) * 4 {
            return;
        }
        let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
        if let Some(tex) = self.osr_texture.as_mut()
            && self.osr_texture_size == (w, h)
        {
            tex.set(image, egui::TextureOptions::LINEAR);
        } else {
            self.osr_texture =
                Some(ctx.load_texture("rmux-chromium-osr", image, egui::TextureOptions::LINEAR));
            self.osr_texture_size = (w, h);
        }
    }

    #[must_use]
    pub fn osr_texture_id(&self) -> Option<egui::TextureId> {
        self.osr_texture.as_ref().map(egui::TextureHandle::id)
    }

    /// Forward input to Chromium OSR (no-op for OS webview).
    pub fn feed_osr_input(&mut self, webview_rect: Rect, ui: &egui::Ui, is_active: bool) {
        if self.engine.kind() != super::engine::EngineKind::Chromium || !self.is_open {
            return;
        }
        if !is_active {
            return;
        }
        // Skip when URL bar (or other text sink) owns the keyboard.
        let url_bar_focused = crate::ui::text_sink::is_active(ui.ctx());

        let pointer = ui.input(|i| i.pointer.clone());
        let hover = pointer.hover_pos();
        let Some(pos) = hover else {
            return;
        };
        if !webview_rect.contains(pos) {
            return;
        }
        let local = pos - webview_rect.min;
        let x = local.x;
        let y = local.y;
        let mods = 0u32; // CEF event flags; expand later if needed

        self.engine.send_mouse_move(x, y, mods);

        if pointer.primary_pressed() {
            self.engine.send_mouse_click(x, y, 0, true, mods);
        }
        if pointer.primary_released() {
            self.engine.send_mouse_click(x, y, 0, false, mods);
        }
        if pointer.secondary_pressed() {
            self.engine.send_mouse_click(x, y, 1, true, mods);
        }
        if pointer.secondary_released() {
            self.engine.send_mouse_click(x, y, 1, false, mods);
        }

        let scroll = ui.input(|i| i.raw_scroll_delta);
        if scroll != egui::Vec2::ZERO {
            self.engine.send_mouse_wheel(x, y, scroll.x, scroll.y, mods);
        }

        if !url_bar_focused {
            let chars: Vec<char> = ui.input(|i| {
                i.events
                    .iter()
                    .filter_map(|ev| {
                        if let egui::Event::Text(t) = ev {
                            Some(t.chars().collect::<Vec<_>>())
                        } else {
                            None
                        }
                    })
                    .flatten()
                    .collect()
            });
            for ch in chars {
                self.engine.send_key_char(ch, mods);
            }
        }
    }
}

impl Default for BrowserPane {
    fn default() -> Self {
        Self::new()
    }
}

/// Height of the address-bar toolbar (egui-only; native webview starts below).
pub const URL_TOOLBAR_HEIGHT: f32 = 36.0_f32;

/// Render browser body: **URL toolbar (egui)** + engine content area.
pub(crate) fn render_browser_pane(
    ui: &mut egui::Ui,
    pane_id: crate::workspace::splits::PaneId,
    browser: &mut BrowserPane,
    rect: egui::Rect,
    is_active: bool,
    active_pane: &mut crate::workspace::splits::PaneId,
) {
    let palette = crate::ui::theme::palette();

    let sense = ui.interact(rect, ui.id().with(("browser_pane", pane_id.0)), egui::Sense::click());
    if sense.clicked() && !is_active {
        *active_pane = pane_id;
    }

    let mut child_ui =
        ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::default()));

    child_ui.painter().rect_filled(rect, 0.0_f32, palette.app_bg);

    let toolbar_h = URL_TOOLBAR_HEIGHT;
    let toolbar_rect =
        egui::Rect::from_min_size(rect.left_top(), egui::Vec2::new(rect.width(), toolbar_h));
    let webview_rect = egui::Rect::from_min_size(
        rect.left_top() + egui::Vec2::new(0.0_f32, toolbar_h),
        egui::Vec2::new(rect.width(), (rect.height() - toolbar_h).max(0.0_f32)),
    );

    child_ui.painter().rect_filled(toolbar_rect, 0.0_f32, palette.chrome_bg);
    child_ui.painter().hline(
        toolbar_rect.x_range(),
        toolbar_rect.bottom() - 0.5_f32,
        egui::Stroke::new(1.0_f32, palette.chrome_border),
    );

    child_ui.allocate_new_ui(
        egui::UiBuilder::new().max_rect(toolbar_rect.shrink2(egui::Vec2::new(6.0_f32, 5.0_f32))),
        |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0_f32;

                let nav_size = egui::Vec2::new(26.0_f32, 24.0_f32);
                let back_enabled = browser.can_go_back();
                if ui
                    .add_enabled(
                        back_enabled,
                        egui::Button::new(egui::RichText::new("\u{2190}").size(14.0_f32))
                            .min_size(nav_size),
                    )
                    .on_hover_text("Back")
                    .clicked()
                {
                    let _ = browser.go_back();
                }

                let fwd_enabled = browser.can_go_forward();
                if ui
                    .add_enabled(
                        fwd_enabled,
                        egui::Button::new(egui::RichText::new("\u{2192}").size(14.0_f32))
                            .min_size(nav_size),
                    )
                    .on_hover_text("Forward")
                    .clicked()
                {
                    let _ = browser.go_forward();
                }

                let reload_label = if browser.is_loading() { "\u{2715}" } else { "\u{21BB}" };
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new(reload_label).size(14.0_f32))
                            .min_size(nav_size),
                    )
                    .on_hover_text("Reload")
                    .clicked()
                {
                    let _ = browser.reload();
                }

                let mut url =
                    if browser.is_new_tab() { String::new() } else { browser.url().to_string() };
                let url_id = ui.id().with(("browser_url", pane_id.0));
                let field_w = (ui.available_width() - 4.0_f32).max(80.0_f32);
                let url_response = ui.add_sized(
                    egui::Vec2::new(field_w, 24.0_f32),
                    egui::TextEdit::singleline(&mut url)
                        .id(url_id)
                        .font(egui::FontId::proportional(13.0_f32))
                        .desired_width(f32::INFINITY)
                        .hint_text("Search or enter URL")
                        .frame(true),
                );

                if browser.focus_url_bar {
                    ui.memory_mut(|mem| mem.request_focus(url_id));
                    browser.focus_url_bar = false;
                }

                if url_response.has_focus() {
                    crate::ui::text_sink::mark_active(ui.ctx());
                }

                let submit = (url_response.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    || (url_response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                if submit && !url.trim().is_empty() {
                    let _ = browser.navigate(&url);
                }
            });
        },
    );

    // Content area: OS webview paints natively; Chromium OSR draws an egui texture.
    if !browser.is_open() {
        child_ui.painter().rect_filled(webview_rect, 0.0_f32, palette.app_bg);
        child_ui.painter().text(
            webview_rect.center(),
            egui::Align2::CENTER_CENTER,
            if browser.has_valid_bounds() {
                "Starting webview\u{2026}"
            } else {
                "Waiting for layout\u{2026}"
            },
            egui::FontId::proportional(12.0_f32),
            palette.text_muted,
        );
    } else if browser.engine_kind() == super::engine::EngineKind::Chromium {
        browser.update_osr_texture(ui.ctx());
        if let Some(tex_id) = browser.osr_texture_id() {
            child_ui.painter().image(
                tex_id,
                webview_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            child_ui.painter().rect_filled(webview_rect, 0.0_f32, palette.app_bg);
            child_ui.painter().text(
                webview_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Loading Chromium\u{2026}",
                egui::FontId::proportional(12.0_f32),
                palette.text_muted,
            );
        }
        browser.feed_osr_input(webview_rect, ui, is_active);
    }

    let ppp = ui.ctx().pixels_per_point();
    browser.set_bounds_scaled(webview_rect, ppp);
    browser.mark_shown_this_frame();
    let _ = is_active;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_url_prepends_https() {
        assert_eq!(BrowserPane::normalize_url("example.com"), "https://example.com");
        assert_eq!(BrowserPane::normalize_url("  example.com/x  "), "https://example.com/x");
    }

    #[test]
    fn normalize_url_keeps_scheme() {
        assert_eq!(BrowserPane::normalize_url("http://localhost:3000"), "http://localhost:3000");
        assert_eq!(BrowserPane::normalize_url("about:blank"), "about:blank");
    }

    #[test]
    fn normalize_url_empty_is_blank() {
        assert_eq!(BrowserPane::normalize_url(""), "about:blank");
        assert_eq!(BrowserPane::normalize_url("   "), "about:blank");
    }

    #[test]
    fn navigate_updates_history_without_webview() {
        let mut b = BrowserPane::new();
        assert_eq!(b.url(), "about:blank");
        assert!(!b.can_go_back());

        b.navigate("example.com").unwrap();
        assert_eq!(b.url(), "https://example.com");
        assert!(b.can_go_back());
        assert!(!b.can_go_forward());

        b.navigate("https://rust-lang.org").unwrap();
        assert_eq!(b.url(), "https://rust-lang.org");
        assert_eq!(b.history().len(), 3);

        b.go_back().unwrap();
        assert_eq!(b.url(), "https://example.com");
        assert!(b.can_go_forward());

        b.go_forward().unwrap();
        assert_eq!(b.url(), "https://rust-lang.org");
    }

    #[test]
    fn navigate_same_url_does_not_duplicate_history() {
        let mut b = BrowserPane::new();
        b.navigate("https://example.com").unwrap();
        let len = b.history().len();
        b.navigate("https://example.com").unwrap();
        assert_eq!(b.history().len(), len);
    }

    #[test]
    fn go_back_truncates_forward_on_new_nav() {
        let mut b = BrowserPane::new();
        b.navigate("https://a.example").unwrap();
        b.navigate("https://b.example").unwrap();
        b.go_back().unwrap();
        assert!(b.can_go_forward());
        b.navigate("https://c.example").unwrap();
        assert!(!b.can_go_forward());
        assert_eq!(b.url(), "https://c.example");
        assert_eq!(
            b.history(),
            &[
                "about:blank".to_string(),
                "https://a.example".to_string(),
                "https://c.example".to_string()
            ]
        );
    }

    #[test]
    fn shown_flag_lifecycle() {
        let mut b = BrowserPane::new();
        assert!(!b.take_shown_this_frame());
        b.mark_shown_this_frame();
        assert!(b.take_shown_this_frame());
        assert!(!b.take_shown_this_frame());
    }

    #[test]
    fn valid_bounds_requires_non_trivial_size() {
        let mut b = BrowserPane::new();
        assert!(!b.has_valid_bounds());
        b.set_bounds_scaled(
            Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 80.0)),
            2.0,
        );
        assert!(b.has_valid_bounds());
    }

    #[test]
    fn tab_title_new_tab_when_blank() {
        let b = BrowserPane::new();
        assert_eq!(b.tab_title(), "New tab");
        assert!(b.is_new_tab());
    }

    #[test]
    fn tab_title_uses_host() {
        let mut b = BrowserPane::new();
        b.navigate("https://example.com/path").unwrap();
        assert_eq!(b.tab_title(), "example.com");
        assert!(!b.is_new_tab());
    }

    #[test]
    fn tab_title_prefers_page_title() {
        let mut b = BrowserPane::new();
        b.navigate("https://example.com").unwrap();
        b.page_title = "Example Domain".into();
        assert_eq!(b.tab_title(), "Example Domain");
    }

    #[test]
    fn pane_uses_compiled_engine_backend() {
        let b = BrowserPane::new();
        assert_eq!(b.engine_kind(), crate::browser::engine::EngineKind::compiled());
        assert!(!b.is_open());
    }
}
