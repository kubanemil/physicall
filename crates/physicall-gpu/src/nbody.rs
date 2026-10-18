use physicall_core::nbody::NBodyParams;
use physicall_core::particles::Particles;

use crate::layout::{NBODY_TILE, NBodyUniforms, WORKGROUP_SIZE};
use crate::pack::{pack, pack_zero_w};
use crate::shader;
use crate::util::{self, PARTICLE_USAGE, UNIFORM_USAGE};

const COMPUTE: wgpu::ShaderStages = wgpu::ShaderStages::COMPUTE;

pub fn uniforms(params: &NBodyParams, count: u32) -> NBodyUniforms {
    NBodyUniforms {
        h: params.substep(),
        g: params.g,
        eps2: params.softening * params.softening,
        count,
    }
}

#[derive(Clone, Debug)]
pub struct NBodyKernels {
    layout: wgpu::BindGroupLayout,
    kick_drift: wgpu::ComputePipeline,
    force_kick: wgpu::ComputePipeline,
}

impl NBodyKernels {
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("nbody"),
            entries: &[
                util::uniform_entry(0, false, COMPUTE),
                util::storage_entry(1, false, COMPUTE),
                util::storage_entry(2, false, COMPUTE),
                util::storage_entry(3, false, COMPUTE),
            ],
        });
        let pipeline_layout = util::pipeline_layout(device, "nbody", &[&layout]);
        let module = shader::module(device, "nbody", shader::NBODY);
        Self {
            kick_drift: util::compute_pipeline(device, &pipeline_layout, &module, "kick_drift"),
            force_kick: util::compute_pipeline(device, &pipeline_layout, &module, "force_kick"),
            layout,
        }
    }
}

pub struct NBodyGpu {
    kernels: NBodyKernels,
    params: NBodyParams,
    count: u32,
    pos: wgpu::Buffer,
    vel: wgpu::Buffer,
    acc: wgpu::Buffer,
    uniforms: wgpu::Buffer,
    group: wgpu::BindGroup,
}

impl NBodyGpu {
    /// Uploads the particles and computes their initial accelerations on the
    /// GPU, which leapfrog's first half kick needs.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        kernels: &NBodyKernels,
        particles: &Particles,
        params: &NBodyParams,
    ) -> Self {
        let count = particles.len() as u32;
        let pos = util::buffer(
            device,
            "nbody pos",
            bytemuck::cast_slice(&pack(&particles.pos, &particles.inv_mass)),
            PARTICLE_USAGE,
        );
        let vel = util::buffer(
            device,
            "nbody vel",
            bytemuck::cast_slice(&pack_zero_w(&particles.vel)),
            PARTICLE_USAGE,
        );
        let acc = util::buffer(
            device,
            "nbody acc",
            &vec![0u8; 16 * count as usize],
            PARTICLE_USAGE,
        );
        let group_with = |uniforms: &wgpu::Buffer| {
            util::bind_group(
                device,
                "nbody",
                &kernels.layout,
                &[
                    uniforms.as_entire_binding(),
                    pos.as_entire_binding(),
                    vel.as_entire_binding(),
                    acc.as_entire_binding(),
                ],
            )
        };

        let prime = NBodyUniforms {
            h: 0.0,
            ..uniforms(params, count)
        };
        let prime_uniforms = util::buffer(
            device,
            "nbody prime",
            bytemuck::bytes_of(&prime),
            wgpu::BufferUsages::UNIFORM,
        );
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_bind_group(0, &group_with(&prime_uniforms), &[]);
            pass.set_pipeline(&kernels.force_kick);
            pass.dispatch_workgroups(count.div_ceil(NBODY_TILE), 1, 1);
        }
        queue.submit([encoder.finish()]);

        let uniforms = util::buffer(
            device,
            "nbody uniforms",
            bytemuck::bytes_of(&uniforms(params, count)),
            UNIFORM_USAGE,
        );
        Self {
            group: group_with(&uniforms),
            kernels: kernels.clone(),
            params: *params,
            count,
            pos,
            vel,
            acc,
            uniforms,
        }
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

    /// Accelerations in `xyz`, per-particle potential in `w`.
    pub fn accelerations(&self) -> &wgpu::Buffer {
        &self.acc
    }

    pub fn set_params(&mut self, queue: &wgpu::Queue, params: &NBodyParams) {
        self.params = *params;
        queue.write_buffer(
            &self.uniforms,
            0,
            bytemuck::bytes_of(&uniforms(params, self.count)),
        );
    }

    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder, steps: u32) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("nbody"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &self.group, &[]);
        for _ in 0..steps * self.params.substeps {
            pass.set_pipeline(&self.kernels.kick_drift);
            pass.dispatch_workgroups(self.count.div_ceil(WORKGROUP_SIZE), 1, 1);
            pass.set_pipeline(&self.kernels.force_kick);
            pass.dispatch_workgroups(self.count.div_ceil(NBODY_TILE), 1, 1);
        }
    }
}
