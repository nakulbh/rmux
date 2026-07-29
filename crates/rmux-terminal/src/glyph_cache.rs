//! Per-glyph galley cache (Warp `CellGlyphCache` idea, egui-backed).
//!
//! egui bakes paint color into the galley mesh, so color is part of the key.

use egui::{Color32, FontId, Galley, Ui};
use std::collections::HashMap;
use std::sync::Arc;

/// Soft cap; on overflow we drop half (not full clear) to avoid a mid-frame cliff.
const GLYPH_CACHE_CAP: usize = 8192;

/// Cache key for a laid-out terminal glyph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct GlyphKey {
    pub(crate) c: char,
    pub(crate) bold: bool,
    /// Font size × 10 as fixed-point so 14.0 and 14.1 don't thrash.
    size_x10: u16,
    /// Packed RGBA (egui bakes color into the galley).
    pub(crate) color: u32,
}

impl GlyphKey {
    pub(crate) fn new(c: char, bold: bool, font_size: f32, color: Color32) -> Self {
        let size_x10 = (font_size * 10.0).round().clamp(1.0, 65535.0) as u16;
        let color = u32::from_le_bytes([color.r(), color.g(), color.b(), color.a()]);
        Self { c, bold, size_x10, color }
    }
}

#[derive(Default)]
pub(crate) struct GlyphCache {
    map: HashMap<GlyphKey, Arc<Galley>>,
    /// Memoised `has_glyph` answers per `(char, bold)`.
    coverage: HashMap<(char, bool), bool>,
}

impl GlyphCache {
    pub(crate) fn with_capacity(cap: usize) -> Self {
        Self { map: HashMap::with_capacity(cap), coverage: HashMap::with_capacity(cap) }
    }

    pub(crate) fn clear(&mut self) {
        self.map.clear();
        self.coverage.clear();
    }

    /// Whether the font cascade can render `c`, cached across frames.
    ///
    /// `Fonts::has_glyph` takes the shared font lock and walks the family's
    /// fallback chain. Box-drawing borders (agent prompt boxes) and Nerd Font
    /// icons (nvim file tree) are non-ASCII, so the uncached version ran that
    /// lookup for hundreds of cells *per frame* — the single largest cost in
    /// the paint loop for exactly the two workloads users report as laggy.
    pub(crate) fn has_glyph(
        &mut self,
        ui: &Ui,
        c: char,
        bold: bool,
        font_regular: &FontId,
        font_bold: &FontId,
    ) -> bool {
        if let Some(&known) = self.coverage.get(&(c, bold)) {
            return known;
        }
        let font_id = if bold { font_bold } else { font_regular };
        let has = ui.fonts(|f| f.has_glyph(font_id, c));
        self.coverage.insert((c, bold), has);
        has
    }

    pub(crate) fn get_or_layout(
        &mut self,
        ui: &Ui,
        key: GlyphKey,
        font_regular: &FontId,
        font_bold: &FontId,
    ) -> Arc<Galley> {
        if let Some(g) = self.map.get(&key) {
            return g.clone();
        }
        if self.map.len() >= GLYPH_CACHE_CAP {
            // Drop roughly half without full mid-frame wipe.
            let drop_n = self.map.len() / 2;
            let keys: Vec<GlyphKey> = self.map.keys().copied().take(drop_n).collect();
            for k in keys {
                self.map.remove(&k);
            }
        }
        let font_id = if key.bold { font_bold.clone() } else { font_regular.clone() };
        let color = {
            let [r, g, b, a] = key.color.to_le_bytes();
            Color32::from_rgba_unmultiplied(r, g, b, a)
        };
        let galley = ui.fonts(|f| f.layout_no_wrap(key.c.to_string(), font_id, color));
        self.map.insert(key, galley.clone());
        galley
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_key_differs_by_style_and_color() {
        let a = GlyphKey::new('a', false, 14.0, Color32::WHITE);
        let b = GlyphKey::new('a', true, 14.0, Color32::WHITE);
        let c = GlyphKey::new('a', false, 14.0, Color32::RED);
        let d = GlyphKey::new('a', false, 14.0, Color32::WHITE);
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_eq!(a, d);
    }

    #[test]
    fn clear_is_idempotent() {
        let mut cache = GlyphCache::with_capacity(8);
        cache.clear();
        cache.clear();
    }
}
