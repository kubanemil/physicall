use std::ops::Range;

use crate::particles::Particles;

/// Selects which compliance a constraint uses, so stiffness can change at
/// runtime without rewriting constraint data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Material {
    Stretch = 0,
    Shear = 1,
    Bend = 2,
    /// Long range attachment (Kim et al. 2012): a one-sided limit on the
    /// distance to a pinned particle, which stops hanging chains and cloth
    /// from stretching under their own weight.
    Tether = 3,
}

impl Material {
    pub fn is_unilateral(self) -> bool {
        self == Self::Tether
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DistanceConstraint {
    pub a: u32,
    pub b: u32,
    pub rest: f32,
    pub material: Material,
}

impl DistanceConstraint {
    /// A constraint whose rest length is the current distance between `a` and `b`.
    pub fn between(particles: &Particles, a: u32, b: u32, material: Material) -> Self {
        let rest = particles.pos[a as usize].distance(particles.pos[b as usize]);
        Self {
            a,
            b,
            rest,
            material,
        }
    }
}

/// Constraints sorted into colours: no two constraints of one colour share a
/// moving particle, so a colour can be solved in parallel with the same
/// result as a sequential Gauss-Seidel sweep. Static particles never move,
/// so they do not restrict colouring.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColouredConstraints {
    constraints: Vec<DistanceConstraint>,
    offsets: Vec<u32>,
}

impl ColouredConstraints {
    /// Greedy colouring in input order, then a stable counting sort by colour.
    /// Each particle's used colours fit in a 64-bit mask, which holds as long
    /// as no two linked particles have more than 64 constraints between them.
    pub fn new(inv_mass: &[f32], constraints: Vec<DistanceConstraint>) -> Self {
        let mut used = vec![0u64; inv_mass.len()];
        let mut colour_of = Vec::with_capacity(constraints.len());
        for c in &constraints {
            let moving = [c.a as usize, c.b as usize].map(|p| (inv_mass[p] > 0.0).then_some(p));
            let taken = moving.iter().flatten().fold(0, |mask, &p| mask | used[p]);
            assert!(
                taken != u64::MAX,
                "constraint graph needs more than 64 colours"
            );
            let colour = (!taken).trailing_zeros();
            for &p in moving.iter().flatten() {
                used[p] |= 1 << colour;
            }
            colour_of.push(colour);
        }

        let colour_count = colour_of.iter().max().map_or(0, |&c| c as usize + 1);
        let mut offsets = vec![0u32; colour_count + 1];
        for &colour in &colour_of {
            offsets[colour as usize + 1] += 1;
        }
        for i in 1..offsets.len() {
            offsets[i] += offsets[i - 1];
        }

        let mut cursor = offsets.clone();
        let mut sorted = constraints.clone();
        for (c, &colour) in constraints.into_iter().zip(&colour_of) {
            sorted[cursor[colour as usize] as usize] = c;
            cursor[colour as usize] += 1;
        }

        Self {
            constraints: sorted,
            offsets,
        }
    }

    pub fn len(&self) -> usize {
        self.constraints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    pub fn as_slice(&self) -> &[DistanceConstraint] {
        &self.constraints
    }

    pub fn colour_count(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    pub fn colour_ranges(&self) -> impl Iterator<Item = Range<u32>> + '_ {
        self.offsets.windows(2).map(|w| w[0]..w[1])
    }

    pub fn colours(&self) -> impl Iterator<Item = &[DistanceConstraint]> + '_ {
        self.colour_ranges()
            .map(|r| &self.constraints[r.start as usize..r.end as usize])
    }
}

#[cfg(test)]
mod tests {
    use crate::scene;

    #[test]
    fn no_colour_contains_two_constraints_on_the_same_particle() {
        let cloth = scene::cloth(12, 1.0, 0.5);
        assert!(cloth.constraints.colour_count() > 1);
        for colour in cloth.constraints.colours() {
            let mut seen = vec![false; cloth.particles.len()];
            for c in colour {
                for p in [c.a as usize, c.b as usize] {
                    if cloth.particles.inv_mass[p] == 0.0 {
                        continue;
                    }
                    assert!(!seen[p], "particle {p} appears twice in one colour");
                    seen[p] = true;
                }
            }
        }
    }
}
