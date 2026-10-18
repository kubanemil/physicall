//! Extended Position Based Dynamics (Macklin, Müller, Chentanez 2016) with
//! substepping and one solver iteration per substep (Macklin et al. 2019,
//! "Small Steps in Physics Simulation").

use glam::Vec3;

use crate::clock;
use crate::constraint::ColouredConstraints;
use crate::contact::{self, Bounds};
use crate::hash::SpatialHash;
use crate::particles::Particles;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XpbdParams {
    /// Fixed step in seconds; each step runs `substeps` substeps.
    pub dt: f32,
    pub substeps: u32,
    pub gravity: Vec3,
    /// Linear air drag rate in 1/s: velocity decays as `exp(-drag * t)`.
    /// It acts on velocity only, so it never changes a resting state.
    pub drag: f32,
    /// Compliance (inverse stiffness, m/N) per `Material`.
    pub compliance: [f32; 4],
    pub radius: f32,
    pub bounds: Bounds,
    pub friction: f32,
    pub self_collision: bool,
    /// Jacobi contact passes per substep. More substeps converge faster
    /// (Macklin et al. 2019), so one is the default.
    pub contact_iterations: u32,
    /// Over-relaxation of averaged contact corrections. Above 1 it injects
    /// energy into piles, because velocities are derived from positions.
    pub contact_relaxation: f32,
}

impl Default for XpbdParams {
    fn default() -> Self {
        Self {
            dt: 1.0 / 60.0,
            substeps: 10,
            gravity: Vec3::new(0.0, -9.81, 0.0),
            drag: 0.0,
            compliance: [0.0; 4],
            radius: 0.01,
            bounds: Bounds::default(),
            friction: 0.0,
            self_collision: false,
            contact_iterations: 1,
            contact_relaxation: 1.0,
        }
    }
}

/// Per-substep constants computed once, so the CPU and GPU solvers start
/// every substep from identical numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Substep {
    pub h: f32,
    pub inv_h: f32,
    pub damping: f32,
    /// Compliance divided by h², the alpha-tilde of the XPBD paper.
    pub alpha: [f32; 4],
    /// The contact grid's cells are one particle diameter wide.
    pub inv_cell_size: f32,
}

impl Substep {
    pub fn new(params: &XpbdParams) -> Self {
        let h = clock::substep(params.dt, params.substeps);
        Self {
            h,
            inv_h: 1.0 / h,
            damping: (-params.drag * h).exp(),
            alpha: params.compliance.map(|c| c / (h * h)),
            inv_cell_size: 1.0 / (2.0 * params.radius),
        }
    }
}

pub fn predict(x: &mut Vec3, prev: &mut Vec3, v: &mut Vec3, w: f32, gravity: Vec3, k: &Substep) {
    *prev = *x;
    if w == 0.0 {
        return;
    }
    *v = *v * k.damping + gravity * k.h;
    *x += *v * k.h;
}

/// One XPBD update of a distance constraint with lambda starting at zero.
/// Returns the position corrections for `a` and `b`. A unilateral constraint
/// only acts when stretched.
pub fn solve_distance(
    xa: Vec3,
    xb: Vec3,
    wa: f32,
    wb: f32,
    rest: f32,
    alpha: f32,
    unilateral: bool,
) -> (Vec3, Vec3) {
    let w = wa + wb;
    let d = xa - xb;
    let len = d.length();
    if w == 0.0 || len == 0.0 || (unilateral && len <= rest) {
        return (Vec3::ZERO, Vec3::ZERO);
    }
    let dlambda = -(len - rest) / (w + alpha);
    let n = d / len;
    (n * (wa * dlambda), n * (-wb * dlambda))
}

pub fn velocity(x: Vec3, prev: Vec3, w: f32, k: &Substep) -> Vec3 {
    if w == 0.0 {
        Vec3::ZERO
    } else {
        (x - prev) * k.inv_h
    }
}

/// CPU reference solver. The GPU solver in `physicall-gpu` runs the same
/// passes in the same order.
#[derive(Clone, Debug)]
pub struct Xpbd {
    pub particles: Particles,
    pub constraints: ColouredConstraints,
    hash: SpatialHash,
    delta: Vec<Vec3>,
}

impl Xpbd {
    pub fn new(particles: Particles, constraints: ColouredConstraints) -> Self {
        let hash = SpatialHash::new(SpatialHash::table_size_for(particles.len()));
        let delta = vec![Vec3::ZERO; particles.len()];
        Self {
            particles,
            constraints,
            hash,
            delta,
        }
    }

    pub fn step(&mut self, params: &XpbdParams) {
        let k = Substep::new(params);
        for _ in 0..params.substeps {
            self.substep(params, &k);
        }
    }

    fn substep(&mut self, params: &XpbdParams, k: &Substep) {
        let p = &mut self.particles;
        for i in 0..p.len() {
            predict(
                &mut p.pos[i],
                &mut p.prev[i],
                &mut p.vel[i],
                p.inv_mass[i],
                params.gravity,
                k,
            );
        }

        for colour in self.constraints.colours() {
            for c in colour {
                let (a, b) = (c.a as usize, c.b as usize);
                let alpha = k.alpha[c.material as usize];
                let (wa, wb) = (p.inv_mass[a], p.inv_mass[b]);
                let (da, db) = solve_distance(
                    p.pos[a],
                    p.pos[b],
                    wa,
                    wb,
                    c.rest,
                    alpha,
                    c.material.is_unilateral(),
                );
                // Static ends are shared within a colour, so they are never written.
                if wa > 0.0 {
                    p.pos[a] += da;
                }
                if wb > 0.0 {
                    p.pos[b] += db;
                }
            }
        }

        if params.self_collision {
            self.hash.build(&p.pos, k.inv_cell_size);
            for _ in 0..params.contact_iterations {
                for i in 0..p.len() {
                    self.delta[i] = contact::contact_delta(
                        p,
                        &self.hash,
                        i,
                        params.radius,
                        params.friction,
                        params.contact_relaxation,
                    );
                }
                for (x, d) in p.pos.iter_mut().zip(&self.delta) {
                    *x += *d;
                }
            }
        }

        for i in 0..p.len() {
            if p.inv_mass[i] != 0.0 {
                contact::collide_bounds(
                    &mut p.pos[i],
                    p.prev[i],
                    params.radius,
                    &params.bounds,
                    params.friction,
                );
            }
            p.vel[i] = velocity(p.pos[i], p.prev[i], p.inv_mass[i], k);
        }
    }

    /// Largest relative violation `|len - rest| / rest` over the two-sided
    /// constraints.
    pub fn max_strain(&self) -> f32 {
        self.constraints
            .as_slice()
            .iter()
            .filter(|c| !c.material.is_unilateral())
            .map(|c| {
                let len =
                    self.particles.pos[c.a as usize].distance(self.particles.pos[c.b as usize]);
                (len - c.rest).abs() / c.rest
            })
            .fold(0.0, f32::max)
    }
}
