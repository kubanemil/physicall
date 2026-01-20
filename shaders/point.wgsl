// Point shader - renders instanced quads as circular points
// Each point is a quad (2 triangles) with fragment discard for circular shape

struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

// Per-vertex data (quad corners)
struct QuadInput {
    @location(0) quad_pos: vec2f,
}

// Per-instance data (point properties)
struct InstanceInput {
    @location(1) world_pos: vec3f,
    @location(2) size: f32,
    @location(3) color: vec3f,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) uv: vec2f,
    @location(1) color: vec3f,
}

@vertex
fn vs_main(quad: QuadInput, instance: InstanceInput) -> VertexOutput {
    // Axis-aligned quad: expand in world XY plane
    let world_pos = vec3f(
        instance.world_pos.x + quad.quad_pos.x * instance.size,
        instance.world_pos.y + quad.quad_pos.y * instance.size,
        instance.world_pos.z,
    );

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4f(world_pos, 1.0);
    out.uv = quad.quad_pos * 0.5 + 0.5;  // Convert -1..1 to 0..1
    out.color = instance.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    // Discard fragments outside the circle (radius 0.5 from center)
    let dist = length(in.uv - vec2f(0.5));
    if (dist > 0.5) {
        discard;
    }
    return vec4f(in.color, 1.0);
}
