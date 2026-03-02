/// Per-instance data for particle rendering.
/// Used by both compute shader (storage buffer) and render shader (vertex buffer).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Particle {
    pub(crate) position: [f32; 3],
    pub(crate) size: f32,
    pub(crate) velocity: [f32; 3],
    pub(crate) flags: u32, // Bit 0 = is_pinned. Align to 32 bytes.
}

/// Distance constraint between two particles.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Constraint {
    pub(crate) index_a: u32,
    pub(crate) index_b: u32,
    pub(crate) rest_length: f32,
    pub(crate) _padding: u32,
}

/// Maximum number of constraints in the simulation.
pub(crate) const MAX_CONSTRAINTS: u32 = 8192;
/// Maximum number of constraints per particle (fixed-bucket lookup).
pub(crate) const MAX_CONSTRAINTS_PER_PARTICLE: u32 = 12;
/// Number of constraint solver iterations per frame.
pub(crate) const SOLVER_ITERATIONS: u32 = 4;
/// Default constraint stiffness (0.0 = no correction, 1.0 = full correction).
pub(crate) const DEFAULT_STIFFNESS: f32 = 1.0;
/// Default pressure coefficient for soft bodies.
pub(crate) const DEFAULT_PRESSURE: f32 = 1.0;
/// Default downward gravity for constraint modes.
/// Tuned for simulation scale (~2.4 unit cloth). Real-world 9.8 is too strong.
pub(crate) const DEFAULT_GRAVITY_Y: f32 = -3.0;

impl Particle {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        1 => Float32x3,  // world_pos @ location(1)
        2 => Float32,    // size @ location(2)
        3 => Float32x3,  // velocity @ location(3)
    ];

    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Particle>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Simulation parameters uniform buffer.
/// Layout must match SimParams in particle_compute.wgsl, point.wgsl,
/// hash_insert.wgsl, and constraint_solve.wgsl.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SimParams {
    pub(crate) delta_time: f32,
    pub(crate) gravitational_constant: f32,
    pub(crate) damping: f32,
    pub(crate) particle_count: u32,
    pub(crate) softening_sq: f32,
    pub(crate) drag_strength: f32,
    pub(crate) dragged_particle_idx: i32,
    pub(crate) hovered_particle_idx: i32,
    pub(crate) drag_target: [f32; 3],
    pub(crate) restitution: f32,
    pub(crate) floor_y: f32,
    pub(crate) cell_size: f32,
    pub(crate) num_cells: u32,
    pub(crate) _padding: f32,
    // Constraint fields (Phase 1)
    pub(crate) constraint_count: u32,
    pub(crate) stiffness: f32,
    pub(crate) pressure: f32,
    pub(crate) simulation_mode: u32, // 0=NBody, 1=Cloth, 2=Rope, 3=SoftBody
    pub(crate) gravity_y: f32,
    pub(crate) rest_area: f32,
    pub(crate) _padding2: [f32; 2], // Pad to 96 bytes total, 16-byte aligned
}

/// Physics simulation constants for N-body simulation
pub(crate) const PARTICLE_COUNT: u32 = 500;
pub(crate) const MAX_PARTICLES: u32 = 2_000;
pub(crate) const PARTICLE_SIZE: f32 = 0.03;
pub(crate) const GRAVITATIONAL_CONSTANT: f32 = 0.3;
pub(crate) const SOFTENING_SQ: f32 = 0.1;
pub(crate) const DAMPING: f32 = 1.0;
pub(crate) const DRAG_STRENGTH: f32 = 50.0;
pub(crate) const RESTITUTION: f32 = 0.8;
pub(crate) const FLOOR_Y: f32 = -2.0;
pub(crate) const NUM_CELLS: u32 = 4096;
pub(crate) const MAX_PER_CELL: u32 = 16;
pub(crate) const CELL_SIZE: f32 = 2.0 * PARTICLE_SIZE;

/// Default initial orbital speed for particles.
pub(crate) const INITIAL_SPEED: f32 = 0.4;

