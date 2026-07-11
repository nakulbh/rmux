#![allow(dead_code)]

use std::sync::Mutex;

use anyhow::Result;
use egui::Rect;
use tracing::{debug, info};

const DEFAULT_URL: &str = "about:blank";

pub struct BrowserPane {
    url: Mutex<String>,
    history: Mutex<Vec<String>>,
    history_index: Mutex<usize>,
    webview: Option<wry::WebView>,
    needs_reposition: bool,
    bounds: Rect,
    is_loading: Mutex<bool>,
    page_title: Mutex<String>,
    is_open: bool,
}

impl BrowserPane {
    pub fn new() -> Self {
        Self {
            url: Mutex::new(DEFAULT_URL.to_string()),
            history: Mutex::new(vec![DEFAULT_URL.to_string()]),
            history_index: Mutex::new(0),
            webview: None,
            needs_reposition: false,
            bounds: Rect::ZERO,
            is_loading: Mutex::new(false),
            page_title: Mutex::new(String::new()),
            is_open: false,
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn set_open(&mut self, open: bool) {
        self.is_open = open;
    }

    pub fn url(&self) -> String {
        self.url.lock().unwrap().clone()
    }

    pub fn title(&self) -> String {
        self.page_title.lock().unwrap().clone()
    }

    pub fn is_loading(&self) -> bool {
        *self.is_loading.lock().unwrap()
    }

    pub fn history(&self) -> Vec<String> {
        self.history.lock().unwrap().clone()
    }

    pub fn navigate(&mut self, url: &str) -> Result<()> {
        let url = if !url.contains("://") { format!("https://{url}") } else { url.to_string() };

        if let Some(ref wv) = self.webview {
            wv.load_url(&url)?;
        }

        self.url.lock().unwrap().clone_from(&url);

        let mut hist = self.history.lock().unwrap();
        let mut idx = self.history_index.lock().unwrap();
        hist.truncate(*idx + 1);
        hist.push(url.clone());
        *idx = hist.len() - 1;

        info!(?url, "Browser navigated");
        Ok(())
    }

    pub fn go_back(&mut self) -> Result<()> {
        let mut idx = self.history_index.lock().unwrap();
        if *idx > 0 {
            *idx -= 1;
            let url = self.history.lock().unwrap()[*idx].clone();
            drop(idx);
            if let Some(ref wv) = self.webview {
                wv.load_url(&url)?;
            }
            self.url.lock().unwrap().clone_from(&url);
            debug!(?url, "Browser went back");
        }
        Ok(())
    }

    pub fn go_forward(&mut self) -> Result<()> {
        let hist = self.history.lock().unwrap();
        let mut idx = self.history_index.lock().unwrap();
        if *idx < hist.len() - 1 {
            *idx += 1;
            let url = hist[*idx].clone();
            drop(idx);
            drop(hist);
            if let Some(ref wv) = self.webview {
                wv.load_url(&url)?;
            }
            self.url.lock().unwrap().clone_from(&url);
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
        *self.history_index.lock().unwrap() > 0
    }

    pub fn can_go_forward(&self) -> bool {
        *self.history_index.lock().unwrap() < self.history.lock().unwrap().len() - 1
    }

    pub fn evaluate_javascript(&mut self, script: &str) -> Result<()> {
        if let Some(ref wv) = self.webview {
            wv.evaluate_script(script)?;
        }
        Ok(())
    }

    pub fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
        self.needs_reposition = true;
    }

    pub fn create_webview(
        &mut self,
        window: &impl wry::raw_window_handle::HasWindowHandle,
    ) -> Result<()> {
        let url = { self.url.lock().unwrap().clone() };

        let webview = wry::WebViewBuilder::new()
            .with_url(&url)
            .with_bounds(wry::Rect {
                position: wry::dpi::Position::Logical(wry::dpi::LogicalPosition::new(
                    self.bounds.min.x as f64,
                    self.bounds.min.y as f64,
                )),
                size: wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(
                    self.bounds.width() as f64,
                    self.bounds.height() as f64,
                )),
            })
            .build_as_child(window)?;

        self.webview = Some(webview);
        self.is_open = true;
        info!("Browser webview created");
        Ok(())
    }

    pub fn destroy_webview(&mut self) {
        self.webview = None;
        self.is_open = false;
        debug!("Browser webview destroyed");
    }

    pub fn reposition_webview(&mut self) {
        if let Some(ref wv) = self.webview
            && self.needs_reposition
        {
            let _ = wv.set_bounds(wry::Rect {
                position: wry::dpi::Position::Logical(wry::dpi::LogicalPosition::new(
                    self.bounds.min.x as f64,
                    self.bounds.min.y as f64,
                )),
                size: wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(
                    self.bounds.width() as f64,
                    self.bounds.height() as f64,
                )),
            });
            self.needs_reposition = false;
        }
    }
}

impl Drop for BrowserPane {
    fn drop(&mut self) {
        self.destroy_webview();
    }
}

impl Default for BrowserPane {
    fn default() -> Self {
        Self::new()
    }
}
