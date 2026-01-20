// Camera uniform - view-projection matrix from Rust
struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

// Vertex input - matches VertexBufferLayout in Rust
struct VertexInput {
    @location(0) position: vec3f,
    @location(1) color: vec3f,
}

// Vertex output - data passed to fragment shader
struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) color: vec3f,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    // Transform position by view-projection matrix
    output.clip_position = camera.view_proj * vec4f(input.position, 1.0);
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    return vec4f(input.color, 1.0);
}
