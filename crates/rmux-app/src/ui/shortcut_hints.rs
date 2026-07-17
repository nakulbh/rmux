//! cmux-style keyboard shortcut overlays.
//!
//! While the platform primary modifier is held (⌘ on macOS, Ctrl on
//! Linux/Windows), chrome controls paint a small floating badge with the
//! chord that activates them — matching cmux's "hold Command to see
//! bindings" discoverability affordance.

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, StrokeKind, vec2};

use crate::ui::theme;

/// True while the platform primary modifier is held and neither Shift nor
/// Alt is down.
///
/// Shift/Alt are excluded so multi-modifier chords (e.g. ⌘⇧D) don't leave
/// the single-key badges visible while the user is mid-chord. Bare Ctrl on
/// macOS is ignored — only Command triggers the overlay, matching cmux.
pub fn primary_mod_held(ctx: &egui::Context) -> bool {
    ctx.input(|i| {
        let m = i.modifiers;
        // egui's logical `command` is ⌘ on macOS and Ctrl on Linux/Windows.
        // Prefer it over platform-specific `ctrl` / `mac_cmd` checks.
        m.command && !m.shift && !m.alt
    })
}

/// Platform glyph for the primary modifier (`⌘` or `Ctrl+`).
pub fn mod_glyph() -> &'static str {
    if cfg!(target_os = "macos") { "\u{2318}" } else { "Ctrl+" }
}

/// Format a single-key chord label (`⌘B` / `Ctrl+B`).
pub fn chord(key: &str) -> String {
    format!("{}{key}", mod_glyph())
}

/// Format a chord with an extra modifier between primary and key
/// (e.g. `⌘⇧[` is not used here — kept for future multi-mod badges).
#[allow(dead_code)]
pub fn chord_with(extra: &str, key: &str) -> String {
    format!("{}{extra}{key}", mod_glyph())
}

/// Draw a compact dark pill badge centered on `center` (cmux toolbar style).
///
/// Kept intentionally small so the underlying icon still peeks around the
/// chip edges — cmux uses ~9px type with tight padding, not full-button
/// cover-ups.
pub fn draw_overlay_badge(ui: &mut egui::Ui, center: Pos2, label: &str) {
    let p = theme::palette();
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(9.0_f32),
        Color32::from_rgb(0xe8, 0xea, 0xee),
    );
    let pad_x = 3.0_f32;
    let pad_y = 1.5_f32;
    let size = vec2(galley.size().x + pad_x * 2.0_f32, galley.size().y + pad_y * 2.0_f32);
    let rect = Rect::from_center_size(center, size);

    let fill = Color32::from_rgb(0x2a, 0x2e, 0x36);
    let border = Color32::from_rgb(0x4a, 0x50, 0x5c);
    ui.painter().rect_filled(rect, CornerRadius::same(3), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(3),
        Stroke::new(1.0_f32, border),
        StrokeKind::Inside,
    );
    ui.painter().galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0_f32,
            rect.center().y - galley.size().y / 2.0_f32,
        ),
        galley,
        p.text_primary,
    );
}

/// Draw a small circular badge for sidebar workspace numbers (`⌘1`).
///
/// Placed at the right edge of a card. Active cards use the accent fill
/// (cmux blue chip); inactive use a muted fill.
pub fn draw_workspace_badge(ui: &mut egui::Ui, center: Pos2, index: usize, active: bool) {
    // Workspace switch shortcuts are Cmd/Ctrl+1..9 — only show 1..=9.
    if index >= 9 {
        return;
    }
    let p = theme::palette();
    let label = chord(&(index + 1).to_string());
    let galley = ui.painter().layout_no_wrap(
        label,
        FontId::proportional(9.0_f32),
        if active { p.accent_fg } else { p.text_primary },
    );
    let r = (galley.size().x / 2.0_f32 + 3.5_f32).max(9.0_f32);
    let fill = if active { p.accent } else { p.panel_bg };
    let border = if active { p.accent } else { p.border };
    ui.painter().circle_filled(center, r, fill);
    ui.painter().circle_stroke(center, r, Stroke::new(1.0_f32, border));
    ui.painter().galley(
        Pos2::new(center.x - galley.size().x / 2.0_f32, center.y - galley.size().y / 2.0_f32),
        galley,
        if active { p.accent_fg } else { p.text_primary },
    );
}

/// Convenience: center-aligned text badge using `Align2` (tests / callers).
#[allow(dead_code)]
pub fn badge_align() -> Align2 {
    Align2::CENTER_CENTER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chord_contains_key() {
        let label = chord("B");
        assert!(label.contains('B'), "label={label}");
        assert!(!label.is_empty());
    }

    #[test]
    fn test_mod_glyph_nonempty() {
        assert!(!mod_glyph().is_empty());
    }

    #[test]
    fn test_workspace_index_limit() {
        // draw_workspace_badge is a no-op for index >= 9; just document the
        // contract via the chord helper for keys 1..=9.
        for i in 1..=9 {
            let label = chord(&i.to_string());
            assert!(label.contains(&i.to_string()));
        }
    }
}
