// Constraint solver compute shader - Position Based Dynamics (PBD)
// Thread per particle. Each thread reads its own constraints and applies corrections.
// Runs multiple iterations per frame (dispatched multiple times from CPU).

struct Particle {
    position: vec3f,
    size: f32,
    velocity: vec3f,
    flags: u32,
}

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
    // Constraint fields
    constraint_count: u32,
    stiffness: f32,
    pressure: f32,
    simulation_mode: u32,
    gravity_y: f32,
    rest_area: f32,
    _padding2a: f32,
    _padding2b: f32,
}

struct Constraint {
    index_a: u32,
    index_b: u32,
    rest_length: f32,
    _padding: u32,
}

// Group 0: particle data + sim params (read_write for in-place solving)
@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(1) var<uniform> params: SimParams;

// Group 1: constraint data (read-only)
@group(1) @binding(0) var<storage, read> constraints: array<Constraint>;
@group(1) @binding(1) var<storage, read> particle_constraint_indices: array<u32>;
@group(1) @binding(2) var<storage, read> particle_constraint_count: array<u32>;

const MAX_CONSTRAINTS_PER_PARTICLE: u32 = 12u;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
    if (idx >= params.particle_count) {
        return;
    }

    // Skip pinned particles (bit 0 of flags)
    if ((particles[idx].flags & 1u) != 0u) {
        return;
    }

    let num_constraints = particle_constraint_count[idx];
    if (num_constraints == 0u) {
        return;
    }

    var correction = vec3f(0.0, 0.0, 0.0);
    var correction_count = 0u;

    let my_pos = particles[idx].position;

    for (var c = 0u; c < num_constraints; c++) {
        let constraint_idx = particle_constraint_indices[idx * MAX_CONSTRAINTS_PER_PARTICLE + c];
        if (constraint_idx >= params.constraint_count) {
            continue;
        }

        let constraint = constraints[constraint_idx];

        // Determine the other particle
        var other_idx = constraint.index_b;
        if (other_idx == idx) {
            other_idx = constraint.index_a;
        }

        let other_pos = particles[other_idx].position;
        let diff = other_pos - my_pos;
        let current_dist = length(diff);

        if (current_dist < 0.0001) {
            continue;
        }

        // PBD distance constraint correction
        let delta = (current_dist - constraint.rest_length) / current_dist * params.stiffness * 0.5;

        // Check if other is pinned - if so, this particle takes full correction
        let other_pinned = (particles[other_idx].flags & 1u) != 0u;
        let weight = select(0.5, 1.0, other_pinned);

        correction += diff * delta * f32(weight);
        correction_count += 1u;
    }

    if (correction_count > 0u) {
        particles[idx].position += correction;
        // Update velocity to reflect constraint correction.
        // Without this, velocity diverges from actual position changes
        // (gravity keeps accelerating while constraints snap positions back).
        // Scale by 0.5 to avoid over-damping — partial velocity update.
        if (params.delta_time > 0.0001) {
            particles[idx].velocity += correction / params.delta_time * 0.3;
        }
    }
}
