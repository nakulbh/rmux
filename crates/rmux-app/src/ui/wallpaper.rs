//! Workspace wallpaper — one shared background image for all terminal panes.
//!
//! Painted **inside** each translucent panel (central workspace + optional
//! glass sidebar), cover-fit against the full **screen** rect so the image
//! lines up continuously. Never painted on a free-floating background layer
//! (that was covering chrome and making the UI disappear).

use std::path::{Path, PathBuf};

use egui::{Color32, ColorImage, Pos2, Rect, TextureHandle, TextureOptions, Vec2};

/// Loads and paints a single wallpaper texture for the terminal workspace.
#[derive(Default)]
pub struct Wallpaper {
    /// Path that was last successfully loaded (expanded).
    loaded_path: Option<PathBuf>,
    /// GPU texture for the image, if loaded.
    texture: Option<TextureHandle>,
    /// Last load error message (shown in settings).
    last_error: Option<String>,
    /// Generation counter bumped on each load attempt (for cache invalidation).
    generation: u64,
}

impl Wallpaper {
    /// Create an empty wallpaper (no image).
    pub fn new() -> Self {
        Self::default()
    }

    /// Human-readable status for the settings panel.
    pub fn status_message(&self) -> Option<&str> {
        if self.texture.is_some() { None } else { self.last_error.as_deref() }
    }

    /// Whether a texture is ready to paint.
    pub fn is_ready(&self) -> bool {
        self.texture.is_some()
    }

    /// Clear the texture and forget the loaded path.
    pub fn clear(&mut self) {
        self.texture = None;
        self.loaded_path = None;
        self.last_error = None;
        self.generation = self.generation.wrapping_add(1);
    }

    /// Ensure the texture matches `path`. Reloads only when the path changes.
    ///
    /// `path` should already be tilde-expanded. Empty path clears the wallpaper.
    pub fn ensure_loaded(&mut self, ctx: &egui::Context, path: Option<&Path>) {
        let Some(path) = path else {
            if self.texture.is_some() || self.loaded_path.is_some() {
                self.clear();
            }
            return;
        };

        if path.as_os_str().is_empty() {
            self.clear();
            return;
        }

        if self.loaded_path.as_deref() == Some(path) && self.texture.is_some() {
            return;
        }

        self.load(ctx, path);
    }

    /// Force-reload from `path` (e.g. after the user clicks Apply).
    pub fn reload(&mut self, ctx: &egui::Context, path: &Path) {
        self.loaded_path = None;
        self.load(ctx, path);
    }

    fn load(&mut self, ctx: &egui::Context, path: &Path) {
        self.generation = self.generation.wrapping_add(1);
        match load_color_image(path) {
            Ok(color_image) => {
                let name = format!("rmux_wallpaper_{}", self.generation);
                let texture = ctx.load_texture(name, color_image, TextureOptions::LINEAR);
                self.texture = Some(texture);
                self.loaded_path = Some(path.to_path_buf());
                self.last_error = None;
                tracing::info!(path = %path.display(), "Loaded workspace wallpaper");
            }
            Err(err) => {
                self.texture = None;
                self.loaded_path = None;
                self.last_error = Some(err);
                tracing::warn!(
                    path = %path.display(),
                    error = %self.last_error.as_deref().unwrap_or("?"),
                    "Failed to load wallpaper"
                );
            }
        }
    }

    /// Paint into `clip`, cover-fit against the full `screen` rect.
    ///
    /// Call this from the sidebar and the central workspace with the same
    /// `screen` (`ctx.screen_rect()`) so both regions show one continuous image.
    pub fn paint_screen_aligned(&self, painter: &egui::Painter, clip: Rect, screen: Rect) {
        let Some(texture) = self.texture.as_ref() else {
            return;
        };
        if clip.width() <= 0.0 || clip.height() <= 0.0 {
            return;
        }

        let tex_size = texture.size_vec2();
        if tex_size.x <= 0.0 || tex_size.y <= 0.0 {
            return;
        }

        // Cover against the full window so sidebar + central stay aligned.
        let fit = if screen.width() > 0.0 && screen.height() > 0.0 { screen } else { clip };
        let scale = (fit.width() / tex_size.x).max(fit.height() / tex_size.y);
        let draw_w = tex_size.x * scale;
        let draw_h = tex_size.y * scale;
        let origin = Pos2::new(fit.center().x - draw_w * 0.5, fit.center().y - draw_h * 0.5);
        let image_rect = Rect::from_min_size(origin, Vec2::new(draw_w, draw_h));

        painter.with_clip_rect(clip).image(
            texture.id(),
            image_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }
}

/// Decode an image file into an egui [`ColorImage`].
fn load_color_image(path: &Path) -> Result<ColorImage, String> {
    if !path.exists() {
        return Err(format!("file not found: {}", path.display()));
    }
    let dyn_img = image::open(path).map_err(|e| format!("decode failed: {e}"))?;
    let rgba = dyn_img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimensions".into());
    }
    // Cap giant images so we don't blow VRAM (downscale keeping aspect).
    const MAX_EDGE: u32 = 4096;
    let (w, h, pixels) = if w > MAX_EDGE || h > MAX_EDGE {
        let scale = (MAX_EDGE as f32 / w as f32).min(MAX_EDGE as f32 / h as f32);
        let nw = ((w as f32) * scale).round().max(1.0) as u32;
        let nh = ((h as f32) * scale).round().max(1.0) as u32;
        let resized = image::imageops::resize(&rgba, nw, nh, image::imageops::FilterType::Triangle);
        (nw, nh, resized.into_raw())
    } else {
        (w, h, rgba.into_raw())
    };
    Ok(ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageBuffer;
    use std::fs;

    #[test]
    fn test_load_color_image_png() {
        let dir = std::env::temp_dir().join(format!("rmux-wall-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("tiny.png");
        let img: ImageBuffer<image::Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(4, 4, image::Rgba([10, 20, 30, 255]));
        img.save(&path).expect("write png");
        let color = load_color_image(&path).expect("load");
        assert_eq!(color.width(), 4);
        assert_eq!(color.height(), 4);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_missing_file() {
        let err = load_color_image(Path::new("/nonexistent/rmux-wallpaper-xyz.png")).unwrap_err();
        assert!(err.contains("not found"));
    }
}
