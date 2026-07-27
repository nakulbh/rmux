//! CPU glyph atlas (G1): rasterize with fontdue, pack into an RGBA8 texture.

use std::collections::HashMap;

use fontdue::Font;

/// Matches the CPU renderer’s line-height pad so cell metrics stay aligned.
const LINE_HEIGHT_PAD: f32 = 1.15;

const ATLAS_SIZE: u32 = 2048;
const PADDING: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct GlyphKey {
    c: char,
    bold: bool,
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
    font_size: f32,
    /// Measured cell size in points (egui logical pixels).
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
        font_size: f32,
    ) -> Result<Self, String> {
        let settings =
            fontdue::FontSettings { collection_index: 0, scale: 100.0, load_substitutions: true };
        let font_regular = Font::from_bytes(font_regular_bytes, settings)
            .map_err(|e| format!("regular font: {e}"))?;
        let font_bold =
            Font::from_bytes(font_bold_bytes, settings).map_err(|e| format!("bold font: {e}"))?;

        let font_size = font_size.max(1.0);
        let (cell_w, cell_h, baseline) = measure_cell(&font_regular, font_size);

        let width = ATLAS_SIZE;
        let height = ATLAS_SIZE;
        let pixels = vec![0u8; (width * height * 4) as usize];

        Ok(Self {
            font_regular,
            font_bold,
            font_size,
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
        let key = GlyphKey { c, bold };
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

        let font = if bold { &self.font_bold } else { &self.font_regular };
        let (metrics, bitmap) = font.rasterize(c, self.font_size);
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
                let cov = bitmap[(row * gw + col) as usize];
                if cov == 0 {
                    continue;
                }
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

        // fontdue layout (same as their image example):
        //   x = pen_x + metrics.xmin
        //   y = baseline - metrics.height - metrics.ymin
        // with ymin typically ≤ 0 (extent below the baseline).
        let ox = metrics.xmin as f32;
        let oy = self.baseline - metrics.height as f32 - metrics.ymin as f32;

        GlyphEntry {
            uv_min: [u0, v0],
            uv_max: [u1, v1],
            ox,
            oy,
            gw: metrics.width as f32,
            gh: metrics.height as f32,
            has_ink: true,
        }
    }
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

    #[test]
    fn atlas_rasterizes_ascii_with_ink() {
        let mut atlas = GlyphAtlas::new(FONT, FONT_BOLD, 14.0).expect("font load");
        let g = atlas.glyph('A', false);
        assert!(g.has_ink, "expected ink for 'A'");
        assert!(g.gw > 0.0 && g.gh > 0.0);
        assert!(g.uv_max[0] > g.uv_min[0]);
        assert!(g.uv_max[1] > g.uv_min[1]);
        let x0 = (g.uv_min[0] * atlas.width as f32) as u32;
        let y0 = (g.uv_min[1] * atlas.height as f32) as u32;
        let x1 = (g.uv_max[0] * atlas.width as f32) as u32;
        let y1 = (g.uv_max[1] * atlas.height as f32) as u32;
        let mut any = false;
        for y in y0..y1 {
            for x in x0..x1 {
                let i = (y * atlas.width + x) as usize * 4;
                if atlas.pixels[i + 3] > 0 {
                    any = true;
                    break;
                }
            }
            if any {
                break;
            }
        }
        assert!(any, "packed glyph region should contain coverage");
    }

    #[test]
    fn space_has_no_ink() {
        let mut atlas = GlyphAtlas::new(FONT, FONT_BOLD, 14.0).expect("font load");
        let g = atlas.glyph(' ', false);
        assert!(!g.has_ink);
    }
}
