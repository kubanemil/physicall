use glam::{IVec3, Vec3};

/// Prime multipliers from Teschner et al. 2003, "Optimized Spatial Hashing for
/// Collision Detection of Deformable Objects".
pub const PRIMES: [u32; 3] = [73_856_093, 19_349_663, 83_492_791];

pub fn cell_of(p: Vec3, cell_size: f32) -> IVec3 {
    (p / cell_size).floor().as_ivec3()
}

pub fn bucket(cell: IVec3, table_size: u32) -> u32 {
    let h = (cell.x as u32).wrapping_mul(PRIMES[0])
        ^ (cell.y as u32).wrapping_mul(PRIMES[1])
        ^ (cell.z as u32).wrapping_mul(PRIMES[2]);
    h % table_size
}

/// Uniform grid hashed into a fixed table and built by counting sort:
/// count particles per bucket, exclusive prefix sum into `cell_start`, then
/// scatter particle indices into `sorted`. The GPU kernels run the same three
/// passes with atomics.
#[derive(Clone, Debug)]
pub struct SpatialHash {
    table_size: u32,
    cell_size: f32,
    cell_start: Vec<u32>,
    entries: Vec<[u32; 2]>,
    sorted: Vec<u32>,
}

impl SpatialHash {
    pub fn new(table_size: u32) -> Self {
        assert!(table_size > 0, "hash table needs at least one bucket");
        Self {
            table_size,
            cell_size: 1.0,
            cell_start: vec![0; table_size as usize + 1],
            entries: Vec::new(),
            sorted: Vec::new(),
        }
    }

    /// Keeps the load factor at or below one half.
    pub fn table_size_for(particle_count: usize) -> u32 {
        (2 * particle_count).next_power_of_two().max(64) as u32
    }

    pub fn table_size(&self) -> u32 {
        self.table_size
    }

    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    pub fn cell_start(&self) -> &[u32] {
        &self.cell_start
    }

    pub fn sorted(&self) -> &[u32] {
        &self.sorted
    }

    pub fn build(&mut self, pos: &[Vec3], cell_size: f32) {
        self.cell_size = cell_size;
        self.cell_start.fill(0);
        self.entries.clear();
        for &p in pos {
            let b = bucket(cell_of(p, cell_size), self.table_size) as usize;
            self.entries.push([b as u32, self.cell_start[b]]);
            self.cell_start[b] += 1;
        }

        let mut running = 0;
        for start in &mut self.cell_start {
            let count = *start;
            *start = running;
            running += count;
        }

        self.sorted.clear();
        self.sorted.resize(pos.len(), 0);
        for (i, &[b, rank]) in self.entries.iter().enumerate() {
            self.sorted[(self.cell_start[b as usize] + rank) as usize] = i as u32;
        }
    }

    /// Calls `f` for every particle in the 27 cells around `p`. Buckets shared
    /// by two of those cells are visited once, so no candidate repeats.
    pub fn for_each_candidate(&self, p: Vec3, mut f: impl FnMut(u32)) {
        let base = cell_of(p, self.cell_size);
        let mut visited = [0u32; 27];
        let mut visited_count = 0;
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let b = bucket(base + IVec3::new(dx, dy, dz), self.table_size);
                    if visited[..visited_count].contains(&b) {
                        continue;
                    }
                    visited[visited_count] = b;
                    visited_count += 1;
                    let range = self.cell_start[b as usize]..self.cell_start[b as usize + 1];
                    for &j in &self.sorted[range.start as usize..range.end as usize] {
                        f(j);
                    }
                }
            }
        }
    }

    /// Indices within `radius` of particle `i`, excluding `i`, via the grid.
    /// `radius` must not exceed the cell size.
    pub fn neighbours(&self, pos: &[Vec3], i: usize, radius: f32) -> Vec<u32> {
        let mut out = Vec::new();
        self.for_each_candidate(pos[i], |j| {
            if j as usize != i && pos[i].distance_squared(pos[j as usize]) < radius * radius {
                out.push(j);
            }
        });
        out
    }
}
