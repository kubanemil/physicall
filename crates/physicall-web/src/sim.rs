//! A scene's GPU simulation, its draw data and its diagnostics.

use glam::Vec3;
use physicall_core::constraint::Material;
use physicall_core::diagnostics::{self, Diagnostics};
use physicall_core::scene::{Look, Scene, Settings};
use physicall_gpu::nbody::{NBodyGpu, NBodyKernels};
use physicall_gpu::pack::unpack;
use physicall_gpu::readback::Readback;
use physicall_gpu::xpbd::{XpbdGpu, XpbdKernels};

use crate::render::{Drawable, RenderUniforms, Renderer, STYLE_ALL, STYLE_STATIC_ONLY};

pub struct Kernels {
    pub xpbd: XpbdKernels,
    pub nbody: NBodyKernels,
}

pub enum Solver {
    Xpbd { gpu: Box<XpbdGpu>, gravity: Vec3 },
    NBody { gpu: NBodyGpu },
}

/// Particle state read back from the GPU.
pub struct Snapshot {
    pub pos: Vec<Vec3>,
    pub inv_mass: Vec<f32>,
    pub vel: Vec<Vec3>,
    pub potential: Vec<f32>,
}

pub struct Simulation {
    pub solver: Solver,
    pub drawable: Drawable,
    /// Inverse masses at build time, to restore a particle the user lets go of.
    pub inv_mass: Vec<f32>,
    pub style: Style,
    readback: Readback,
}

#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub radius: f32,
    pub spheres: u32,
    pub speed_scale: f32,
    pub columns: u32,
    pub floor: f32,
    pub half_width: f32,
}

