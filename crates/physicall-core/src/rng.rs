use glam::Vec3;

/// SplitMix64: small, fast and deterministic across platforms, which is all
/// the scenes and tests need.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    pub fn in_cube(&mut self, half: f32) -> Vec3 {
        Vec3::new(
            self.range(-half, half),
            self.range(-half, half),
            self.range(-half, half),
        )
    }

    /// Uniform in the unit ball, by rejection.
    pub fn in_ball(&mut self) -> Vec3 {
        loop {
            let p = self.in_cube(1.0);
            if p.length_squared() <= 1.0 {
                return p;
            }
        }
    }
}
