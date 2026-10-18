use physicall_core::Vec3;
use physicall_core::constraint::ColouredConstraints;
use physicall_core::scene;
use physicall_core::xpbd::{Xpbd, XpbdParams};

/// Catenary parameter `a` with `2 a sinh(span / 2a) = length`, by bisection.
fn catenary_parameter(span: f32, length: f32) -> f32 {
    let f = |a: f64| 2.0 * a * (f64::from(span) / (2.0 * a)).sinh() - f64::from(length);
    let (mut lo, mut hi) = (1e-3_f64, 1e3_f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 { lo = mid } else { hi = mid }
    }
    (0.5 * (lo + hi)) as f32
}

#[test]
fn hanging_chain_settles_into_a_catenary() {
    let (span, length, segments) = (1.0_f32, 1.5_f32, 30);

    // Start as a V with the right total length, so the chain has to find its shape.
    let half = 0.5 * length;
    let depth = (half * half - 0.25 * span * span).sqrt();
    let (left, bottom, right) = (
        Vec3::new(-0.5 * span, 0.0, 0.0),
        Vec3::new(0.0, -depth, 0.0),
        Vec3::new(0.5 * span, 0.0, 0.0),
    );
    let points = (0..=segments).map(|i| {
        let s = length * i as f32 / segments as f32;
        if s <= half {
            left.lerp(bottom, s / half)
        } else {
            bottom.lerp(right, (s - half) / half)
        }
    });
    let (mut particles, links) = scene::chain(points, 0.01);
    particles.inv_mass[0] = 0.0;
    particles.inv_mass[segments] = 0.0;
    let constraints = ColouredConstraints::new(&particles.inv_mass, links);
    let mut sim = Xpbd::new(particles, constraints);

    let params = XpbdParams {
        substeps: 30,
        drag: 3.0,
        ..XpbdParams::default()
    };
    for _ in 0..(20.0 / params.dt) as u32 {
        sim.step(&params);
    }

    let a = catenary_parameter(span, length);
    let sag = |x: f32| a * ((x / a).cosh() - (0.5 * span / a).cosh());
    let worst = sim
        .particles
        .pos
        .iter()
        .map(|p| (p.y - sag(p.x)).abs())
        .fold(0.0, f32::max);
    assert!(
        worst < 1.5e-3 * span,
        "max deviation from catenary {worst:.2e} m"
    );
}