/// Creates initial particles in a ring distribution for N-body simulation.
/// Particles are arranged in a disk in the XZ plane with tangential velocities
/// to create orbital motion. `base_orbital_speed` controls the tangential velocity magnitude.
pub(crate) fn create_initial_particles(count: u32, base_orbital_speed: f32) -> Vec<Particle> {
    let mut particles = Vec::with_capacity(count as usize);

    // Ring parameters
    let inner_radius = 0.8;
    let outer_radius = 2.0;

    for i in 0..count {
        let t = i as f32 / count as f32;
        let angle = t * std::f32::consts::PI * 2.0 * 3.0; // 3 rotations for spiral

        // Distribute radius from inner to outer
        let radius = inner_radius + (outer_radius - inner_radius) * t;

        // Position in XZ plane (y = 0) with small vertical variation
        let y_variation = 0.1 * ((i as f32 * 0.5).sin());
        let x = radius * angle.cos();
        let z = radius * angle.sin();
        let y = y_variation;

        // Tangential velocity (perpendicular to radius in XZ plane)
        // Scale velocity with sqrt(1/radius) for Keplerian-ish motion
        let speed = base_orbital_speed * (1.0 / radius).sqrt();
        let vx = -speed * angle.sin(); // Perpendicular to radius
        let vz = speed * angle.cos();
        let vy = 0.0;

        particles.push(Particle {
            position: [x, y, z],
            size: PARTICLE_SIZE,
            velocity: [vx, vy, vz],
            flags: 0,
        });
    }

    // Center the distribution: subtract center-of-mass position and velocity.
    // The spiral distribution is not perfectly symmetric, leaving a small CoM
    // offset (~0.064 units) and net momentum that would cause visible drift.
    let n = count as f32;
    let (mut cx, mut cy, mut cz) = (0.0f32, 0.0f32, 0.0f32);
    let (mut cvx, mut cvy, mut cvz) = (0.0f32, 0.0f32, 0.0f32);
    for p in &particles {
        cx += p.position[0];
        cy += p.position[1];
        cz += p.position[2];
        cvx += p.velocity[0];
        cvy += p.velocity[1];
        cvz += p.velocity[2];
    }
    cx /= n; cy /= n; cz /= n;
    cvx /= n; cvy /= n; cvz /= n;
    for p in &mut particles {
        p.position[0] -= cx;
        p.position[1] -= cy;
        p.position[2] -= cz;
        p.velocity[0] -= cvx;
        p.velocity[1] -= cvy;
        p.velocity[2] -= cvz;
    }

    particles
}

/// Result type for preset initialization functions.
/// Contains particles, constraints, and per-particle constraint index lists.
pub(crate) struct PresetData {
    pub(crate) particles: Vec<Particle>,
    pub(crate) constraints: Vec<Constraint>,
    /// Per-particle constraint indices, flattened. Length = particle_count * MAX_CONSTRAINTS_PER_PARTICLE.
    pub(crate) constraint_indices: Vec<u32>,
    /// Per-particle constraint count. Length = particle_count.
    pub(crate) constraint_counts: Vec<u32>,
    /// Rest area for pressure-based soft bodies (0.0 for non-pressure presets).
    pub(crate) rest_area: f32,
}

/// Creates a cloth grid in the XY plane for cloth simulation.
/// Top-left and top-right corners are pinned.
pub(crate) fn create_cloth_particles(cols: u32, rows: u32, spacing: f32) -> PresetData {
    let count = cols * rows;
    let mut particles = Vec::with_capacity(count as usize);
    let mut constraints = Vec::new();
    // Per-particle constraint index lists
    let mut per_particle: Vec<Vec<u32>> = vec![Vec::new(); count as usize];

    let half_w = (cols - 1) as f32 * spacing * 0.5;
    let top_y = (rows - 1) as f32 * spacing * 0.5;

    // Create particles in a grid (row-major: row 0 is top)
    for row in 0..rows {
        for col in 0..cols {
            let x = col as f32 * spacing - half_w;
            let y = top_y - row as f32 * spacing;
            let z = 0.0;

            // Pin top-left and top-right corners
            let is_pinned = row == 0 && (col == 0 || col == cols - 1);

            particles.push(Particle {
                position: [x, y, z],
                size: PARTICLE_SIZE,
                velocity: [0.0; 3],
                flags: if is_pinned { 1 } else { 0 },
            });
        }
    }

    // Helper to add a constraint between two particle indices
    let mut add_constraint = |a: u32, b: u32, rest_len: f32| {
        let idx = constraints.len() as u32;
        constraints.push(Constraint {
            index_a: a,
            index_b: b,
            rest_length: rest_len,
            _padding: 0,
        });
        per_particle[a as usize].push(idx);
        per_particle[b as usize].push(idx);
    };

    // Structural constraints: horizontal + vertical neighbors
    for row in 0..rows {
        for col in 0..cols {
            let i = row * cols + col;
            // Right neighbor
            if col + 1 < cols {
                add_constraint(i, i + 1, spacing);
            }
            // Bottom neighbor
            if row + 1 < rows {
                add_constraint(i, i + cols, spacing);
            }
        }
    }

    // Shear constraints: diagonal neighbors
    let diag = spacing * std::f32::consts::SQRT_2;
    for row in 0..rows {
        for col in 0..cols {
            let i = row * cols + col;
            // Bottom-right diagonal
            if col + 1 < cols && row + 1 < rows {
                add_constraint(i, i + cols + 1, diag);
            }
            // Bottom-left diagonal
            if col > 0 && row + 1 < rows {
                add_constraint(i, i + cols - 1, diag);
            }
        }
    }

    // Flatten per-particle indices into fixed-bucket arrays
    let (constraint_indices, constraint_counts) = flatten_constraint_indices(&per_particle);

    PresetData {
        particles,
        constraints,
        constraint_indices,
        constraint_counts,
        rest_area: 0.0,
    }
}

