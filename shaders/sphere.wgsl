// Sphere shader - renders instanced icosphere meshes with basic lighting

struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

// Per-vertex data (icosphere geometry)
struct VertexInput {
    @location(0) position: vec3f,
    @location(1) normal: vec3f,
}

// Per-instance data (sphere properties)
struct InstanceInput {
    @location(2) world_pos: vec3f,
    @location(3) radius: f32,
    @location(4) color: vec3f,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) world_normal: vec3f,
    @location(1) color: vec3f,
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    // Scale vertex by radius and translate by instance position
    let world_pos = instance.world_pos + vertex.position * instance.radius;

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4f(world_pos, 1.0);
    out.world_normal = vertex.normal;  // Normal unchanged for uniform scale
    out.color = instance.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    // Simple directional light from upper-right-front
    let light_dir = normalize(vec3f(0.3, 0.5, 1.0));
    let normal = normalize(in.world_normal);

    // Diffuse lighting with ambient minimum
    let diffuse = max(dot(normal, light_dir), 0.2);

    return vec4f(in.color * diffuse, 1.0);
}
