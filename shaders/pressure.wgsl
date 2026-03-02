// Pressure compute shader for soft body simulation.
// Thread per ring particle. Pushes particles outward from centroid
// to maintain enclosed area (2D volume preservation).
// Ring particles are assumed to be contiguous indices 0..particle_count.

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

// Group 0: particle data + sim params
@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(1) var<uniform> params: SimParams;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
    let n = params.particle_count;
    if (idx >= n) {
        return;
    }

    // Compute centroid of the ring
    var cx = 0.0;
    var cy = 0.0;
    for (var i = 0u; i < n; i++) {
        cx += particles[i].position.x;
        cy += particles[i].position.y;
    }
    cx /= f32(n);
    cy /= f32(n);

    // Compute current enclosed area via shoelace formula
    var area_sum = 0.0;
    for (var i = 0u; i < n; i++) {
        let j = (i + 1u) % n;
        let pi = particles[i].position;
        let pj = particles[j].position;
        area_sum += pi.x * pj.y - pj.x * pi.y;
    }
    let current_area = abs(area_sum) * 0.5;

    // How much the ring is compressed: positive when area < rest_area
    let area_deficit = (params.rest_area - current_area) / max(params.rest_area, 0.001);

    // Direction from centroid to this particle (outward)
    let my_pos = particles[idx].position;
    let dx = my_pos.x - cx;
    let dy = my_pos.y - cy;
    let dist_from_center = sqrt(dx * dx + dy * dy);

    if (dist_from_center < 0.0001) {
        return;
    }

    // Outward unit direction
    let outward = vec3f(dx / dist_from_center, dy / dist_from_center, 0.0);

    // Pressure force: push outward proportional to area deficit.
    // The factor of 8.0 is tuned so pressure=1.0 gives a visible, bouncy effect.
    // Using area_deficit (not clamped) allows negative pressure when over-inflated,
    // which provides damping and prevents explosion.
    let force = area_deficit * params.pressure * 8.0;

    // Apply as position correction (PBD-style)
    let displacement = outward * force * params.delta_time;
    particles[idx].position += displacement;

    // Velocity nudge: transfer deformation energy into kinetic energy for bounce.
    // Factor of 0.5 balances bounce height vs stability.
    particles[idx].velocity += outward * force * 0.5;

    // Floor enforcement (final pass before render)
    let floor_limit = params.floor_y + particles[idx].size;
    if (particles[idx].position.y < floor_limit) {
        particles[idx].position.y = floor_limit;
        if (particles[idx].velocity.y < 0.0) {
            particles[idx].velocity.y = -particles[idx].velocity.y * params.restitution;
        }
    }
}
