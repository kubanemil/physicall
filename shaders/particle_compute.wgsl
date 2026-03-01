// Particle compute shader - N-body gravitational simulation with hash-based collision
// Uses double-buffering: reads from particles_in, writes to particles_out
// N-body gravity is O(n²), collision uses spatial hash for O(n) neighbor queries

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
    restitution: f32,             // Collision restitution (bounciness)
    floor_y: f32,                 // Floor boundary y-coordinate
    cell_size: f32,               // Spatial hash cell size
    num_cells: u32,               // Spatial hash table size
    _padding: f32,
}

@group(0) @binding(0) var<storage, read> particles_in: array<Particle>;
@group(0) @binding(1) var<storage, read_write> particles_out: array<Particle>;
@group(0) @binding(2) var<uniform> params: SimParams;

// Hash bind group kept for pipeline compatibility (unused by this shader)
@group(1) @binding(0) var<storage, read_write> cell_count: array<u32>;
@group(1) @binding(1) var<storage, read_write> cell_entries: array<u32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
    if (idx >= params.particle_count) {
        return;
    }

    var particle = particles_in[idx];
    var acceleration = vec3f(0.0, 0.0, 0.0);
    var collision_velocity_delta = vec3f(0.0, 0.0, 0.0);
    var collision_position_delta = vec3f(0.0, 0.0, 0.0);

    // N-body gravitational force + collision detection (O(n²))
    // Collision is checked in the same loop to guarantee symmetric pair detection,
    // which preserves momentum conservation (spatial hash can miss pairs on overflow).
    for (var i = 0u; i < params.particle_count; i++) {
        if (i == idx) {
            continue;
        }

        let other = particles_in[i];
        let diff = other.position - particle.position;
        let actual_dist_sq = dot(diff, diff);
        let dist_sq = actual_dist_sq + params.softening_sq;
        let dist = sqrt(dist_sq);

        // Gravitational acceleration: G / r² (with softening)
        let force_mag = params.gravitational_constant / dist_sq;
        acceleration += (diff / dist) * force_mag;

        // Collision detection (no hash needed — same O(n²) loop)
        let actual_dist = sqrt(actual_dist_sq);
        let combined_radius = particle.size + other.size;

        if (actual_dist < combined_radius && actual_dist > 0.0001) {
            let normal = diff / actual_dist;

            // Position correction: push away by half the overlap
            let overlap = combined_radius - actual_dist;
            collision_position_delta -= normal * (overlap * 0.5);

            // Relative velocity along collision normal
            let rel_vel = particle.velocity - other.velocity;
            let v_n = dot(rel_vel, normal);

            // Only apply impulse if particles are approaching
            if (v_n > 0.0) {
                let impulse = -(1.0 + params.restitution) * v_n * 0.5;
                collision_velocity_delta += normal * impulse;
            }
        }
    }

    // Apply drag force if this particle is being dragged
    if (i32(idx) == params.dragged_particle_idx) {
        let to_target = params.drag_target - particle.position;
        acceleration = to_target * params.drag_strength;
        // Dampen velocity for smoother control
        particle.velocity *= 0.8;
    }

    // Apply collision velocity delta before integration
    particle.velocity += collision_velocity_delta;

    // Semi-implicit Euler integration
    particle.velocity += acceleration * params.delta_time;
    particle.velocity *= params.damping;
    particle.position += particle.velocity * params.delta_time;

    // Apply collision position correction after integration
    particle.position += collision_position_delta;

    // Floor bounce
    if (particle.position.y - particle.size < params.floor_y) {
        particle.position.y = params.floor_y + particle.size;
        if (particle.velocity.y < 0.0) {
            particle.velocity.y = -particle.velocity.y * params.restitution;
        }
    }

    // Keep size unchanged
    particles_out[idx] = particle;
}