impl Simulation {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        kernels: &Kernels,
        renderer: &Renderer,
        scene: &Scene,
    ) -> Self {
        match scene {
            Scene::Xpbd(s) => {
                let gpu = Box::new(XpbdGpu::new(
                    device,
                    &kernels.xpbd,
                    &s.particles,
                    &s.constraints,
                    &s.params,
                ));
                let lines: Vec<u32> = match s.look {
                    Look::Chain => s
                        .constraints
                        .as_slice()
                        .iter()
                        .filter(|c| c.material == Material::Stretch)
                        .flat_map(|c| [c.a, c.b])
                        .collect(),
                    Look::Cloth | Look::Spheres => Vec::new(),
                };
                let triangles: Vec<u32> = s.triangles.iter().flatten().copied().collect();
                let n = s.particles.len();
                let drawable = Drawable {
                    bind_group: renderer.bind(device, gpu.positions(), gpu.velocities()),
                    count: n as u32,
                    spheres: true,
                    floor: s.params.bounds.floor.is_some(),
                    walls: s.params.bounds.half_width.is_some(),
                    lines: Renderer::index_buffer(device, &lines),
                    triangles: Renderer::index_buffer(device, &triangles),
                };
                let style = Style {
                    radius: match s.look {
                        Look::Cloth => 1.5 * s.params.radius,
                        Look::Chain | Look::Spheres => s.params.radius,
                    },
                    spheres: if s.look == Look::Cloth {
                        STYLE_STATIC_ONLY
                    } else {
                        STYLE_ALL
                    },
                    speed_scale: if s.look == Look::Spheres { 0.6 } else { 0.25 },
                    columns: (n as f32).sqrt().round() as u32,
                    floor: s.params.bounds.floor.unwrap_or(0.0),
                    half_width: s.params.bounds.half_width.unwrap_or(0.0),
                };
                Self {
                    readback: Readback::new(device, 32 * n as u64),
                    inv_mass: s.particles.inv_mass.clone(),
                    solver: Solver::Xpbd {
                        gpu,
                        gravity: s.params.gravity,
                    },
                    drawable,
                    style,
                }
            }
            Scene::NBody(s) => {
                let gpu = NBodyGpu::new(device, queue, &kernels.nbody, &s.particles, &s.params);
                let n = s.particles.len();
                let drawable = Drawable {
                    bind_group: renderer.bind(device, gpu.positions(), gpu.velocities()),
                    count: n as u32,
                    spheres: true,
                    floor: false,
                    walls: false,
                    lines: None,
                    triangles: None,
                };
                Self {
                    readback: Readback::new(device, 48 * n as u64),
                    inv_mass: s.particles.inv_mass.clone(),
                    solver: Solver::NBody { gpu },
                    drawable,
                    style: Style {
                        radius: 0.012,
                        spheres: STYLE_ALL,
                        speed_scale: 0.9,
                        columns: 0,
                        floor: 0.0,
                        half_width: 0.0,
                    },
                }
            }
        }
    }

    /// Pushes live parameter changes to the GPU.
    pub fn apply(&mut self, queue: &wgpu::Queue, settings: &Settings) {
        match &mut self.solver {
            Solver::Xpbd { gpu, gravity } => {
                let params = settings.xpbd_params();
                gpu.set_params(queue, &params);
                *gravity = params.gravity;
            }
            Solver::NBody { gpu } => gpu.set_params(queue, &settings.nbody_params()),
        }
    }

    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder, steps: u32) {
        match &self.solver {
            Solver::Xpbd { gpu, .. } => gpu.encode(encoder, steps),
            Solver::NBody { gpu } => gpu.encode(encoder, steps),
        }
    }

    /// Only XPBD particles can be held: N-body gravity needs finite masses.
    pub fn place(&self, queue: &wgpu::Queue, index: u32, position: Vec3, inv_mass: f32) {
        if let Solver::Xpbd { gpu, .. } = &self.solver {
            gpu.place(queue, index, position, inv_mass);
        }
    }

    pub fn can_grab(&self) -> bool {
        matches!(self.solver, Solver::Xpbd { .. })
    }

    pub fn uniforms(&self, highlight: Option<u32>) -> RenderUniforms {
        RenderUniforms {
            radius: self.style.radius,
            highlight: highlight.unwrap_or(u32::MAX),
            style: self.style.spheres,
            speed_scale: self.style.speed_scale,
            floor: self.style.floor,
            half_width: self.style.half_width,
            columns: self.style.columns,
            ..Default::default()
        }
    }

    pub fn record_readback(&self, encoder: &mut wgpu::CommandEncoder) {
        match &self.solver {
            Solver::Xpbd { gpu, .. } => self
                .readback
                .record(encoder, &[gpu.positions(), gpu.velocities()]),
            Solver::NBody { gpu } => self.readback.record(
                encoder,
                &[gpu.positions(), gpu.velocities(), gpu.accelerations()],
            ),
        };
    }

    pub fn map_readback(&self) {
        self.readback.map();
    }

    pub fn take_snapshot(&self) -> Option<Snapshot> {
        let bytes = self.readback.take()?;
        let n = self.inv_mass.len() * 16;
        if bytes.len() < 2 * n {
            return None;
        }
        let (pos, inv_mass) = unpack(&bytes[..n]);
        let (vel, _) = unpack(&bytes[n..2 * n]);
        let potential = if bytes.len() >= 3 * n {
            unpack(&bytes[2 * n..3 * n]).1
        } else {
            Vec::new()
        };
        Some(Snapshot {
            pos,
            inv_mass,
            vel,
            potential,
        })
    }

    pub fn diagnostics(&self, s: &Snapshot) -> Diagnostics {
        let potential = match &self.solver {
            Solver::Xpbd { gravity, .. } => {
                diagnostics::field_potential_energy(&s.pos, &s.inv_mass, *gravity)
            }
            Solver::NBody { .. } => diagnostics::pair_potential_energy(&s.potential, &s.inv_mass),
        };
        Diagnostics {
            kinetic: diagnostics::kinetic_energy(&s.vel, &s.inv_mass),
            potential,
            momentum: diagnostics::momentum(&s.vel, &s.inv_mass),
        }
    }
}
