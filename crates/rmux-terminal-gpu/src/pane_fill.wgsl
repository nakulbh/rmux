// G0: fill the paint-callback viewport with a solid premultiplied color.
// egui sets the render-pass viewport to the pane rect, so NDC (-1..1) maps to the pane.

struct Uniforms {
    color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
};

// Full-screen triangle (covers NDC without a vertex buffer).
@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOut {
    var out: VertexOut;
    // (0,0), (2,0), (0,2) in 0..1 then map to -1..1 — classic oversized triangle.
    let x = f32(i32(idx & 1u) * 4 - 1);
    let y = f32(i32(idx >> 1u) * 4 - 1);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(_in: VertexOut) -> @location(0) vec4<f32> {
    return uniforms.color;
}
