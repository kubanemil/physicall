use physicall_core::Vec3;
use physicall_core::hash::SpatialHash;
use physicall_core::rng::Rng;

fn brute_force(pos: &[Vec3], i: usize, radius: f32) -> Vec<u32> {
    (0..pos.len())
        .filter(|&j| j != i && pos[i].distance_squared(pos[j]) < radius * radius)
        .map(|j| j as u32)
        .collect()
}

fn check(pos: &[Vec3], radius: f32, table_size: u32) {
    let mut hash = SpatialHash::new(table_size);
    hash.build(pos, radius);
    for i in 0..pos.len() {
        let mut found = hash.neighbours(pos, i, radius);
        found.sort_unstable();
        assert_eq!(
            found,
            brute_force(pos, i, radius),
            "particle {i}, table {table_size}"
        );
    }
}

#[test]
fn hash_neighbours_match_brute_force_on_random_sets() {
    for seed in 0..5 {
        let mut rng = Rng::new(seed);
        let pos: Vec<Vec3> = (0..1500).map(|_| rng.in_cube(1.0)).collect();
        check(&pos, 0.08, SpatialHash::table_size_for(pos.len()));
    }
}

#[test]
fn hash_neighbours_survive_heavy_bucket_collisions() {
    let mut rng = Rng::new(42);
    let pos: Vec<Vec3> = (0..800)
        .map(|_| rng.in_cube(2.0) + Vec3::splat(-5.0))
        .collect();
    for table_size in [1, 7, 64] {
        check(&pos, 0.15, table_size);
    }
}

#[test]
fn hash_buckets_partition_all_particles() {
    let mut rng = Rng::new(7);
    let pos: Vec<Vec3> = (0..1000).map(|_| rng.in_cube(1.0)).collect();
    let mut hash = SpatialHash::new(512);
    hash.build(&pos, 0.1);
    let starts = hash.cell_start();
    assert_eq!(starts[0], 0);
    assert_eq!(*starts.last().unwrap_or(&0) as usize, pos.len());
    assert!(starts.windows(2).all(|w| w[0] <= w[1]));
    let mut sorted = hash.sorted().to_vec();
    sorted.sort_unstable();
    assert_eq!(sorted, (0..pos.len() as u32).collect::<Vec<_>>());
}