/// Creates a rope (chain) of particles along the X axis.
/// First particle is pinned.
pub(crate) fn create_rope_particles(count: u32, spacing: f32) -> PresetData {
    let mut particles = Vec::with_capacity(count as usize);
    let mut constraints = Vec::new();
    let mut per_particle: Vec<Vec<u32>> = vec![Vec::new(); count as usize];

    let half_len = (count - 1) as f32 * spacing * 0.5;

    for i in 0..count {
        let x = i as f32 * spacing - half_len;
        let is_pinned = i == 0;

        particles.push(Particle {
            position: [x, 1.0, 0.0], // Start elevated so it can swing
            size: PARTICLE_SIZE,
            velocity: [0.0; 3],
            flags: if is_pinned { 1 } else { 0 },
        });
    }

    // Chain constraints between consecutive particles
    for i in 0..(count - 1) {
        let idx = constraints.len() as u32;
        constraints.push(Constraint {
            index_a: i,
            index_b: i + 1,
            rest_length: spacing,
            _padding: 0,
        });
        per_particle[i as usize].push(idx);
        per_particle[(i + 1) as usize].push(idx);
    }

    let (constraint_indices, constraint_counts) = flatten_constraint_indices(&per_particle);

    PresetData {
        particles,
        constraints,
        constraint_indices,
        constraint_counts,
        rest_area: 0.0,
    }
}

/// Creates a soft body ring of particles in the XY plane.
/// Particles are arranged in a circle with perimeter constraints and cross-bracing.
/// Returns PresetData with computed rest_area via shoelace formula.
pub(crate) fn create_soft_body_particles(num_ring: u32, radius: f32) -> PresetData {
    let mut particles = Vec::with_capacity(num_ring as usize);
    let mut constraints = Vec::new();
    let mut per_particle: Vec<Vec<u32>> = vec![Vec::new(); num_ring as usize];

    // Create ring of particles in XY plane, elevated so it can fall
    let start_y = 1.5;
    for i in 0..num_ring {
        let angle = (i as f32 / num_ring as f32) * std::f32::consts::PI * 2.0;
        let x = radius * angle.cos();
        let y = start_y + radius * angle.sin();

        particles.push(Particle {
            position: [x, y, 0.0],
            size: PARTICLE_SIZE,
            velocity: [0.0; 3],
            flags: 0,
        });
    }

    // Perimeter constraints only: each particle connected to next (wrapping).
    // No cross-bracing! The ring shape is maintained by internal pressure,
    // not by rigid diameter constraints. This allows the ring to deform freely
    // on impact, which triggers a strong pressure response (the bounce).
    let perimeter_spacing = 2.0 * std::f32::consts::PI * radius / num_ring as f32;
    let mut add_constraint = |a: u32, b: u32, rest_len: f32| {
        let idx = constraints.len() as u32;
        constraints.push(Constraint {
            index_a: a,
            index_b: b,
            rest_length: rest_len,
            _padding: 0,
        });
        per_particle[a as usize].push(idx);
        per_particle[b as usize].push(idx);
    };

    for i in 0..num_ring {
        let next = (i + 1) % num_ring;
        add_constraint(i, next, perimeter_spacing);
    }

    // Compute rest area via shoelace formula
    let mut area = 0.0f32;
    for i in 0..num_ring {
        let j = (i + 1) % num_ring;
        let pi = &particles[i as usize];
        let pj = &particles[j as usize];
        area += pi.position[0] * pj.position[1] - pj.position[0] * pi.position[1];
    }
    let rest_area = area.abs() * 0.5;

    let (constraint_indices, constraint_counts) = flatten_constraint_indices(&per_particle);

    PresetData {
        particles,
        constraints,
        constraint_indices,
        constraint_counts,
        rest_area,
    }
}

/// Flattens per-particle constraint index lists into fixed-bucket arrays
/// for GPU upload.
fn flatten_constraint_indices(per_particle: &[Vec<u32>]) -> (Vec<u32>, Vec<u32>) {
    let count = per_particle.len();
    let mut indices = vec![0u32; count * MAX_CONSTRAINTS_PER_PARTICLE as usize];
    let mut counts = vec![0u32; count];

    for (i, particle_constraints) in per_particle.iter().enumerate() {
        let n = particle_constraints.len().min(MAX_CONSTRAINTS_PER_PARTICLE as usize);
        counts[i] = n as u32;
        for (j, &ci) in particle_constraints.iter().take(n).enumerate() {
            indices[i * MAX_CONSTRAINTS_PER_PARTICLE as usize + j] = ci;
        }
    }

    (indices, counts)
}
