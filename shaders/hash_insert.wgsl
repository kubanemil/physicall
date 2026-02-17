// Hash insert shader - assigns particles to spatial hash cells
// Dispatched as ceil(particle_count / 256) workgroups

struct Particle {
    position: vec3f,
    size: f32,
    velocity: vec3f,
    _padding: f32,
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
}

const MAX_PER_CELL: u32 = 16u;

@group(0) @binding(0) var<storage, read> particles_in: array<Particle>;
@group(0) @binding(2) var<uniform> params: SimParams;

@group(1) @binding(0) var<storage, read_write> cell_count: array<atomic<u32>>;
@group(1) @binding(1) var<storage, read_write> cell_entries: array<u32>;

/// Hash 3D cell coordinates to a flat index.
fn hash_cell(cx: i32, cy: i32, cz: i32, num_cells: u32) -> u32 {
    // Large primes for spatial hashing
    let h = cx * 73856093 + cy * 19349663 + cz * 83492791;
    return u32(abs(h)) % num_cells;
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
    if (idx >= params.particle_count) {
        return;
    }

    let pos = particles_in[idx].position;

    // Compute 3D cell coordinates
    let cx = i32(floor(pos.x / params.cell_size));
    let cy = i32(floor(pos.y / params.cell_size));
    let cz = i32(floor(pos.z / params.cell_size));

    let cell_hash = hash_cell(cx, cy, cz, params.num_cells);

    // Atomically insert into cell bucket
    let offset = atomicAdd(&cell_count[cell_hash], 1u);
    if (offset < MAX_PER_CELL) {
        cell_entries[cell_hash * MAX_PER_CELL + offset] = idx;
    }
}
