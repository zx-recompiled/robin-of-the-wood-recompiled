// The overlay: one texture, drawn over the game picture in the rectangle the
// picture occupies, with its colours already multiplied by their alpha.

struct VertexOutput {
    @location(0) tex_coord: vec2<f32>,
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs_main(@location(0) position: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coord = fma(position, vec2<f32>(0.5, -0.5), vec2<f32>(0.5, 0.5));
    out.position = vec4<f32>(position, 0.0, 1.0);
    return out;
}

@group(0) @binding(0) var r_tex: texture_2d<f32>;
@group(0) @binding(1) var r_sampler: sampler;

@fragment
fn fs_main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(r_tex, r_sampler, tex_coord);
}
