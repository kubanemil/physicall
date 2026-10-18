//! The XPBD solver of `physicall_core::xpbd` as wgpu compute passes.

use glam::Vec3;
use physicall_core::constraint::ColouredConstraints;
use physicall_core::hash::SpatialHash;
use physicall_core::particles::Particles;
use physicall_core::xpbd::{Substep, XpbdParams};

use crate::layout::{
    ColourRange, FLAG_FLOOR, FLAG_WALLS, GpuConstraint, SCAN_THREADS, WORKGROUP_SIZE, XpbdUniforms,
};
use crate::pack::{pack, pack_zero_w};
use crate::shader;
use crate::util::{self, PARTICLE_USAGE, UNIFORM_USAGE};

const COMPUTE: wgpu::ShaderStages = wgpu::ShaderStages::COMPUTE;

pub fn uniforms(params: &XpbdParams, count: u32, table_size: u32) -> XpbdUniforms {
    let k = Substep::new(params);
    let mut flags = 0;
    if params.bounds.floor.is_some() {
        flags |= FLAG_FLOOR;
    }
    if params.bounds.half_width.is_some() {
        flags |= FLAG_WALLS;
    }
    XpbdUniforms {
        gravity: params.gravity.extend(0.0).to_array(),
        alpha: k.alpha,
        h: k.h,
        inv_h: k.inv_h,
        damping: k.damping,
        radius: params.radius,
        friction: params.friction,
        relaxation: params.contact_relaxation,
        floor: params.bounds.floor.unwrap_or(0.0),
        half_width: params.bounds.half_width.unwrap_or(0.0),
        count,
        table_size,
        flags,
        inv_cell_size: k.inv_cell_size,
    }
}

/// Pipelines and layouts, created once per device and shared by every scene.
#[derive(Clone, Debug)]
pub struct XpbdKernels {
    particles: wgpu::BindGroupLayout,
    constraints: wgpu::BindGroupLayout,
    grid: wgpu::BindGroupLayout,
    predict: wgpu::ComputePipeline,
    finish: wgpu::ComputePipeline,
    solve: wgpu::ComputePipeline,
    hash_clear: wgpu::ComputePipeline,
    hash_count: wgpu::ComputePipeline,
    hash_scan_blocks: wgpu::ComputePipeline,
    hash_scan_sums: wgpu::ComputePipeline,
    hash_scan_add: wgpu::ComputePipeline,
    hash_scatter: wgpu::ComputePipeline,
    hash_sort: wgpu::ComputePipeline,
    contacts: wgpu::ComputePipeline,
    apply_delta: wgpu::ComputePipeline,
}

impl XpbdKernels {
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = |label, entries: &[wgpu::BindGroupLayoutEntry]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries,
            })
        };
        let particles = layout(
            "xpbd particles",
            &[
                util::uniform_entry(0, false, COMPUTE),
                util::storage_entry(1, false, COMPUTE),
                util::storage_entry(2, false, COMPUTE),
                util::storage_entry(3, false, COMPUTE),
            ],
        );
        let constraints = layout(
            "xpbd constraints",
            &[
                util::uniform_entry(0, true, COMPUTE),
                util::storage_entry(1, true, COMPUTE),
            ],
        );
        let grid = layout(
            "xpbd grid",
            &(0..4)
                .map(|i| util::storage_entry(i, false, COMPUTE))
                .collect::<Vec<_>>(),
        );

        let integrate_layout = util::pipeline_layout(device, "integrate", &[&particles]);
        let solve_layout = util::pipeline_layout(device, "solve", &[&particles, &constraints]);
        let grid_layout = util::pipeline_layout(device, "grid", &[&particles, &grid]);
        let integrate = shader::module(device, "integrate", shader::INTEGRATE);
        let solve = shader::module(device, "solve", shader::SOLVE);
        let grid_module = shader::module(device, "grid", shader::GRID);
        let on_grid = |entry| util::compute_pipeline(device, &grid_layout, &grid_module, entry);

        Self {
            predict: util::compute_pipeline(device, &integrate_layout, &integrate, "predict"),
            finish: util::compute_pipeline(device, &integrate_layout, &integrate, "finish"),
            solve: util::compute_pipeline(device, &solve_layout, &solve, "solve_distance"),
            hash_clear: on_grid("hash_clear"),
            hash_count: on_grid("hash_count"),
            hash_scan_blocks: on_grid("hash_scan_blocks"),
            hash_scan_sums: on_grid("hash_scan_sums"),
            hash_scan_add: on_grid("hash_scan_add"),
            hash_scatter: on_grid("hash_scatter"),
            hash_sort: on_grid("hash_sort"),
            contacts: on_grid("contacts"),
            apply_delta: on_grid("apply_delta"),
            particles,
            constraints,
            grid,
        }
    }
}

