// G2: instanced terminal cells.
// Metrics are per-instance for multipane safety.
// Output is premultiplied alpha (matches egui's render pass).
//
// Glyph UVs are computed as a *continuous* mapping from cell pixel → glyph
// local space at every vertex (values may be outside 0..1). Fragment shader
// only samples when the interpolated UV is inside 0..1. Setting UV to a
// sentinel at cell corners (which lie outside the glyph sub-rect) is wrong:
// all four corners fail the test and interpolation never hits the glyph.

struct VsOut {
    @builtin(position) position: vec4<f32>,
    /// Glyph-local UV in "bitmap space" (0..1 = inside ink rect; may be outside).
    @location(0) glyph_local: vec2<f32>,
    @location(1) fg: vec4<f32>,
    @location(2) bg: vec4<f32>,
    @location(3) @interpolate(flat) flags: u32,
    @location(4) cell_uv: vec2<f32>,
    @location(5) @interpolate(flat) atlas_uv_min: vec2<f32>,
    @location(6) @interpolate(flat) atlas_uv_max: vec2<f32>,
    @location(7) @interpolate(flat) has_glyph: u32,
};

@group(0) @binding(0) var atlas_tex: texture_2d<f32>;
@group(0) @binding(1) var atlas_samp: sampler;

@vertex
fn vs_main(
    @builtin(vertex_index) vid: u32,
    @location(0) geo: vec4<f32>,       // col, row, span, flags
    @location(1) metrics: vec4<f32>,   // pane_w, pane_h, cell_w, cell_h
    @location(2) fg: vec4<f32>,
    @location(3) bg: vec4<f32>,
    @location(4) glyph_rect: vec4<f32>, // ox, oy, gw, gh in points
    @location(5) uv_min: vec2<f32>,
    @location(6) uv_max: vec2<f32>,
) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );
    let corner = corners[vid];

    let col = geo.x;
    let row = geo.y;
    let span = max(geo.z, 1.0);
    let flags = u32(geo.w + 0.5);

    let pane_w = max(metrics.x, 1.0);
    let pane_h = max(metrics.y, 1.0);
    let cell_w = max(metrics.z, 1.0);
    let cell_h = max(metrics.w, 1.0);

    let cell_x0 = col * cell_w;
    let cell_y0 = row * cell_h;
    let cell_x1 = cell_x0 + span * cell_w;
    let cell_y1 = cell_y0 + cell_h;

    let px = mix(cell_x0, cell_x1, corner.x);
    let py = mix(cell_y0, cell_y1, corner.y);

    let ndc_x = (px / pane_w) * 2.0 - 1.0;
    let ndc_y = 1.0 - (py / pane_h) * 2.0;

    var out: VsOut;
    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.fg = fg;
    out.bg = bg;
    out.flags = flags;
    out.cell_uv = corner;
    out.atlas_uv_min = uv_min;
    out.atlas_uv_max = uv_max;

    let has = glyph_rect.z > 0.5 && glyph_rect.w > 0.5;
    out.has_glyph = select(0u, 1u, has);

    if (has) {
        let gx0 = cell_x0 + glyph_rect.x;
        let gy0 = cell_y0 + glyph_rect.y;
        // Continuous mapping — may be <0 or >1 at cell corners; that is required
        // so the fragment stage can interpolate into the ink region.
        out.glyph_local = vec2<f32>(
            (px - gx0) / glyph_rect.z,
            (py - gy0) / glyph_rect.w,
        );
    } else {
        out.glyph_local = vec2<f32>(-1.0, -1.0);
    }
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var rgb = in.bg.rgb;
    var a = in.bg.a;

    if (in.has_glyph != 0u
        && in.glyph_local.x >= 0.0 && in.glyph_local.x <= 1.0
        && in.glyph_local.y >= 0.0 && in.glyph_local.y <= 1.0)
    {
        let atlas_uv = mix(in.atlas_uv_min, in.atlas_uv_max, in.glyph_local);
        let sample = textureSampleLevel(atlas_tex, atlas_samp, atlas_uv, 0.0);
        let cov = sample.a;
        rgb = rgb * (1.0 - cov) + in.fg.rgb * cov;
        a = a * (1.0 - cov) + in.fg.a * cov;
    }

    if ((in.flags & 1u) != 0u && in.cell_uv.y > 0.88) {
        rgb = in.fg.rgb;
        a = max(a, in.fg.a);
    }

    if ((in.flags & 2u) != 0u) {
        rgb = rgb * 0.35 + in.fg.rgb * 0.65;
        a = max(a, in.fg.a);
    }

    return vec4<f32>(rgb * a, a);
}
