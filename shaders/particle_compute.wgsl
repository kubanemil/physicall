// Particle compute shader - N-body gravitational simulation
// Uses double-buffering: reads from particles_in, writes to particles_out

struct Particle {
    position: vec3f,
    size: f32,
    velocity: vec3f,
    _padding: f32,
}

struct SimParams {
    delta_time: f32,
    gravitational_constant: f32,  // G - gravitational constant
    damping: f32,
    particle_count: u32,
    softening_sq: f32,            // epsilon² - prevents singularity at r=0
    drag_strength: f32,           // Force multiplier for drag interaction
    dragged_particle_idx: i32,    // -1 if none, else particle index
    hovered_particle_idx: i32,    // -1 if none (for render shader)
    drag_target: vec3f,           // World position to drag toward
    _padding: f32,
}

@group(0) @binding(0) var<storage, read> particles_in: array<Particle>;
@group(0) @binding(1) var<storage, read_write> particles_out: array<Particle>;
@group(0) @binding(2) var<uniform> params: SimParams;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
    if (idx >= params.particle_count) {
        return;
    }

    var particle = particles_in[idx];
    var acceleration = vec3f(0.0, 0.0, 0.0);

    // N-body gravitational force calculation (O(n²))
    // F = G * m1 * m2 / r²
    // Assuming uniform mass = 1, acceleration = G / r² in direction of other particle
    for (var i = 0u; i < params.particle_count; i++) {
        if (i == idx) {
            continue;
        }

        let other = particles_in[i];
        let diff = other.position - particle.position;
        let dist_sq = dot(diff, diff) + params.softening_sq;
        let dist = sqrt(dist_sq);

        // Force magnitude: G / r² (with softening)
        let force_mag = params.gravitational_constant / dist_sq;

        // Acceleration in direction of other particle
        acceleration += (diff / dist) * force_mag;
    }

    // Apply drag force if this particle is being dragged
    if (i32(idx) == params.dragged_particle_idx) {
        let to_target = params.drag_target - particle.position;
        acceleration = to_target * params.drag_strength;
        // Dampen velocity for smoother control
        particle.velocity *= 0.8;
    }

    // Semi-implicit Euler integration
    particle.velocity += acceleration * params.delta_time;
    particle.velocity *= params.damping;
    particle.position += particle.velocity * params.delta_time;

    // Keep size unchanged
    particles_out[idx] = particle;
}
