//! The GPU kernels against the CPU reference. These need a GPU adapter (a
//! software one such as lavapipe works). Without one they print a notice and
//! pass, unless PHYSICALL_REQUIRE_GPU is set, as it is in CI.

use physicall_core::Vec3;
use physicall_core::constraint::ColouredConstraints;
use physicall_core::contact::Bounds;
use physicall_core::hash::SpatialHash;
use physicall_core::nbody::NBody;
use physicall_core::particles::Particles;
use physicall_core::rng::Rng;
use physicall_core::scene::{self, Scene, SceneKind, Settings};
use physicall_core::xpbd::{Xpbd, XpbdParams};
use physicall_gpu::nbody::{NBodyGpu, NBodyKernels};
use physicall_gpu::pack::unpack;
use physicall_gpu::readback::read_buffers;
use physicall_gpu::xpbd::{XpbdGpu, XpbdKernels};

fn gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = match pollster::block_on(
        instance.request_adapter(&wgpu::RequestAdapterOptions::default()),
    ) {
        Ok(adapter) => adapter,
        Err(e) => {
            assert!(
                std::env::var_os("PHYSICALL_REQUIRE_GPU").is_none(),
                "PHYSICALL_REQUIRE_GPU is set but no GPU adapter was found: {e}"
            );
            eprintln!(
                "skipping GPU test: no GPU adapter ({e}); set PHYSICALL_REQUIRE_GPU=1 to fail instead"
            );
            return None;
        }
    };
    eprintln!(
        "GPU adapter: {} ({:?})",
        adapter.get_info().name,
        adapter.get_info().backend
    );
    let device = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()));
    Some(device.unwrap_or_else(|e| panic!("adapter refused a default device: {e}")))
}

fn max_distance(a: &[Vec3], b: &[Vec3]) -> f32 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(a, b)| a.distance(*b))
        .fold(0.0, f32::max)
}

fn xpbd_scene(kind: SceneKind, tweak: impl FnOnce(&mut Settings)) -> scene::XpbdScene {
    let mut settings = Settings::new(kind);
    tweak(&mut settings);
    match settings.build() {
        Scene::Xpbd(s) => s,
        Scene::NBody(_) => panic!("{} is not an XPBD scene", kind.id()),
    }
}

/// Runs `steps` fixed steps on both solvers and returns both final states.
fn run_both(
    particles: &Particles,
    constraints: &ColouredConstraints,
    params: &XpbdParams,
    steps: u32,
) -> Option<(Vec<Vec3>, Xpbd)> {
    let (device, queue) = gpu()?;
    let kernels = XpbdKernels::new(&device);
    let sim = XpbdGpu::new(&device, &kernels, particles, constraints, params);
    let mut encoder = device.create_command_encoder(&Default::default());
    sim.encode(&mut encoder, steps);
    queue.submit([encoder.finish()]);
    let (gpu_pos, _) = unpack(&read_buffers(&device, &queue, &[sim.positions()]));

    let mut cpu = Xpbd::new(particles.clone(), constraints.clone());
    for _ in 0..steps {
        cpu.step(params);
    }
    Some((gpu_pos, cpu))
}

fn scene_gap(scene: &scene::XpbdScene, steps: u32) -> Option<f32> {
    let (gpu_pos, cpu) = run_both(&scene.particles, &scene.constraints, &scene.params, steps)?;
    Some(max_distance(&gpu_pos, &cpu.particles.pos))
}

// Both runs start one rounding step apart (GPUs fuse multiply-adds) and a
// folding cloth amplifies that, so compare over a third of a second.
#[test]
fn gpu_cloth_matches_the_cpu_reference() {
    let scene = xpbd_scene(SceneKind::Cloth, |s| {
        s.set("resolution", 24.0);
    });
    if let Some(gap) = scene_gap(&scene, 20) {
        assert!(
            gap < 5e-4,
            "GPU cloth drifted {gap:.2e} m from the CPU reference"
        );
    }
}

#[test]
fn gpu_rope_matches_the_cpu_reference() {
    let scene = xpbd_scene(SceneKind::Rope, |_| {});
    if let Some(gap) = scene_gap(&scene, 60) {
        assert!(
            gap < 1e-3,
            "GPU rope drifted {gap:.2e} m from the CPU reference"
        );
    }
}

#[test]
fn gpu_contacts_match_the_cpu_reference_for_isolated_collisions() {
    // Pairs with zero net momentum, 2 m apart, so no grain meets a second one.
    let mut rng = Rng::new(17);
    let mut particles = Particles::default();
    for i in 0..100 {
        let centre = Vec3::new((i % 10) as f32, 0.0, (i / 10) as f32) * 2.0;
        let dir = rng.in_ball().normalize_or(Vec3::X);
        let v = -dir * rng.range(0.5, 2.0) + rng.in_cube(0.5);
        let inv_mass = rng.range(0.5, 2.0);
        particles.push(centre - dir * 0.05, -v, inv_mass);
        particles.push(centre + dir * 0.05, v, inv_mass);
    }
    let params = XpbdParams {
        gravity: Vec3::ZERO,
        radius: 0.02,
        friction: 0.4,
        self_collision: true,
        contact_iterations: 2,
        contact_relaxation: 1.5,
        ..XpbdParams::default()
    };
    if let Some((gpu_pos, cpu)) = run_both(&particles, &ColouredConstraints::default(), &params, 30)
    {
        let gap = max_distance(&gpu_pos, &cpu.particles.pos);
        assert!(
            gap < 1e-5,
            "GPU collisions drifted {gap:.2e} m from the CPU reference"
        );
    }
}

