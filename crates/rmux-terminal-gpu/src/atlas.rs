//! CPU glyph atlas (G1): rasterize with fontdue, pack into an RGBA8 texture.
//!
//! Quality notes:
//! - Glyphs are rasterized at `font_size * pixels_per_point` so HiDPI stays sharp.
//! - Font cascade: JetBrains regular/bold → optional Nerd symbols (icons).
//! - Coverage is gamma-adjusted so AA edges look less muddy on dark UIs.

use std::collections::HashMap;

use fontdue::Font;

/// Matches the CPU renderer’s line-height pad so cell metrics stay aligned.
const LINE_HEIGHT_PAD: f32 = 1.15;

const ATLAS_SIZE: u32 = 2048;
const PADDING: u32 = 1;
/// Light gamma on coverage (sRGB-ish) so thin stems stay readable.
const COVERAGE_GAMMA: f32 = 1.45;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct GlyphKey {
    c: char,
    bold: bool,
    /// Quantized pixels_per_point * 100 (atlas entries are scale-specific).
    ppp_x100: u16,
}

/// Packed glyph sample + placement inside a cell (points, top-left origin).
#[derive(Clone, Copy, Debug)]
pub(crate) struct GlyphEntry {
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    /// Offset from cell top-left to glyph bitmap top-left (points).
    pub ox: f32,
    pub oy: f32,
    pub gw: f32,
    pub gh: f32,
    pub has_ink: bool,
}

pub(crate) struct GlyphAtlas {
    font_regular: Font,
    font_bold: Font,
    /// Optional symbols / Nerd Font for PUA icons (LazyVim, powerline, …).
    font_symbols: Option<Font>,
    font_size: f32,
    /// Last pixels_per_point used for cell metrics / default keying.
    pixels_per_point: f32,
    /// Measured cell size in **points** (egui logical pixels).
    pub cell_w: f32,
    pub cell_h: f32,
    /// Distance from cell top to baseline (points).
    pub baseline: f32,
    width: u32,
    height: u32,
    /// CPU-side RGBA8 pixels (premultiplied white glyphs).
    pixels: Vec<u8>,
    shelf_x: u32,
    shelf_y: u32,
    shelf_h: u32,
    cache: HashMap<GlyphKey, GlyphEntry>,
    /// Texture must be re-uploaded after new glyphs.
    pub dirty: bool,
}

impl GlyphAtlas {
    pub fn new(
        font_regular_bytes: &[u8],
        font_bold_bytes: &[u8],
        font_symbols_bytes: Option<&[u8]>,
        font_size: f32,
    ) -> Result<Self, String> {
        let settings =
            fontdue::FontSettings { collection_index: 0, scale: 100.0, load_substitutions: true };
        let font_regular = Font::from_bytes(font_regular_bytes, settings)
            .map_err(|e| format!("regular font: {e}"))?;
        let font_bold =
            Font::from_bytes(font_bold_bytes, settings).map_err(|e| format!("bold font: {e}"))?;
        let font_symbols =
            font_symbols_bytes.and_then(|bytes| match Font::from_bytes(bytes, settings) {
                Ok(f) => Some(f),
                Err(e) => {
                    tracing::warn!(error = %e, "symbol font load failed; icons may tofu");
                    None
                }
            });

        let font_size = font_size.max(1.0);
        let pixels_per_point = 1.0;
        let (cell_w, cell_h, baseline) = measure_cell(&font_regular, font_size);

        let width = ATLAS_SIZE;
        let height = ATLAS_SIZE;
        let pixels = vec![0u8; (width * height * 4) as usize];

        Ok(Self {
            font_regular,
            font_bold,
            font_symbols,
            font_size,
            pixels_per_point,
            cell_w,
            cell_h,
            baseline,
            width,
            height,
            pixels,
            shelf_x: PADDING,
            shelf_y: PADDING,
            shelf_h: 0,
            cache: HashMap::with_capacity(512),
            dirty: true,
        })
    }

