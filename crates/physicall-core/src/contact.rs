use glam::Vec3;

use crate::hash::SpatialHash;
use crate::particles::Particles;

/// Axis-aligned container: a floor and optional square walls around the y axis.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounds {
    pub floor: Option<f32>,
    pub half_width: Option<f32>,
}

/// Position-based Coulomb friction (Macklin et al. 2014, "Unified Particle
/// Physics for Real-Time Applications", sec. 6.1) with equal static and
/// kinetic coefficients. Returns the tangential displacement to remove.
pub fn friction(tangential: Vec3, penetration: f32, mu: f32) -> Vec3 {
    let limit = mu * penetration;
    let len = tangential.length();
    if len > limit {
        tangential * (limit / len)
    } else {
        tangential
    }
}

/// Keeps `x` on the positive side of the plane `dot(x, normal) >= offset`.
pub fn collide_plane(x: &mut Vec3, prev: Vec3, normal: Vec3, offset: f32, mu: f32) {
    let penetration = offset - x.dot(normal);
    if penetration <= 0.0 {
        return;
    }
    *x += normal * penetration;
    let moved = *x - prev;
    let tangential = moved - normal * moved.dot(normal);
    *x -= friction(tangential, penetration, mu);
}

pub fn collide_bounds(x: &mut Vec3, prev: Vec3, radius: f32, bounds: &Bounds, mu: f32) {
    if let Some(floor) = bounds.floor {
        collide_plane(x, prev, Vec3::Y, floor + radius, mu);
    }
    if let Some(half_width) = bounds.half_width {
        let offset = radius - half_width;
        for normal in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
            collide_plane(x, prev, normal, offset, mu);
        }
    }
}

/// Jacobi contact correction for particle `i`: every overlapping neighbour
/// contributes a mass-weighted push plus friction, and the sum is averaged
/// over the contact count and scaled by `relaxation` (Macklin et al. 2014,
/// sec. 4.2). Reads positions only, so all particles can run in parallel.
pub fn contact_delta(
    particles: &Particles,
    hash: &SpatialHash,
    i: usize,
    radius: f32,
    mu: f32,
    relaxation: f32,
) -> Vec3 {
    let wi = particles.inv_mass[i];
    if wi == 0.0 {
        return Vec3::ZERO;
    }
    let xi = particles.pos[i];
    let moved_i = xi - particles.prev[i];
    let contact = 2.0 * radius;

    let mut sum = Vec3::ZERO;
    let mut count = 0u32;
    hash.for_each_candidate(xi, |j| {
        let j = j as usize;
        if j == i {
            return;
        }
        let d = xi - particles.pos[j];
        let dist2 = d.length_squared();
        if dist2 >= contact * contact || dist2 == 0.0 {
            return;
        }
        let dist = dist2.sqrt();
        let normal = d / dist;
        let penetration = contact - dist;
        let share = wi / (wi + particles.inv_mass[j]);

        let relative = moved_i - (particles.pos[j] - particles.prev[j]);
        let tangential = relative - normal * relative.dot(normal);
        sum += (normal * penetration - friction(tangential, penetration, mu)) * share;
        count += 1;
    });

    if count == 0 {
        Vec3::ZERO
    } else {
        sum * (relaxation / count as f32)
    }
}
