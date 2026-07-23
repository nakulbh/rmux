//! OS webview backend (`wry`) — optional light fallback.
//!
//! Enable with `--features browser-os-webview` (disables default Chromium).
//! macOS: WKWebView · Windows: WebView2 · Linux: webkit2gtk

use anyhow::{Context, Result};
use egui::Rect;
use tracing::{debug, info, warn};

use super::engine::EngineNavHooks;

/// Dark empty document so new tabs match app chrome.
const NEW_TAB_HTML: &str = r#"<!DOCTYPE html>
<html><head><meta charset="utf-8">
<style>
  html,body{margin:0;padding:0;height:100%;background:#0c0c0c;color:#71717a;
  font-family:-apple-system,system-ui,sans-serif;}
  body{display:flex;align-items:center;justify-content:center;}
  .hint{opacity:0.55;font-size:13px;letter-spacing:0.02em;text-align:center;line-height:1.5;}
</style></head>
<body><div class="hint">Type a URL in the address bar above<br>and press Enter</div></body></html>"#;

pub struct OsWebviewEngine {
    webview: Option<wry::WebView>,
    bounds: Rect,
}

impl OsWebviewEngine {
    #[must_use]
    pub fn new() -> Self {
        Self { webview: None, bounds: Rect::ZERO }
    }

    fn to_wry_bounds(&self) -> wry::Rect {
        wry::Rect {
            position: wry::dpi::Position::Logical(wry::dpi::LogicalPosition::new(
                f64::from(self.bounds.min.x),
                f64::from(self.bounds.min.y),
            )),
            size: wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(
                f64::from(self.bounds.width()).max(1.0),
                f64::from(self.bounds.height()).max(1.0),
            )),
        }
    }

    fn has_valid_bounds(&self) -> bool {
        self.bounds.width() > 1.0 && self.bounds.height() > 1.0
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.webview.is_some()
    }

    pub fn navigate(&mut self, url: &str) -> Result<()> {
        if let Some(ref wv) = self.webview {
            wv.load_url(url).context("os webview load_url")?;
        }
        Ok(())
    }

    pub fn reload(&mut self) -> Result<()> {
        if let Some(ref wv) = self.webview {
            wv.reload().context("os webview reload")?;
        }
        Ok(())
    }

    pub fn set_bounds(&mut self, bounds: Rect, _pixels_per_point: f32) {
        self.bounds = bounds;
        if let Some(ref wv) = self.webview
            && let Err(e) = wv.set_bounds(self.to_wry_bounds())
        {
            warn!(error = %e, "os webview set_bounds failed");
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        if let Some(ref wv) = self.webview
            && let Err(e) = wv.set_visible(visible)
        {
            warn!(error = %e, "os webview set_visible failed");
        }
    }

    pub fn ensure_attached(
        &mut self,
        window: &impl raw_window_handle::HasWindowHandle,
        initial_url: &str,
        hooks: EngineNavHooks,
    ) -> Result<()> {
        if self.webview.is_some() {
            return Ok(());
        }
        if !self.has_valid_bounds() {
            return Ok(());
        }

        let is_blank = initial_url.is_empty()
            || initial_url == "about:blank"
            || initial_url.starts_with("data:text/html");

        let mut builder = if is_blank {
            wry::WebViewBuilder::new().with_html(NEW_TAB_HTML)
        } else {
            wry::WebViewBuilder::new().with_url(initial_url)
        };

        builder =
            builder.with_bounds(self.to_wry_bounds()).with_background_color((12, 12, 12, 255));

        if let Some(tx) = hooks.url_tx {
            builder = builder.with_navigation_handler(move |url| {
                let _ = tx.send(url);
                true
            });
        }
        if let Some(tx) = hooks.title_tx {
            builder = builder.with_document_title_changed_handler(move |title| {
                let _ = tx.send(title);
            });
        }
        if let Some(tx) = hooks.loading_tx {
            builder = builder.with_on_page_load_handler(move |event, _url| {
                let loading = matches!(event, wry::PageLoadEvent::Started);
                let _ = tx.send(loading);
            });
        }

        let webview = builder.build_as_child(window).context("failed to create wry OS webview")?;

        self.webview = Some(webview);
        info!(url = %initial_url, "OS webview attached via EngineBackend");
        Ok(())
    }

    pub fn destroy(&mut self) {
        self.webview = None;
        debug!("OS webview destroyed");
    }

    pub fn evaluate_script(&mut self, script: &str) -> Result<()> {
        let Some(ref wv) = self.webview else {
            anyhow::bail!("OS webview not ready");
        };
        wv.evaluate_script(script).context("evaluate_script")?;
        Ok(())
    }

    pub fn evaluate_script_async(
        &mut self,
        script: &str,
    ) -> Result<std::sync::mpsc::Receiver<String>> {
        let Some(ref wv) = self.webview else {
            anyhow::bail!("OS webview not ready");
        };
        let (tx, rx) = std::sync::mpsc::channel();
        wv.evaluate_script_with_callback(script, move |result| {
            let _ = tx.send(result);
        })
        .context("evaluate_script_with_callback")?;
        Ok(rx)
    }
}

impl Default for OsWebviewEngine {
    fn default() -> Self {
        Self::new()
    }
}