/// One scene's GPU state. `encode` records whole fixed steps.
pub struct XpbdGpu {
    kernels: XpbdKernels,
    params: XpbdParams,
    count: u32,
    table_size: u32,
    deterministic: bool,
    pos: wgpu::Buffer,
    vel: wgpu::Buffer,
    uniforms: wgpu::Buffer,
    cell_start: wgpu::Buffer,
    sorted: wgpu::Buffer,
    colour_stride: u32,
    colour_sizes: Vec<u32>,
    particles_group: wgpu::BindGroup,
    constraints_group: wgpu::BindGroup,
    grid_group: wgpu::BindGroup,
}

impl XpbdGpu {
    pub fn new(
        device: &wgpu::Device,
        kernels: &XpbdKernels,
        particles: &Particles,
        constraints: &ColouredConstraints,
        params: &XpbdParams,
    ) -> Self {
        let count = particles.len() as u32;
        let table_size = SpatialHash::table_size_for(particles.len());
        let bytes = |data: &[[f32; 4]]| bytemuck::cast_slice::<_, u8>(data).to_vec();

        let pos = util::buffer(
            device,
            "pos",
            &bytes(&pack(&particles.pos, &particles.inv_mass)),
            PARTICLE_USAGE,
        );
        let prev = util::buffer(
            device,
            "prev",
            &bytes(&pack(&particles.prev, &particles.inv_mass)),
            PARTICLE_USAGE,
        );
        let vel = util::buffer(
            device,
            "vel",
            &bytes(&pack_zero_w(&particles.vel)),
            PARTICLE_USAGE,
        );
        let uniforms = util::buffer(
            device,
            "xpbd uniforms",
            bytemuck::bytes_of(&uniforms(params, count, table_size)),
            UNIFORM_USAGE,
        );

        let gpu_constraints: Vec<GpuConstraint> = constraints
            .as_slice()
            .iter()
            .map(|c| GpuConstraint {
                a: c.a,
                b: c.b,
                rest: c.rest,
                material: c.material as u32,
            })
            .collect();
        let constraint_buffer = util::buffer(
            device,
            "constraints",
            bytemuck::cast_slice(&gpu_constraints),
            wgpu::BufferUsages::STORAGE,
        );

        let colour_stride = device.limits().min_uniform_buffer_offset_alignment;
        let ranges: Vec<_> = constraints.colour_ranges().collect();
        let mut colour_bytes = vec![0u8; ranges.len().max(1) * colour_stride as usize];
        for (i, r) in ranges.iter().enumerate() {
            let range = ColourRange {
                first: r.start,
                count: r.end - r.start,
                ..Default::default()
            };
            let at = i * colour_stride as usize;
            colour_bytes[at..at + size_of::<ColourRange>()]
                .copy_from_slice(bytemuck::bytes_of(&range));
        }
        let colours = util::buffer(
            device,
            "colours",
            &colour_bytes,
            wgpu::BufferUsages::UNIFORM,
        );

        let grid_buffer = |label, size: u64| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size.max(16),
                usage: PARTICLE_USAGE,
                mapped_at_creation: false,
            })
        };
        let n = u64::from(count);
        // Bucket offsets, the end marker, then one partial sum per scan block.
        let scan_blocks = table_size.div_ceil(SCAN_THREADS);
        let cell_start = grid_buffer("cell start", 4 * u64::from(table_size + 1 + scan_blocks));
        let entries = grid_buffer("cell entries", 8 * n);
        let sorted = grid_buffer("sorted", 4 * n);
        let delta = grid_buffer("delta", 16 * n);

        let particles_group = util::bind_group(
            device,
            "xpbd particles",
            &kernels.particles,
            &[
                uniforms.as_entire_binding(),
                pos.as_entire_binding(),
                prev.as_entire_binding(),
                vel.as_entire_binding(),
            ],
        );
        let constraints_group = util::bind_group(
            device,
            "xpbd constraints",
            &kernels.constraints,
            &[
                wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &colours,
                    offset: 0,
                    size: wgpu::BufferSize::new(size_of::<ColourRange>() as u64),
                }),
                constraint_buffer.as_entire_binding(),
            ],
        );
        let grid_group = util::bind_group(
            device,
            "xpbd grid",
            &kernels.grid,
            &[
                cell_start.as_entire_binding(),
                entries.as_entire_binding(),
                sorted.as_entire_binding(),
                delta.as_entire_binding(),
            ],
        );

        Self {
            kernels: kernels.clone(),
            params: *params,
            count,
            table_size,
            deterministic: false,
            pos,
            vel,
            uniforms,
            cell_start,
            sorted,
            colour_stride,
            colour_sizes: ranges.iter().map(|r| r.end - r.start).collect(),
            particles_group,
            constraints_group,
            grid_group,
        }
    }

    /// Sorts every hash bucket by particle index each substep, so contact
    /// sums add up in a fixed order: runs repeat exactly and follow the CPU's
    /// summation order. Costs a dispatch and O(k²) per bucket of k particles.
    pub fn deterministic(mut self) -> Self {
        self.deterministic = true;
        self
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    pub fn positions(&self) -> &wgpu::Buffer {
        &self.pos
    }

    pub fn velocities(&self) -> &wgpu::Buffer {
        &self.vel
    }

    /// The spatial hash from the last substep: bucket starts and sorted indices.
    pub fn hash(&self) -> (&wgpu::Buffer, &wgpu::Buffer) {
        (&self.cell_start, &self.sorted)
    }

    pub fn set_params(&mut self, queue: &wgpu::Queue, params: &XpbdParams) {
        self.params = *params;
        let u = uniforms(params, self.count, self.table_size);
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));
    }

    /// Moves particle `index` to `position` with the given inverse mass; 0
    /// holds it in place while the solver runs.
    pub fn place(&self, queue: &wgpu::Queue, index: u32, position: Vec3, inv_mass: f32) {
        let value = position.extend(inv_mass).to_array();
        queue.write_buffer(&self.pos, u64::from(index) * 16, bytemuck::bytes_of(&value));
    }

    /// Records `steps` fixed steps, each of `params.substeps` substeps, in one
    /// compute pass. Kernel order matches `physicall_core::xpbd::Xpbd`.
    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder, steps: u32) {
        let k = &self.kernels;
        let particles = self.count.div_ceil(WORKGROUP_SIZE);
        let table = (self.table_size + 1).div_ceil(WORKGROUP_SIZE);
        let buckets = self.table_size.div_ceil(WORKGROUP_SIZE);
        let scan_blocks = self.table_size.div_ceil(SCAN_THREADS);
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("xpbd"),
            timestamp_writes: None,
        });
        let run =
            |pass: &mut wgpu::ComputePass<'_>, pipeline: &wgpu::ComputePipeline, groups: u32| {
                pass.set_pipeline(pipeline);
                pass.dispatch_workgroups(groups, 1, 1);
            };
        pass.set_bind_group(0, &self.particles_group, &[]);
        for _ in 0..steps * self.params.substeps {
            run(&mut pass, &k.predict, particles);

            for (i, &size) in self.colour_sizes.iter().enumerate() {
                pass.set_bind_group(1, &self.constraints_group, &[i as u32 * self.colour_stride]);
                run(&mut pass, &k.solve, size.div_ceil(WORKGROUP_SIZE));
            }

            if self.params.self_collision {
                pass.set_bind_group(1, &self.grid_group, &[]);
                run(&mut pass, &k.hash_clear, table);
                run(&mut pass, &k.hash_count, particles);
                run(&mut pass, &k.hash_scan_blocks, scan_blocks);
                run(&mut pass, &k.hash_scan_sums, 1);
                run(&mut pass, &k.hash_scan_add, scan_blocks);
                run(&mut pass, &k.hash_scatter, particles);
                if self.deterministic {
                    run(&mut pass, &k.hash_sort, buckets);
                }
                for _ in 0..self.params.contact_iterations {
                    run(&mut pass, &k.contacts, particles);
                    run(&mut pass, &k.apply_delta, particles);
                }
            }

            run(&mut pass, &k.finish, particles);
        }
    }
}

const _: () = assert!(
    SCAN_THREADS <= 256,
    "WebGPU guarantees 256 invocations per workgroup"
);
