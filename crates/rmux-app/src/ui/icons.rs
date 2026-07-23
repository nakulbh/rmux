//! Stroke icons painted with egui (no emoji / Nerd Font dependency).
//!
//! Shapes follow Lucide / SF-Symbols weight so they match the top-bar chrome.

use egui::{Color32, Pos2, Stroke, pos2};

/// Lucide-style globe: circle + vertical ellipse (meridian) + equator.
///
/// Matches:
/// `data:image/svg+xml,...lucid e-globe` — circle r=10, path meridian, path equator.
pub fn draw_globe(painter: &egui::Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.4_f32, color);
    let r = 6.5_f32;

    // Outer circle
    painter.circle_stroke(center, r, stroke);

    // Vertical meridian (ellipse-ish: two arcs approximated as a tall oval)
    // SVG: M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20
    // In our local coords: thin vertical ellipse through the circle.
    let mer_rx = r * 0.42_f32;
    let mer_ry = r;
    let steps = 16;
    let mut pts = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = std::f32::consts::TAU * (i as f32) / (steps as f32);
        pts.push(pos2(center.x + mer_rx * t.sin(), center.y - mer_ry * t.cos()));
    }
    painter.add(egui::Shape::line(pts, stroke));

    // Equator
    painter.line_segment([pos2(center.x - r, center.y), pos2(center.x + r, center.y)], stroke);
}

/// Small close "×" for pane chrome.
pub fn draw_close_x(painter: &egui::Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.4_f32, color);
    let s = 3.5_f32;
    painter
        .line_segment([pos2(center.x - s, center.y - s), pos2(center.x + s, center.y + s)], stroke);
    painter
        .line_segment([pos2(center.x + s, center.y - s), pos2(center.x - s, center.y + s)], stroke);
}
