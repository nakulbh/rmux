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

    #[allow(dead_code)]
    pub fn set_open(&mut self, open: bool) {
        self.is_open = open;
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    #[allow(dead_code)]
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
