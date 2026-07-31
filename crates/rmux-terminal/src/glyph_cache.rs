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
    /// Identity of the font texture atlas the cached entries were produced
    /// against (see [`Self::ensure_fresh`]). `None` before the first frame.
    atlas_identity: Option<usize>,
}

impl GlyphCache {
    pub(crate) fn with_capacity(cap: usize) -> Self {
        Self {
            map: HashMap::with_capacity(cap),
            coverage: HashMap::with_capacity(cap),
            atlas_identity: None,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.map.clear();
        self.coverage.clear();
    }

    /// Drop every cached entry if egui recreated its font texture atlas
    /// since the last call. Call this once per `draw()`, before looking
    /// anything up.
    ///
    /// egui rebuilds the whole atlas (and its own internal galley cache)
    /// when `pixels_per_point` changes, `max_texture_side` changes, or the
    /// atlas fills past ~80% (`epaint::text::Fonts::begin_pass`). Any of
    /// those can happen after a display/DPI change, and in practice also
    /// after the OS suspends and resumes the GPU context — eframe rebuilds
    /// font textures on the way back up. Our own cached `Arc<Galley>`s bake
    /// in mesh UV coordinates for the *old* atlas packing; painting them
    /// against a freshly repacked atlas samples whatever now happens to
    /// live at those old coordinates, which is exactly what "all the text
    /// scrambled up after sleep" looks like. Comparing the atlas's `Arc`
    /// identity (not its contents) catches every recreation reason at once,
    /// on any platform, without needing to special-case sleep/wake events.
    pub(crate) fn ensure_fresh(&mut self, ui: &Ui) {
        let identity = ui.fonts(|f| Arc::as_ptr(&f.texture_atlas()) as *const () as usize);
        self.invalidate_if_changed(identity);
    }

    /// Core of [`Self::ensure_fresh`], split out so the invalidation logic
    /// is testable without a live `egui::Ui`.
    fn invalidate_if_changed(&mut self, atlas_identity: usize) {
        if self.atlas_identity.replace(atlas_identity) != Some(atlas_identity) {
            self.map.clear();
            self.coverage.clear();
        }
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
    fn same_atlas_identity_keeps_cached_entries() {
        let mut cache = GlyphCache::with_capacity(8);
        // Establish the baseline identity first — the very first call always
        // "changes" it (from `None`), which would otherwise wipe the entries
        // inserted below before the real assertion even runs.
        cache.invalidate_if_changed(0xDEAD_BEEF);
        cache.map.insert(GlyphKey::new('a', false, 14.0, Color32::WHITE), fake_galley());
        cache.coverage.insert(('a', false), true);

        cache.invalidate_if_changed(0xDEAD_BEEF);

        assert_eq!(cache.map.len(), 1, "same atlas identity must not evict entries");
        assert_eq!(cache.coverage.len(), 1);
    }

    #[test]
    fn atlas_identity_change_clears_cache() {
        // A resized/recreated font atlas (DPI change, or the GPU context
        // rebuilt after sleep/wake) invalidates every cached galley's UVs —
        // this is the guard against "scrambled text after resume".
        let mut cache = GlyphCache::with_capacity(8);
        cache.map.insert(GlyphKey::new('a', false, 14.0, Color32::WHITE), fake_galley());
        cache.coverage.insert(('a', false), true);
        cache.invalidate_if_changed(0x1111_1111);

        cache.invalidate_if_changed(0x2222_2222);

        assert!(cache.map.is_empty(), "atlas swap must evict stale galleys");
        assert!(cache.coverage.is_empty());
    }

    #[test]
    fn first_call_does_not_panic_on_empty_cache() {
        let mut cache = GlyphCache::with_capacity(8);
        cache.invalidate_if_changed(0x1234);
        assert!(cache.map.is_empty());
    }

    fn fake_galley() -> Arc<Galley> {
        let ctx = egui::Context::default();
        // Fonts aren't initialized until the first frame runs.
        let _ = ctx.run(Default::default(), |_| {});
        ctx.fonts(|f| f.layout_no_wrap("x".to_string(), FontId::monospace(14.0), Color32::WHITE))
    }

    #[test]
    fn clear_is_idempotent() {
        let mut cache = GlyphCache::with_capacity(8);
        cache.clear();
        cache.clear();
    }
}
