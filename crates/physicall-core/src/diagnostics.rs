use glam::{DVec3, Vec3};

/// Conserved quantities, accumulated in f64. Static particles are excluded.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Diagnostics {
    pub kinetic: f64,
    pub potential: f64,
    pub momentum: DVec3,
}

impl Diagnostics {
    pub fn total(&self) -> f64 {
        self.kinetic + self.potential
    }
}

fn masses(inv_mass: &[f32]) -> impl Iterator<Item = Option<f64>> + '_ {
    inv_mass
        .iter()
        .map(|&w| (w > 0.0).then(|| 1.0 / f64::from(w)))
}

pub fn kinetic_energy(vel: &[Vec3], inv_mass: &[f32]) -> f64 {
    vel.iter()
        .zip(masses(inv_mass))
        .filter_map(|(v, m)| m.map(|m| 0.5 * m * v.as_dvec3().length_squared()))
        .sum()
}

pub fn momentum(vel: &[Vec3], inv_mass: &[f32]) -> DVec3 {
    vel.iter()
        .zip(masses(inv_mass))
        .filter_map(|(v, m)| m.map(|m| v.as_dvec3() * m))
        .sum()
}

/// Potential energy in a uniform field: `-sum m g.x`.
pub fn field_potential_energy(pos: &[Vec3], inv_mass: &[f32], gravity: Vec3) -> f64 {
    let g = gravity.as_dvec3();
    pos.iter()
        .zip(masses(inv_mass))
        .filter_map(|(x, m)| m.map(|m| -m * g.dot(x.as_dvec3())))
        .sum()
}

/// Pairwise potential energy from per-particle potentials: `1/2 sum m phi`.
pub fn pair_potential_energy(potential: &[f32], inv_mass: &[f32]) -> f64 {
    potential
        .iter()
        .zip(masses(inv_mass))
        .filter_map(|(&phi, m)| m.map(|m| 0.5 * m * f64::from(phi)))
        .sum()
}
