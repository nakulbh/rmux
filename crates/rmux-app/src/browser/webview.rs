use anyhow::Result;
use egui::Rect;
use tracing::{debug, info};

const DEFAULT_URL: &str = "about:blank";

pub struct BrowserPane {
    url: String,
    history: Vec<String>,
    history_index: usize,
    webview: Option<wry::WebView>,
    needs_reposition: bool,
    bounds_egui: Rect,
    #[allow(dead_code)]
    is_loading: bool,
    #[allow(dead_code)]
    page_title: String,
    is_open: bool,
    /// Set by Cmd/Ctrl+L to request keyboard focus on the URL bar.
    pub focus_url_bar: bool,
}

impl BrowserPane {
    pub fn new() -> Self {
        Self {
            url: DEFAULT_URL.to_string(),
            history: vec![DEFAULT_URL.to_string()],
            history_index: 0,
            webview: None,
            needs_reposition: false,
            bounds_egui: Rect::ZERO,
            is_loading: false,
            page_title: String::new(),
            is_open: false,
            focus_url_bar: false,
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn set_open(&mut self, open: bool) {
        self.is_open = open;
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn title(&self) -> &str {
        &self.page_title
    }

    #[allow(dead_code)]
    pub fn is_loading(&self) -> bool {
        self.is_loading
    }

    #[allow(dead_code)]
    pub fn history(&self) -> &[String] {
        &self.history
    }

    fn push_history(&mut self, url: String) {
        self.history.truncate(self.history_index + 1);
        self.history.push(url);
        self.history_index = self.history.len() - 1;
    }

    pub fn navigate(&mut self, url: &str) -> Result<()> {
        let url = if !url.contains("://") { format!("https://{url}") } else { url.to_string() };

        if let Some(ref wv) = self.webview {
            wv.load_url(&url)?;
        }

        self.url.clone_from(&url);
        self.push_history(url.clone());

        info!(?url, "Browser navigated");
        Ok(())
    }

    pub fn go_back(&mut self) -> Result<()> {
        if self.history_index > 0 {
            self.history_index -= 1;
            let url = self.history[self.history_index].clone();
            if let Some(ref wv) = self.webview {
                wv.load_url(&url)?;
            }
            self.url.clone_from(&url);
            debug!(?url, "Browser went back");
        }
        Ok(())
    }

    pub fn go_forward(&mut self) -> Result<()> {
        if self.history_index < self.history.len() - 1 {
            self.history_index += 1;
            let url = self.history[self.history_index].clone();
            if let Some(ref wv) = self.webview {
                wv.load_url(&url)?;
            }
            self.url.clone_from(&url);
            debug!(?url, "Browser went forward");
        }
        Ok(())
    }

    pub fn reload(&mut self) -> Result<()> {
        if let Some(ref wv) = self.webview {
            wv.evaluate_script("location.reload()")?;
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
        if let Some(ref wv) = self.webview {
            wv.evaluate_script(script)?;
        }
        Ok(())
    }

    pub fn set_bounds(&mut self, bounds: Rect) {
        self.bounds_egui = bounds;
        self.needs_reposition = true;
    }

    fn to_wry_bounds(&self) -> wry::Rect {
        let pos = wry::dpi::Position::Logical(wry::dpi::LogicalPosition::new(
            self.bounds_egui.min.x as f64,
            self.bounds_egui.min.y as f64,
        ));
        let size = wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(
            self.bounds_egui.width() as f64,
            self.bounds_egui.height() as f64,
        ));
        wry::Rect { position: pos, size }
    }

    #[allow(dead_code)]
    pub fn create_webview(
        &mut self,
        window: &impl wry::raw_window_handle::HasWindowHandle,
    ) -> Result<()> {
        let webview = wry::WebViewBuilder::new()
            .with_url(&self.url)
            .with_bounds(self.to_wry_bounds())
            .build_as_child(window)?;

        self.webview = Some(webview);
        self.is_open = true;
        info!("Browser webview created");
        Ok(())
    }

    #[allow(dead_code)]
    pub fn destroy_webview(&mut self) {
        self.webview = None;
        self.is_open = false;
        debug!("Browser webview destroyed");
    }

    pub fn reposition_webview(&mut self) {
        if let Some(ref wv) = self.webview
            && self.needs_reposition
        {
            let _ = wv.set_bounds(self.to_wry_bounds());
            self.needs_reposition = false;
        }
    }
}

impl Default for BrowserPane {
    fn default() -> Self {
        Self::new()
    }
}

/// Render a browser pane with navigation controls and webview area.
///
/// Called from the workspace view when a [`BrowserPane`] leaf is encountered.
pub(crate) fn render_browser_pane(
    ui: &mut egui::Ui,
    pane_id: crate::workspace::splits::PaneId,
    browser: &mut BrowserPane,
    rect: egui::Rect,
    is_active: bool,
    active_pane: &mut crate::workspace::splits::PaneId,
) {
    let palette = crate::ui::theme::palette();
    let mut child_ui =
        ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::default()));

    // Fill background
    child_ui.painter().rect_filled(rect, 0.0_f32, palette.panel_bg);

    if is_active {
        let painter = child_ui.painter();
        let border_rect = rect.shrink(0.5_f32);
        let glow = [
            (2.0_f32, palette.accent.gamma_multiply(0.4_f32)),
            (1.5_f32, palette.accent.gamma_multiply(0.7_f32)),
            (1.0_f32, palette.accent),
        ];
        for (width, color) in glow {
            painter.rect_stroke(
                border_rect,
                egui::CornerRadius::ZERO,
                egui::Stroke::new(width, color),
                egui::StrokeKind::Inside,
            );
        }
    }

    let toolbar_h = 32.0_f32;
    let toolbar_border = 1.0_f32;
    let toolbar_rect =
        egui::Rect::from_min_size(rect.left_top(), egui::Vec2::new(rect.width(), toolbar_h));
    let webview_rect = egui::Rect::from_min_size(
        rect.left_top() + egui::Vec2::new(0.0_f32, toolbar_h + toolbar_border),
        egui::Vec2::new(rect.width(), rect.height() - toolbar_h - toolbar_border),
    );

    // Toolbar background
    child_ui.painter().rect_filled(toolbar_rect, 0.0_f32, palette.chrome_bg);

    // Layout toolbar with egui widgets
    child_ui.allocate_new_ui(egui::UiBuilder::new().max_rect(toolbar_rect.shrink(4.0_f32)), |ui| {
        ui.horizontal(|ui| {
            // Back button
            let back_enabled = browser.can_go_back();
            let back_btn = egui::Button::new(egui::RichText::new("\u{2190}").size(14.0_f32))
                .min_size(egui::Vec2::new(24.0_f32, 22.0_f32));
            if ui.add_enabled(back_enabled, back_btn).clicked() {
                let _ = browser.go_back();
            }

            // Forward button
            let fwd_enabled = browser.can_go_forward();
            let fwd_btn = egui::Button::new(egui::RichText::new("\u{2192}").size(14.0_f32))
                .min_size(egui::Vec2::new(24.0_f32, 22.0_f32));
            if ui.add_enabled(fwd_enabled, fwd_btn).clicked() {
                let _ = browser.go_forward();
            }

            // Reload button
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("\u{21BB}").size(14.0_f32))
                        .min_size(egui::Vec2::new(24.0_f32, 22.0_f32)),
                )
                .clicked()
            {
                let _ = browser.reload();
            }

            // URL bar
            let mut url = browser.url().to_string();
            let url_id = ui.next_auto_id();
            let url_response = ui.add_sized(
                egui::Vec2::new(ui.available_width() - 4.0_f32, 22.0_f32),
                egui::TextEdit::singleline(&mut url)
                    .id(url_id)
                    .font(egui::FontId::proportional(12.0_f32))
                    .desired_width(f32::INFINITY),
            );

            // Cmd/Ctrl+L: request focus on URL bar
            if browser.focus_url_bar {
                ui.memory_mut(|mem| mem.request_focus(url_id));
                browser.focus_url_bar = false;
            }

            if url_response.has_focus() {
                // Keep browser URL typing out of the PTY.
                crate::ui::text_sink::mark_active(ui.ctx());
            }

            if url_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let _ = browser.navigate(&url);
            }
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) && url_response.has_focus() {
                let _ = browser.navigate(&url);
            }
        });
    });

    // Webview area placeholder / real webview
    if !browser.is_open() {
        child_ui.painter().rect_filled(webview_rect, 0.0_f32, palette.app_bg);
        child_ui.painter().rect_stroke(
            webview_rect.shrink(0.5_f32),
            egui::CornerRadius::ZERO,
            egui::Stroke::new(1.0_f32, palette.border),
            egui::StrokeKind::Inside,
        );
        child_ui.painter().text(
            webview_rect.center(),
            egui::Align2::CENTER_CENTER,
            "Waiting for webview...",
            egui::FontId::proportional(12.0_f32),
            palette.text_muted,
        );
    }

    // Update browser bounds for native webview positioning
    browser.set_bounds(webview_rect);
    browser.reposition_webview();

    // Set active pane on click
    if child_ui.response().clicked() && !is_active {
        *active_pane = pane_id;
    }
}
