// Particle state shared by every XPBD kernel. Mirrors physicall-core's xpbd
// and contact modules; pos.w holds the inverse mass, 0 for static particles.

@group(0) @binding(0) var<uniform> params: XpbdUniforms;
@group(0) @binding(1) var<storage, read_write> pos: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> prev: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> vel: array<vec4<f32>>;

// Position-based Coulomb friction (Macklin et al. 2014, sec. 6.1).
fn friction(tangential: vec3<f32>, penetration: f32, mu: f32) -> vec3<f32> {
    let limit = mu * penetration;
    let len = length(tangential);
    if len > limit {
        return tangential * (limit / len);
    }
    return tangential;
}
