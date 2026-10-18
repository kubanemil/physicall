use std::f32::consts::PI;

use physicall_core::Vec3;
use physicall_core::nbody::{NBody, NBodyParams};
use physicall_core::particles::Particles;
use physicall_core::scene;

/// Two equal masses on an eccentric orbit (G = 1, M = 1, a = 1), started at
/// apoapsis in the centre-of-mass frame.
fn binary(eccentricity: f32) -> Particles {
    let (a, m) = (1.0, 0.5);
    let r = a * (1.0 + eccentricity);
    let v = ((1.0 - eccentricity) / r).sqrt();
    let mut p = Particles::default();
    p.push(
        Vec3::new(-0.5 * r, 0.0, 0.0),
        Vec3::new(0.0, 0.0, -0.5 * v),
        1.0 / m,
    );
    p.push(
        Vec3::new(0.5 * r, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 0.5 * v),
        1.0 / m,
    );
    p
}

#[test]
fn kepler_orbit_energy_error_stays_bounded_over_many_orbits() {
    let period = 2.0 * PI;
    let steps_per_orbit = 2000;
    let orbits = 20;
    let params = NBodyParams {
        dt: period / steps_per_orbit as f32,
        substeps: 1,
        g: 1.0,
        softening: 0.0,
    };
    let mut sim = NBody::new(binary(0.5), &params);
    let e0 = sim.diagnostics().total();

    let mut worst = 0.0_f64;
    let mut after_first = 0.0;
    for orbit in 0..orbits {
        for _ in 0..steps_per_orbit {
            sim.step(&params);
            worst = worst.max(((sim.diagnostics().total() - e0) / e0).abs());
        }
        if orbit == 0 {
            after_first = sim.diagnostics().total();
        }
    }
    let drift = ((sim.diagnostics().total() - after_first) / e0).abs();
    assert!(worst < 1e-3, "energy error reached {worst:.2e}");
    assert!(
        drift < 1e-4,
        "energy drifted {drift:.2e} between orbit 1 and {orbits}"
    );
}

#[test]
fn kepler_orbit_returns_to_apoapsis_after_one_period() {
    let steps = 2000;
    let params = NBodyParams {
        dt: 2.0 * PI / steps as f32,
        substeps: 1,
        g: 1.0,
        softening: 0.0,
    };
    let start = binary(0.5);
    let mut sim = NBody::new(start.clone(), &params);
    for _ in 0..steps {
        sim.step(&params);
    }
    let error = sim.particles.pos[1].distance(start.pos[1]);
    assert!(
        error < 1e-2,
        "body is {error:.2e} away from its start after one period"
    );
}

#[test]
fn nbody_conserves_momentum() {
    let mut particles = scene::cluster(256, 0.5, 11);
    for (i, v) in particles.vel.iter_mut().enumerate() {
        *v += Vec3::new(0.3, -0.1, 0.2) * (i % 3) as f32;
    }
    let params = NBodyParams {
        dt: 1.0 / 60.0,
        substeps: 2,
        g: 1.0,
        softening: 0.03,
    };
    let mut sim = NBody::new(particles, &params);
    let start = sim.diagnostics();
    let scale: f64 = sim
        .particles
        .vel
        .iter()
        .zip(&sim.particles.inv_mass)
        .map(|(v, &w)| f64::from(v.length() / w))
        .sum();
    for _ in 0..300 {
        sim.step(&params);
    }
    let change = (sim.diagnostics().momentum - start.momentum).length() / scale;
    assert!(
        change < 1e-5,
        "momentum changed by {change:.2e} of its scale"
    );
}

#[test]
fn nbody_cluster_conserves_energy_with_softening() {
    let params = NBodyParams {
        dt: 1.0 / 60.0,
        substeps: 2,
        g: 1.0,
        softening: 0.05,
    };
    let mut sim = NBody::new(scene::cluster(512, 0.6, 5), &params);
    let e0 = sim.diagnostics().total();
    for _ in 0..240 {
        sim.step(&params);
    }
    let error = ((sim.diagnostics().total() - e0) / e0).abs();
    assert!(error < 5e-3, "energy error {error:.2e} after 4 time units");
}