    pub fn set_font_size(&mut self, font_size: f32) {
        let font_size = font_size.max(1.0);
        if (font_size - self.font_size).abs() < 0.01 {
            return;
        }
        self.font_size = font_size;
        let (cell_w, cell_h, baseline) = measure_cell(&self.font_regular, font_size);
        self.cell_w = cell_w;
        self.cell_h = cell_h;
        self.baseline = baseline;
        self.clear();
    }

    /// Update HiDPI scale. Clears the atlas when the quantized scale changes.
    pub fn set_pixels_per_point(&mut self, ppp: f32) {
        let ppp = ppp.clamp(0.5, 4.0);
        let old_q = quantize_ppp(self.pixels_per_point);
        let new_q = quantize_ppp(ppp);
        self.pixels_per_point = ppp;
        if old_q != new_q {
            self.clear();
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(0);
        self.shelf_x = PADDING;
        self.shelf_y = PADDING;
        self.shelf_h = 0;
        self.cache.clear();
        self.dirty = true;
    }

    pub fn rgba(&self) -> &[u8] {
        &self.pixels
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn glyph(&mut self, c: char, bold: bool) -> GlyphEntry {
        let key = GlyphKey { c, bold, ppp_x100: quantize_ppp(self.pixels_per_point) };
        if let Some(e) = self.cache.get(&key) {
            return *e;
        }
        let entry = self.rasterize_and_pack(c, bold);
        self.cache.insert(key, entry);
        entry
    }

    fn rasterize_and_pack(&mut self, c: char, bold: bool) -> GlyphEntry {
        let empty = GlyphEntry {
            uv_min: [0.0, 0.0],
            uv_max: [0.0, 0.0],
            ox: 0.0,
            oy: 0.0,
            gw: 0.0,
            gh: 0.0,
            has_ink: false,
        };

        if c == ' ' || c == '\t' || c == '\0' || c == '\n' {
            return empty;
        }

        let ppp = self.pixels_per_point.max(0.5);
        // Rasterize in **physical pixels** for sharpness on HiDPI.
        let px_size = (self.font_size * ppp).max(1.0);

        let (metrics, bitmap, used_symbols) = self.rasterize_cascade(c, bold, px_size);
        if metrics.width == 0 || metrics.height == 0 || bitmap.is_empty() {
            return empty;
        }

        let gw = metrics.width as u32;
        let gh = metrics.height as u32;
        let need_w = gw + PADDING;
        let need_h = gh + PADDING;

        if self.shelf_x + need_w >= self.width {
            self.shelf_x = PADDING;
            self.shelf_y += self.shelf_h + PADDING;
            self.shelf_h = 0;
        }
        if self.shelf_y + need_h >= self.height {
            tracing::warn!("glyph atlas full; clearing cache");
            self.pixels.fill(0);
            self.shelf_x = PADDING;
            self.shelf_y = PADDING;
            self.shelf_h = 0;
            self.cache.clear();
            self.dirty = true;
            if self.shelf_y + need_h >= self.height {
                return empty;
            }
        }

        let x = self.shelf_x;
        let y = self.shelf_y;
        self.shelf_x += need_w;
        self.shelf_h = self.shelf_h.max(need_h);

        for row in 0..gh {
            for col in 0..gw {
                let raw = bitmap[(row * gw + col) as usize];
                if raw == 0 {
                    continue;
                }
                // Gamma lift: keep stems opaque enough on dark + wallpaper.
                let t = f32::from(raw) / 255.0;
                let cov = (t.powf(1.0 / COVERAGE_GAMMA) * 255.0).round().clamp(0.0, 255.0) as u8;
                let px = (x + col) as usize;
                let py = (y + row) as usize;
                let i = (py * self.width as usize + px) * 4;
                self.pixels[i] = cov;
                self.pixels[i + 1] = cov;
                self.pixels[i + 2] = cov;
                self.pixels[i + 3] = cov;
            }
        }
        self.dirty = true;

        let u0 = x as f32 / self.width as f32;
        let v0 = y as f32 / self.height as f32;
        let u1 = (x + gw) as f32 / self.width as f32;
        let v1 = (y + gh) as f32 / self.height as f32;

        // Convert bitmap metrics (physical px) → layout points.
        let ox = metrics.xmin as f32 / ppp;
        let oy = self.baseline - (metrics.height as f32 + metrics.ymin as f32) / ppp;
        let _ = used_symbols;

        GlyphEntry {
            uv_min: [u0, v0],
            uv_max: [u1, v1],
            ox,
            oy,
            gw: metrics.width as f32 / ppp,
            gh: metrics.height as f32 / ppp,
            has_ink: true,
        }
    }

    /// Prefer primary mono face; fall back to symbols font for missing ink
    /// (Nerd PUA icons, etc.).
    fn rasterize_cascade(
        &self,
        c: char,
        bold: bool,
        px_size: f32,
    ) -> (fontdue::Metrics, Vec<u8>, bool) {
        let primary = if bold { &self.font_bold } else { &self.font_regular };
        let (metrics, bitmap) = primary.rasterize(c, px_size);
        if metrics.width > 0 && metrics.height > 0 && !bitmap.is_empty() {
            // fontdue returns a box for .notdef sometimes; treat near-empty as miss.
            let ink: u32 = bitmap.iter().map(|&b| u32::from(b)).sum();
            if ink > 64 {
                return (metrics, bitmap, false);
            }
        }
        if let Some(sym) = self.font_symbols.as_ref() {
            let (m, b) = sym.rasterize(c, px_size);
            if m.width > 0 && m.height > 0 && !b.is_empty() {
                return (m, b, true);
            }
        }
        (metrics, bitmap, false)
    }
}

fn quantize_ppp(ppp: f32) -> u16 {
    (ppp * 100.0).round().clamp(50.0, 400.0) as u16
}

fn measure_cell(font: &Font, font_size: f32) -> (f32, f32, f32) {
    let (metrics, _) = font.rasterize('M', font_size);
    let line = font.horizontal_line_metrics(font_size);
    let ascent = line.map(|l| l.ascent).unwrap_or(font_size * 0.8);
    let descent = line.map(|l| l.descent.abs()).unwrap_or(font_size * 0.2);
    let cell_w = metrics.advance_width.max(1.0);
    let natural = ascent + descent;
    let cell_h = (natural * LINE_HEIGHT_PAD).max(font_size);
    let pad = (cell_h - natural) * 0.5;
    let baseline = pad + ascent;
    (cell_w, cell_h, baseline)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT: &[u8] = include_bytes!("../../rmux-app/assets/fonts/JetBrainsMono-Regular.ttf");
    const FONT_BOLD: &[u8] = include_bytes!("../../rmux-app/assets/fonts/JetBrainsMono-Bold.ttf");
    const NERD: &[u8] =
        include_bytes!("../../rmux-app/assets/fonts/SymbolsNerdFontMono-Regular.ttf");

    #[test]
    fn atlas_rasterizes_ascii_with_ink() {
        let mut atlas = GlyphAtlas::new(FONT, FONT_BOLD, Some(NERD), 14.0).expect("font load");
        atlas.set_pixels_per_point(2.0);
        let g = atlas.glyph('A', false);
        assert!(g.has_ink, "expected ink for 'A'");
        assert!(g.gw > 0.0 && g.gh > 0.0);
        assert!(g.uv_max[0] > g.uv_min[0]);
    }

    #[test]
    fn space_has_no_ink() {
        let mut atlas = GlyphAtlas::new(FONT, FONT_BOLD, Some(NERD), 14.0).expect("font load");
        let g = atlas.glyph(' ', false);
        assert!(!g.has_ink);
    }

    #[test]
    fn nerd_pua_has_ink() {
        let mut atlas = GlyphAtlas::new(FONT, FONT_BOLD, Some(NERD), 14.0).expect("font load");
        // Typical Nerd Font folder icon in PUA.
        let g = atlas.glyph('\u{e5fe}', false);
        assert!(g.has_ink, "expected Nerd Font symbol ink");
    }
}
