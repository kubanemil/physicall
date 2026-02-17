// Point shader - renders instanced quads as circular points
// Each point is a quad (2 triangles) with fragment discard for circular shape
// Color is derived from velocity magnitude (slow=blue, fast=red)
// Hovered/dragged particles are highlighted

struct CameraUniform {
    view_proj: mat4x4<f32>,
};

struct SimParams {
    delta_time: f32,
    gravitational_constant: f32,
    damping: f32,
    particle_count: u32,
    softening_sq: f32,
    drag_strength: f32,
    dragged_particle_idx: i32,
    hovered_particle_idx: i32,
    drag_target: vec3f,
    _padding: f32,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(0) @binding(1)
var<uniform> sim_params: SimParams;

// Per-vertex data (quad corners)
struct QuadInput {
    @location(0) quad_pos: vec2f,
    @builtin(instance_index) instance_idx: u32,
}

// Per-instance data (particle properties)
struct InstanceInput {
    @location(1) world_pos: vec3f,
    @location(2) size: f32,
    @location(3) velocity: vec3f,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) uv: vec2f,
    @location(1) velocity: vec3f,
    @location(2) @interpolate(flat) instance_idx: u32,
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
    out.velocity = instance.velocity;
    out.instance_idx = quad.instance_idx;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    // Discard fragments outside the circle (radius 0.5 from center)
    let dist = length(in.uv - vec2f(0.5));
    if (dist > 0.5) {
        discard;
    }

    // Velocity-based coloring: slow (blue) to fast (red/orange)
    let speed = length(in.velocity);
    let t = saturate(speed * 5.0);  // Normalize speed, adjust multiplier as needed
    let slow_color = vec3f(0.2, 0.4, 1.0);   // Blue
    let fast_color = vec3f(1.0, 0.3, 0.1);   // Red/orange
    var color = mix(slow_color, fast_color, t);

    // Highlight hovered or dragged particle
    let idx = i32(in.instance_idx);
    if (idx == sim_params.dragged_particle_idx) {
        // Dragged: bright white highlight
        color = mix(color, vec3f(1.0, 1.0, 1.0), 0.6);
    } else if (idx == sim_params.hovered_particle_idx) {
        // Hovered: subtle white highlight
        color = mix(color, vec3f(1.0, 1.0, 1.0), 0.3);
    }

    return vec4f(color, 1.0);
}
