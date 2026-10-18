use physicall_core::clock::FixedClock;
use physicall_core::scene::{self, Scene, SceneKind, Settings};
use physicall_core::xpbd::Xpbd;

fn run(kind: SceneKind, fps: f64, seconds: f64) -> Vec<physicall_core::Vec3> {
    let Scene::Xpbd(s) = Settings::new(kind).build() else {
        panic!("{} is not an XPBD scene", kind.id());
    };
    let mut sim = Xpbd::new(s.particles, s.constraints);
    let mut clock = FixedClock::new(f64::from(scene::STEP), 1000);
    for _ in 0..(fps * seconds).round() as u32 {
        for _ in 0..clock.advance(1.0 / fps) {
            sim.step(&s.params);
        }
    }
    sim.particles.pos
}

#[test]
fn results_are_identical_at_any_frame_rate() {
    for kind in [SceneKind::Rope, SceneKind::Cloth] {
        let reference = run(kind, 60.0, 2.0);
        for fps in [30.0, 45.0, 144.0, 240.0] {
            assert_eq!(run(kind, fps, 2.0), reference, "{} at {fps} fps", kind.id());
        }
    }
}
