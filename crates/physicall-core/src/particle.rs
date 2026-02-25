/// Per-instance data for particle rendering.
/// Used by both compute shader (storage buffer) and render shader (vertex buffer).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Particle {
    pub(crate) position: [f32; 3],
    pub(crate) size: f32,
    pub(crate) velocity: [f32; 3],
    pub(crate) _padding: f32,  // Align to 32 bytes
}

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
/// Layout must match SimParams in particle_compute.wgsl and point.wgsl.
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

/// Creates initial particles in a ring distribution for N-body simulation.
/// Particles are arranged in a disk in the XZ plane with tangential velocities
/// to create orbital motion.
pub(crate) fn create_initial_particles(count: u32) -> Vec<Particle> {
    let mut particles = Vec::with_capacity(count as usize);

    // Ring parameters
    let inner_radius = 0.8;
    let outer_radius = 2.0;

    // Tangential velocity for roughly circular orbits
    // v = sqrt(G * M_enclosed / r) approximated empirically
    let base_orbital_speed = 0.4;

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
            _padding: 0.0,
        });
    }

    particles
}
