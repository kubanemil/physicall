use glam::Vec3;

/// Particle state in structure-of-arrays form. `inv_mass == 0` marks a static
/// (pinned or user-held) particle.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Particles {
    pub pos: Vec<Vec3>,
    pub prev: Vec<Vec3>,
    pub vel: Vec<Vec3>,
    pub inv_mass: Vec<f32>,
}

impl Particles {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            pos: Vec::with_capacity(capacity),
            prev: Vec::with_capacity(capacity),
            vel: Vec::with_capacity(capacity),
            inv_mass: Vec::with_capacity(capacity),
        }
    }

    pub fn len(&self) -> usize {
        self.pos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pos.is_empty()
    }

    pub fn push(&mut self, pos: Vec3, vel: Vec3, inv_mass: f32) -> u32 {
        let index = self.pos.len() as u32;
        self.pos.push(pos);
        self.prev.push(pos);
        self.vel.push(vel);
        self.inv_mass.push(inv_mass);
        index
    }

    pub fn translate(&mut self, offset: Vec3) {
        for (x, prev) in self.pos.iter_mut().zip(&mut self.prev) {
            *x += offset;
            *prev += offset;
        }
    }

    pub fn mass(&self, i: usize) -> Option<f32> {
        let w = self.inv_mass[i];
        (w > 0.0).then(|| 1.0 / w)
    }
}
