// Instanced sprites with a uniform camera.
enable f16;

struct Camera {
    view_proj: mat4x4<f32>,
    time: f32,
};

struct VertexOut {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) instance: u32,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var sprite_tex: texture_2d<f32>;
@group(0) @binding(2) var sprite_sampler: sampler;
@group(1) @binding(0) var<storage, read> offsets: array<vec2<f32>>;

const QUAD = array<vec2f, 4>(
    vec2f(-0.5, -0.5), vec2f(0.5, -0.5), vec2f(-0.5, 0.5), vec2f(0.5, 0.5),
);

/* Vertex stage: places each quad at its instance offset. */
@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VertexOut {
    var out: VertexOut;
    let corner = QUAD[vi];
    let wobble = sin(camera.time * 2.0 + f32(ii)) * 0.1;
    out.position = camera.view_proj * vec4f(corner + offsets[ii] + wobble, 0.0, 1.0);
    out.uv = corner + vec2f(0.5);
    out.instance = ii;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let color = textureSample(sprite_tex, sprite_sampler, in.uv);
    if (color.a < 0.01) {
        discard;
    }
    return color * vec4f(1.0, 1.0, 1.0, 0.9);
}
