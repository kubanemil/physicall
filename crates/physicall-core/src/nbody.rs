//! Direct-summation N-body gravity with Plummer softening, integrated with
//! kick-drift-kick leapfrog (symplectic, second order).

use glam::Vec3;

use crate::diagnostics::{self, Diagnostics};
use crate::particles::Particles;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NBodyParams {
    pub dt: f32,
    pub substeps: u32,
    pub g: f32,
    pub softening: f32,
}

impl NBodyParams {
    pub fn substep(&self) -> f32 {
        self.dt / self.substeps.max(1) as f32
    }
}

/// Accelerations and per-particle potentials `phi_i = -G sum_j m_j / r_ij`,
/// summed in index order like the GPU tiles. Every particle needs a finite mass.
pub fn gravity(
    pos: &[Vec3],
    inv_mass: &[f32],
    g: f32,
    softening: f32,
    acc: &mut [Vec3],
    potential: &mut [f32],
) {
    let eps2 = softening * softening;
    for (i, &xi) in pos.iter().enumerate() {
        let mut a = Vec3::ZERO;
        let mut phi = 0.0;
        for (j, (&xj, &wj)) in pos.iter().zip(inv_mass).enumerate() {
            if j == i {
                continue;
            }
            let mj = 1.0 / wj;
            let d = xj - xi;
            let inv_r = 1.0 / (d.length_squared() + eps2).sqrt();
            a += d * (mj * inv_r * inv_r * inv_r);
            phi -= mj * inv_r;
        }
        acc[i] = a * g;
        potential[i] = phi * g;
    }
}

pub fn kick_drift(x: &mut Vec3, v: &mut Vec3, a: Vec3, h: f32) {
    *v += a * (0.5 * h);
    *x += *v * h;
}

#[derive(Clone, Debug)]
pub struct NBody {
    pub particles: Particles,
    pub acc: Vec<Vec3>,
    pub potential: Vec<f32>,
}

impl NBody {
    pub fn new(particles: Particles, params: &NBodyParams) -> Self {
        let n = particles.len();
        let mut body = Self {
            particles,
            acc: vec![Vec3::ZERO; n],
            potential: vec![0.0; n],
        };
        body.update_forces(params);
        body
    }

    pub fn update_forces(&mut self, params: &NBodyParams) {
        let p = &self.particles;
        gravity(
            &p.pos,
            &p.inv_mass,
            params.g,
            params.softening,
            &mut self.acc,
            &mut self.potential,
        );
    }

    pub fn step(&mut self, params: &NBodyParams) {
        let h = params.substep();
        for _ in 0..params.substeps {
            let p = &mut self.particles;
            for ((x, v), &a) in p.pos.iter_mut().zip(&mut p.vel).zip(&self.acc) {
                kick_drift(x, v, a, h);
            }
            self.update_forces(params);
            for (v, &a) in self.particles.vel.iter_mut().zip(&self.acc) {
                *v += a * (0.5 * h);
            }
        }
    }

    pub fn diagnostics(&self) -> Diagnostics {
        let p = &self.particles;
        Diagnostics {
            kinetic: diagnostics::kinetic_energy(&p.vel, &p.inv_mass),
            potential: diagnostics::pair_potential_energy(&self.potential, &p.inv_mass),
            momentum: diagnostics::momentum(&p.vel, &p.inv_mass),
        }
    }
}
