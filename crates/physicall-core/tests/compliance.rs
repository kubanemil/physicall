use physicall_core::Vec3;
use physicall_core::constraint::{ColouredConstraints, DistanceConstraint, Material};
use physicall_core::particles::Particles;
use physicall_core::scene;
use physicall_core::xpbd::{Xpbd, XpbdParams};

const G: f32 = 9.81;

fn settle(sim: &mut Xpbd, params: &XpbdParams, seconds: f32) {
    for _ in 0..(seconds / params.dt) as u32 {
        sim.step(params);
    }
}

#[test]
fn compliant_link_stretches_by_hookes_law_at_any_substep_count() {
    let mass = 0.5;
    for compliance in [1e-4, 1e-3, 1e-2] {
        for substeps in [4, 16] {
            let mut p = Particles::default();
            p.push(Vec3::ZERO, Vec3::ZERO, 0.0);
            p.push(Vec3::new(0.0, -1.0, 0.0), Vec3::ZERO, 1.0 / mass);
            let link = DistanceConstraint::between(&p, 0, 1, Material::Stretch);
            let constraints = ColouredConstraints::new(&p.inv_mass, vec![link]);
            let mut sim = Xpbd::new(p, constraints);
            let params = XpbdParams {
                substeps,
                drag: 4.0,
                compliance: [compliance; 4],
                ..XpbdParams::default()
            };
            settle(&mut sim, &params, 10.0);

            let extension = sim.particles.pos[1].length() - 1.0;
            let expected = compliance * mass * G;
            let error = (extension - expected).abs() / expected;
            assert!(
                error < 1e-2,
                "compliance {compliance}, {substeps} substeps: {extension:.3e} m vs {expected:.3e} m"
            );
        }
    }
}

fn hanging_chain() -> Xpbd {
    let segments = 20;
    let points = (0..=segments).map(|i| Vec3::new(0.0, -(i as f32) / segments as f32, 0.0));
    let (mut particles, links) = scene::chain(points, 0.05);
    particles.inv_mass[0] = 0.0;
    let constraints = ColouredConstraints::new(&particles.inv_mass, links);
    Xpbd::new(particles, constraints)
}

fn settled_strain(compliance: f32, substeps: u32) -> f32 {
    let mut sim = hanging_chain();
    let params = XpbdParams {
        substeps,
        drag: 2.0,
        compliance: [compliance; 4],
        ..XpbdParams::default()
    };
    settle(&mut sim, &params, 8.0);
    sim.max_strain()
}

#[test]
fn constraint_error_vanishes_as_compliance_goes_to_zero() {
    let strains: Vec<f32> = [1e-2, 1e-3, 1e-4, 1e-5, 1e-6, 0.0]
        .into_iter()
        .map(|c| settled_strain(c, 20))
        .collect();
    for pair in strains.windows(2) {
        assert!(pair[1] < pair[0], "strain did not decrease: {strains:?}");
    }
    // The top link carries the whole chain: strain = compliance * weight / rest.
    let compliant = strains[0] - strains[strains.len() - 1];
    let expected = 1e-2 * (20.0 * 0.05 * G) / 0.05;
    assert!(
        (compliant / expected - 1.0).abs() < 0.05,
        "strain {compliant:.3e} vs Hooke {expected:.3e}"
    );
}

#[test]
fn rigid_chain_error_shrinks_with_the_square_of_the_substep() {
    let strains: Vec<f32> = [5, 10, 20, 40]
        .into_iter()
        .map(|n| settled_strain(0.0, n))
        .collect();
    for pair in strains.windows(2) {
        let ratio = pair[0] / pair[1];
        assert!(
            (3.5..4.5).contains(&ratio),
            "halving h cut the error by {ratio:.2}, not 4: {strains:?}"
        );
    }
    assert!(
        strains[3] < 2e-3,
        "rigid chain still stretched by {:.2e}",
        strains[3]
    );
}

#[test]
fn tethers_keep_hanging_cloth_from_stretching() {
    let mut settings = scene::Settings::new(scene::SceneKind::Cloth);
    settings.set("stretch", 1e-9);
    let scene::Scene::Xpbd(s) = settings.build() else {
        panic!("cloth is an XPBD scene");
    };
    let mut sim = Xpbd::new(s.particles, s.constraints);
    settle(&mut sim, &s.params, 6.0);
    assert!(
        sim.max_strain() < 0.02,
        "cloth stretched by {:.1}%",
        100.0 * sim.max_strain()
    );
}
