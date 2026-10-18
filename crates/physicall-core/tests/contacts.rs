use physicall_core::Vec3;
use physicall_core::constraint::ColouredConstraints;
use physicall_core::contact::Bounds;
use physicall_core::diagnostics;
use physicall_core::hash::SpatialHash;
use physicall_core::particles::Particles;
use physicall_core::scene;
use physicall_core::xpbd::{Xpbd, XpbdParams};

const RADIUS: f32 = 0.02;
const HALF_WIDTH: f32 = 0.3;

fn pile_params() -> XpbdParams {
    XpbdParams {
        substeps: 10,
        radius: RADIUS,
        bounds: Bounds {
            floor: Some(0.0),
            half_width: Some(HALF_WIDTH),
        },
        friction: 0.3,
        self_collision: true,
        contact_iterations: 2,
        contact_relaxation: 1.5,
        ..XpbdParams::default()
    }
}

#[test]
fn falling_pile_comes_to_rest_without_deep_overlaps() {
    let particles = scene::pile(1200, RADIUS, HALF_WIDTH, 9);
    let mut sim = Xpbd::new(particles, ColouredConstraints::default());
    let params = pile_params();
    for _ in 0..240 {
        sim.step(&params);
    }

    let p = &sim.particles;
    let mut hash = SpatialHash::new(SpatialHash::table_size_for(p.len()));
    hash.build(&p.pos, 2.0 * RADIUS);
    let worst = (0..p.len())
        .flat_map(|i| {
            hash.neighbours(&p.pos, i, 2.0 * RADIUS)
                .into_iter()
                .map(move |j| (i, j as usize))
        })
        .map(|(i, j)| 2.0 * RADIUS - p.pos[i].distance(p.pos[j]))
        .fold(0.0, f32::max);
    assert!(
        worst < 0.1 * RADIUS,
        "deepest overlap {:.1}% of radius",
        100.0 * worst / RADIUS
    );

    for x in &p.pos {
        assert!(
            x.y >= RADIUS - 1e-4 && x.x.abs() <= HALF_WIDTH && x.z.abs() <= HALF_WIDTH,
            "escaped to {x}"
        );
    }
    let kinetic = diagnostics::kinetic_energy(&p.vel, &p.inv_mass) / p.len() as f64;
    assert!(
        kinetic < 1e-6,
        "pile still moving: {kinetic:.2e} J per particle"
    );
}

#[test]
fn friction_stops_a_sliding_particle_on_the_floor() {
    let mut p = Particles::default();
    p.push(Vec3::new(0.0, RADIUS, 0.0), Vec3::new(1.0, 0.0, 0.0), 1.0);
    let mut sim = Xpbd::new(p, ColouredConstraints::default());
    let params = XpbdParams {
        radius: RADIUS,
        bounds: Bounds {
            floor: Some(0.0),
            half_width: None,
        },
        friction: 0.5,
        ..XpbdParams::default()
    };
    for _ in 0..60 {
        sim.step(&params);
    }
    // Coulomb friction stops it after v^2 / (2 mu g), about 0.1 m.
    let travelled = sim.particles.pos[0].x;
    assert!(
        sim.particles.vel[0].length() < 1e-3,
        "still sliding at {}",
        sim.particles.vel[0]
    );
    assert!(
        (travelled - 1.0 / (2.0 * 0.5 * 9.81)).abs() < 0.02,
        "slid {travelled:.3} m"
    );
}

#[test]
fn equal_particles_swap_momentum_in_a_head_on_collision_without_losing_it() {
    let mut p = Particles::default();
    p.push(Vec3::new(-0.1, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 1.0);
    p.push(Vec3::new(0.1, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0), 1.0);
    let mut sim = Xpbd::new(p, ColouredConstraints::default());
    let params = XpbdParams {
        gravity: Vec3::ZERO,
        radius: RADIUS,
        self_collision: true,
        ..XpbdParams::default()
    };
    for _ in 0..30 {
        sim.step(&params);
    }
    let p = &sim.particles;
    let momentum = diagnostics::momentum(&p.vel, &p.inv_mass);
    assert!(momentum.length() < 1e-6, "net momentum {momentum}");
    assert!(
        p.pos[0].distance(p.pos[1]) >= 2.0 * RADIUS - 1e-4,
        "particles passed through each other"
    );
}
