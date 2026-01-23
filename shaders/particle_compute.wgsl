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
    gravity_strength: f32,
    damping: f32,
    particle_count: u32,
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
    var acceleration = vec3f(0.0);

    // N-body: sum gravitational forces from all other particles
    for (var i = 0u; i < params.particle_count; i++) {
        if (i == idx) {
            continue;
        }

        let other = particles_in[i];
        let diff = other.position - particle.position;
        let dist_sq = dot(diff, diff) + 0.001; // Softening to avoid singularity
        let dist = sqrt(dist_sq);
        let force_mag = params.gravity_strength / dist_sq;
        acceleration += (diff / dist) * force_mag;
    }

    // Semi-implicit Euler integration
    particle.velocity += acceleration * params.delta_time;
    particle.velocity *= params.damping;
    particle.position += particle.velocity * params.delta_time;

    // Keep size unchanged
    particles_out[idx] = particle;
}
