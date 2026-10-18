use std::f32::consts::PI;

use physicall_core::Vec3;
use physicall_core::constraint::{ColouredConstraints, DistanceConstraint, Material};
use physicall_core::particles::Particles;
use physicall_core::xpbd::{Xpbd, XpbdParams};

fn pendulum(length: f32, angle: f32) -> Xpbd {
    let mut p = Particles::default();
    p.push(Vec3::ZERO, Vec3::ZERO, 0.0);
    p.push(
        Vec3::new(length * angle.sin(), -length * angle.cos(), 0.0),
        Vec3::ZERO,
        1.0,
    );
    let link = DistanceConstraint::between(&p, 0, 1, Material::Stretch);
    let constraints = ColouredConstraints::new(&p.inv_mass, vec![link]);
    Xpbd::new(p, constraints)
}

/// Times of downward zero crossings of the bob's x coordinate, interpolated
/// between steps.
fn crossings(sim: &mut Xpbd, params: &XpbdParams, count: usize) -> Vec<f32> {
    let mut out = Vec::new();
    let mut t = 0.0;
    let mut last = sim.particles.pos[1].x;
    while out.len() < count {
        sim.step(params);
        t += params.dt;
        let x = sim.particles.pos[1].x;
        if last > 0.0 && x <= 0.0 {
            out.push(t - params.dt * x / (x - last));
        }
        last = x;
        assert!(t < 60.0, "pendulum stopped swinging");
    }
    out
}

// Ten substeps (h = 1/600 s): with f32 positions near 1 m, much smaller
// substeps round the per-substep gravity displacement and lengthen the period.
#[test]
fn small_angle_pendulum_period_matches_analytic() {
    let (length, g, amplitude) = (1.0, 9.81, 0.05);
    let params = XpbdParams {
        substeps: 10,
        gravity: Vec3::new(0.0, -g, 0.0),
        ..XpbdParams::default()
    };
    let mut sim = pendulum(length, amplitude);
    let t = crossings(&mut sim, &params, 6);
    let measured = (t[5] - t[0]) / 5.0;
    // Includes the first finite-amplitude correction, 1 + theta^2 / 16.
    let expected = 2.0 * PI * (length / g).sqrt() * (1.0 + amplitude * amplitude / 16.0);
    let error = (measured - expected).abs() / expected;
    assert!(
        error < 1e-3,
        "period {measured:.5} s vs {expected:.5} s ({error:.2e})"
    );
}

#[test]
fn pendulum_link_stays_rigid_without_compliance() {
    let params = XpbdParams {
        substeps: 20,
        ..XpbdParams::default()
    };
    let mut sim = pendulum(1.0, 1.2);
    for _ in 0..300 {
        sim.step(&params);
        assert!(sim.max_strain() < 1e-5, "strain {}", sim.max_strain());
    }
}