// Grains are chaotic, so the settled pile is compared by its statistics.
#[test]
fn gpu_pile_settles_like_the_cpu_reference() {
    let scene = xpbd_scene(SceneKind::Collisions, |s| {
        s.set("count", 1024.0);
    });
    let Some((gpu_pos, cpu)) = run_both(&scene.particles, &scene.constraints, &scene.params, 120)
    else {
        return;
    };
    let mean_height = |pos: &[Vec3]| pos.iter().map(|p| p.y).sum::<f32>() / pos.len() as f32;
    let (g, c) = (mean_height(&gpu_pos), mean_height(&cpu.particles.pos));
    assert!(
        (g - c).abs() < 0.05 * c,
        "mean height {g:.4} m on GPU vs {c:.4} m on CPU"
    );
    let r = scene.params.radius;
    let mut hash = SpatialHash::new(SpatialHash::table_size_for(gpu_pos.len()));
    hash.build(&gpu_pos, 2.0 * r);
    for i in 0..gpu_pos.len() {
        for j in hash.neighbours(&gpu_pos, i, 2.0 * r) {
            let overlap = 2.0 * r - gpu_pos[i].distance(gpu_pos[j as usize]);
            assert!(
                overlap < 0.1 * r,
                "GPU pile overlaps by {:.0}% of the radius",
                100.0 * overlap / r
            );
        }
    }
}

#[test]
fn gpu_spatial_hash_matches_the_cpu_hash() {
    let Some((device, queue)) = gpu() else { return };
    let mut rng = Rng::new(3);
    let mut particles = Particles::default();
    for _ in 0..3000 {
        particles.push(rng.in_cube(1.0), Vec3::ZERO, 1.0);
    }
    let params = XpbdParams {
        substeps: 1,
        gravity: Vec3::ZERO,
        radius: 0.02,
        bounds: Bounds::default(),
        self_collision: true,
        ..XpbdParams::default()
    };
    let kernels = XpbdKernels::new(&device);
    let sim = XpbdGpu::new(
        &device,
        &kernels,
        &particles,
        &ColouredConstraints::default(),
        &params,
    );
    let mut encoder = device.create_command_encoder(&Default::default());
    sim.encode(&mut encoder, 1);
    queue.submit([encoder.finish()]);
    let (cell_start, sorted) = sim.hash();
    let gpu_start: Vec<u32> =
        bytemuck::pod_collect_to_vec(&read_buffers(&device, &queue, &[cell_start]));
    let gpu_sorted: Vec<u32> =
        bytemuck::pod_collect_to_vec(&read_buffers(&device, &queue, &[sorted]));

    let mut cpu = SpatialHash::new(SpatialHash::table_size_for(particles.len()));
    cpu.build(&particles.pos, 2.0 * params.radius);
    assert_eq!(
        &gpu_start[..cpu.cell_start().len()],
        cpu.cell_start(),
        "bucket offsets differ"
    );
    assert_eq!(
        &gpu_sorted[..particles.len()],
        cpu.sorted(),
        "bucket contents differ"
    );
}

#[test]
fn gpu_nbody_matches_the_cpu_reference() {
    let Some((device, queue)) = gpu() else { return };
    let params = Settings::new(SceneKind::NBody).nbody_params();
    let particles = scene::cluster(1000, 0.6, 21);
    let kernels = NBodyKernels::new(&device);
    let sim = NBodyGpu::new(&device, &queue, &kernels, &particles, &params);
    let steps = 30;
    let mut encoder = device.create_command_encoder(&Default::default());
    sim.encode(&mut encoder, steps);
    queue.submit([encoder.finish()]);
    let bytes = read_buffers(&device, &queue, &[sim.positions(), sim.accelerations()]);
    let (gpu_pos, _) = unpack(&bytes[..16 * 1000]);
    let (_, gpu_potential) = unpack(&bytes[16 * 1000..]);

    let mut cpu = NBody::new(particles, &params);
    for _ in 0..steps {
        cpu.step(&params);
    }
    let gap = max_distance(&gpu_pos, &cpu.particles.pos);
    assert!(
        gap < 1e-3,
        "GPU bodies drifted {gap:.2e} from the CPU reference"
    );
    let worst = gpu_potential
        .iter()
        .zip(&cpu.potential)
        .map(|(g, c)| ((g - c) / c).abs())
        .fold(0.0, f32::max);
    assert!(worst < 1e-4, "GPU potentials differ by {worst:.2e}");
}
