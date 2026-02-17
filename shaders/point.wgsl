// Point shader - renders instanced quads as sphere impostors
// Each point is a billboard quad with sphere lighting in the fragment shader
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
    restitution: f32,
    floor_y: f32,
    cell_size: f32,
    num_cells: u32,
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
    // Billboard quad: expand in view-aligned XY using view_proj columns
    // Extract right and up vectors from view_proj for billboarding
    let right = normalize(vec3f(camera.view_proj[0][0], camera.view_proj[1][0], camera.view_proj[2][0]));
    let up = normalize(vec3f(camera.view_proj[0][1], camera.view_proj[1][1], camera.view_proj[2][1]));

    let world_pos = instance.world_pos
        + right * quad.quad_pos.x * instance.size
        + up * quad.quad_pos.y * instance.size;

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4f(world_pos, 1.0);
    out.uv = quad.quad_pos;  // Keep in -1..1 range for sphere normal calculation
    out.velocity = instance.velocity;
    out.instance_idx = quad.instance_idx;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    // Discard fragments outside the unit circle
    let len_sq = dot(in.uv, in.uv);
    if (len_sq > 1.0) {
        discard;
    }

    // Compute sphere normal from UV (treating quad as hemisphere projection)
    let nz = sqrt(1.0 - len_sq);
    let normal = vec3f(in.uv.x, in.uv.y, nz);

    // Simple directional light from upper-right-front
    let light_dir = normalize(vec3f(0.4, 0.7, 0.6));
    let diffuse = max(dot(normal, light_dir), 0.0);
    let ambient = 0.15;

    // Specular highlight (Blinn-Phong)
    let view_dir = vec3f(0.0, 0.0, 1.0); // billboard faces camera
    let half_dir = normalize(light_dir + view_dir);
    let specular = pow(max(dot(normal, half_dir), 0.0), 32.0) * 0.4;

    // Velocity-based coloring: slow (blue) to fast (red/orange)
    let speed = length(in.velocity);
    let t = saturate(speed * 5.0);
    let slow_color = vec3f(0.2, 0.4, 1.0);   // Blue
    let fast_color = vec3f(1.0, 0.3, 0.1);   // Red/orange
    var base_color = mix(slow_color, fast_color, t);

    // Highlight hovered or dragged particle
    let idx = i32(in.instance_idx);
    if (idx == sim_params.dragged_particle_idx) {
        base_color = mix(base_color, vec3f(1.0, 1.0, 1.0), 0.6);
    } else if (idx == sim_params.hovered_particle_idx) {
        base_color = mix(base_color, vec3f(1.0, 1.0, 1.0), 0.3);
    }

    // Combine lighting
    let color = base_color * (ambient + diffuse) + vec3f(specular);

    return vec4f(color, 1.0);
}
