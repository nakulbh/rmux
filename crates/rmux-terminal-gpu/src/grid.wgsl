// G2: instanced terminal cells. Metrics are per-instance for multipane safety.
// Output is premultiplied alpha (matches egui's render pass blending).

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) fg: vec4<f32>, // straight RGBA 0..1
    @location(2) bg: vec4<f32>, // straight RGBA 0..1
    @location(3) @interpolate(flat) flags: u32,
    @location(4) cell_uv: vec2<f32>,
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

    // Points → NDC (viewport = pane; y down in points, y up in NDC).
    let ndc_x = (px / pane_w) * 2.0 - 1.0;
    let ndc_y = 1.0 - (py / pane_h) * 2.0;

    var out: VsOut;
    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.fg = fg;
    out.bg = bg;
    out.flags = flags;
    out.cell_uv = corner;

    // Map UVs only inside the glyph sub-rect; elsewhere uv.x < 0 → bg only.
    let has_glyph = glyph_rect.z > 0.5 && glyph_rect.w > 0.5;
    if (has_glyph) {
        let gx0 = cell_x0 + glyph_rect.x;
        let gy0 = cell_y0 + glyph_rect.y;
        let gx1 = gx0 + glyph_rect.z;
        let gy1 = gy0 + glyph_rect.w;
        let u = (px - gx0) / max(gx1 - gx0, 0.001);
        let v = (py - gy0) / max(gy1 - gy0, 0.001);
        if (u >= 0.0 && u <= 1.0 && v >= 0.0 && v <= 1.0) {
            out.uv = mix(uv_min, uv_max, vec2<f32>(clamp(u, 0.0, 1.0), clamp(v, 0.0, 1.0)));
        } else {
            out.uv = vec2<f32>(-1.0, -1.0);
        }
    } else {
        out.uv = vec2<f32>(-1.0, -1.0);
    }
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Start with straight-alpha background, then composite glyph coverage.
    var rgb = in.bg.rgb;
    var a = in.bg.a;

    if (in.uv.x >= 0.0) {
        let sample = textureSampleLevel(atlas_tex, atlas_samp, in.uv, 0.0);
        // Atlas stores premultiplied white coverage in .a (and .rgb).
        let cov = sample.a;
        rgb = rgb * (1.0 - cov) + in.fg.rgb * cov;
        a = a * (1.0 - cov) + in.fg.a * cov;
    }

    // Underline near bottom of cell.
    if ((in.flags & 1u) != 0u && in.cell_uv.y > 0.88) {
        rgb = in.fg.rgb;
        a = max(a, in.fg.a);
    }

    // Cursor block overlay.
    if ((in.flags & 2u) != 0u) {
        rgb = rgb * 0.35 + in.fg.rgb * 0.65;
        a = max(a, in.fg.a);
    }

    // Premultiply for egui's PREMULTIPLIED_ALPHA_BLENDING target.
    return vec4<f32>(rgb * a, a);
}
