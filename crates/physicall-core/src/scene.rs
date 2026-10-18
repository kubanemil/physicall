//! Demo scenes and their tunable parameters.

use glam::Vec3;

use crate::constraint::{ColouredConstraints, DistanceConstraint, Material};
use crate::contact::Bounds;
use crate::nbody::NBodyParams;
use crate::particles::Particles;
use crate::rng::Rng;
use crate::xpbd::XpbdParams;

pub const STEP: f32 = 1.0 / 60.0;
const SEED: u64 = 0x5EED_CAFE;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SceneKind {
    NBody,
    Cloth,
    Rope,
    Collisions,
}

impl SceneKind {
    pub const ALL: [Self; 4] = [Self::NBody, Self::Cloth, Self::Rope, Self::Collisions];

    pub fn id(self) -> &'static str {
        match self {
            Self::NBody => "nbody",
            Self::Cloth => "cloth",
            Self::Rope => "rope",
            Self::Collisions => "collisions",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::NBody => "N-body",
            Self::Cloth => "Cloth",
            Self::Rope => "Rope",
            Self::Collisions => "Collisions",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }

    pub fn params(self) -> &'static [ParamSpec] {
        match self {
            Self::NBody => NBODY,
            Self::Cloth => CLOTH,
            Self::Rope => ROPE,
            Self::Collisions => COLLISIONS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scale {
    Linear { step: f32 },
    Log,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub unit: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub scale: Scale,
    /// Changing it rebuilds the scene instead of updating it live.
    pub rebuild: bool,
}

const fn linear(
    key: &'static str,
    label: &'static str,
    unit: &'static str,
    range: [f32; 3],
    step: f32,
    rebuild: bool,
) -> ParamSpec {
    let [min, max, default] = range;
    ParamSpec {
        key,
        label,
        unit,
        min,
        max,
        default,
        scale: Scale::Linear { step },
        rebuild,
    }
}

const fn log(
    key: &'static str,
    label: &'static str,
    unit: &'static str,
    range: [f32; 3],
) -> ParamSpec {
    let [min, max, default] = range;
    ParamSpec {
        key,
        label,
        unit,
        min,
        max,
        default,
        scale: Scale::Log,
        rebuild: false,
    }
}

const GRAVITY: ParamSpec = linear("gravity", "Gravity", "m/s²", [0.0, 20.0, 9.81], 0.01, false);

const NBODY: &[ParamSpec] = &[
    linear(
        "bodies",
        "Bodies",
        "",
        [256.0, 16384.0, 4096.0],
        256.0,
        true,
    ),
    linear("spin", "Spin", "", [0.0, 1.2, 0.6], 0.05, true),
    linear("g", "G", "", [0.0, 2.0, 1.0], 0.05, false),
    log("softening", "Softening", "", [0.002, 0.2, 0.03]),
    linear("substeps", "Substeps", "", [1.0, 8.0, 2.0], 1.0, false),
];

const CLOTH: &[ParamSpec] = &[
    linear("resolution", "Resolution", "", [8.0, 96.0, 40.0], 1.0, true),
    log("stretch", "Stretch compliance", "m/N", [1e-9, 1e-2, 1e-7]),
    log("shear", "Shear compliance", "m/N", [1e-9, 1e-1, 1e-5]),
    log("bend", "Bend compliance", "m/N", [1e-6, 10.0, 0.1]),
    linear("drag", "Air drag", "1/s", [0.0, 2.0, 0.2], 0.05, false),
    GRAVITY,
    linear("substeps", "Substeps", "", [1.0, 40.0, 15.0], 1.0, false),
];

const ROPE: &[ParamSpec] = &[
    linear("segments", "Segments", "", [4.0, 256.0, 48.0], 1.0, true),
    log("stretch", "Stretch compliance", "m/N", [1e-9, 1e-2, 1e-8]),
    linear("drag", "Air drag", "1/s", [0.0, 2.0, 0.05], 0.05, false),
    GRAVITY,
    linear("substeps", "Substeps", "", [1.0, 60.0, 20.0], 1.0, false),
];

const COLLISIONS: &[ParamSpec] = &[
    linear(
        "count",
        "Particles",
        "",
        [512.0, 32768.0, 8192.0],
        512.0,
        true,
    ),
    linear("friction", "Friction", "", [0.0, 1.0, 0.3], 0.05, false),
    GRAVITY,
    linear("substeps", "Substeps", "", [1.0, 40.0, 20.0], 1.0, false),
];

const CLOTH_SIZE: f32 = 1.0;
const CLOTH_MASS: f32 = 0.5;
const CLOTH_HEIGHT: f32 = 1.6;
const ROPE_LENGTH: f32 = 2.0;
const ROPE_MASS: f32 = 0.2;
const ROPE_HEIGHT: f32 = 2.2;
const PILE_RADIUS: f32 = 0.02;
const PILE_HALF_WIDTH: f32 = 0.6;
const PILE_PARTICLE_MASS: f32 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Live,
    Rebuild,
}

/// Current parameter values for one scene kind.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    kind: SceneKind,
    values: Vec<f32>,
}

impl Settings {
    pub fn new(kind: SceneKind) -> Self {
        Self {
            kind,
            values: kind.params().iter().map(|p| p.default).collect(),
        }
    }

    pub fn kind(&self) -> SceneKind {
        self.kind
    }

    pub fn get(&self, key: &str) -> Option<f32> {
        let i = self.kind.params().iter().position(|p| p.key == key)?;
        Some(self.values[i])
    }

    /// Clamps `value` into the parameter's range. Returns `None` for an
    /// unknown key or a non-finite value.
    pub fn set(&mut self, key: &str, value: f32) -> Option<Change> {
        let i = self.kind.params().iter().position(|p| p.key == key)?;
        let spec = &self.kind.params()[i];
        if !value.is_finite() {
            return None;
        }
        self.values[i] = value.clamp(spec.min, spec.max);
        Some(if spec.rebuild {
            Change::Rebuild
        } else {
            Change::Live
        })
    }

    fn value(&self, key: &str) -> f32 {
        self.get(key)
            .unwrap_or_else(|| panic!("{} scene has no parameter {key}", self.kind.id()))
    }

    fn count(&self, key: &str) -> u32 {
        self.value(key).round() as u32
    }

    fn gravity(&self) -> Vec3 {
        Vec3::new(0.0, -self.value("gravity"), 0.0)
    }

    pub fn nbody_params(&self) -> NBodyParams {
        NBodyParams {
            dt: STEP,
            substeps: self.count("substeps"),
            g: self.value("g"),
            softening: self.value("softening"),
        }
    }

    pub fn xpbd_params(&self) -> XpbdParams {
        let floor = Bounds {
            floor: Some(0.0),
            half_width: None,
        };
        match self.kind {
            SceneKind::NBody => XpbdParams::default(),
            SceneKind::Cloth => {
                let spacing = CLOTH_SIZE / (self.count("resolution") - 1) as f32;
                XpbdParams {
                    substeps: self.count("substeps"),
                    gravity: self.gravity(),
                    drag: self.value("drag"),
                    compliance: [
                        self.value("stretch"),
                        self.value("shear"),
                        self.value("bend"),
                        0.0,
                    ],
                    radius: 0.25 * spacing,
                    bounds: floor,
                    friction: 0.5,
                    ..XpbdParams::default()
                }
            }
            SceneKind::Rope => {
                let stretch = self.value("stretch");
                XpbdParams {
                    substeps: self.count("substeps"),
                    gravity: self.gravity(),
                    drag: self.value("drag"),
                    compliance: [stretch, stretch, stretch, 0.0],
                    radius: 0.5 * ROPE_LENGTH / self.count("segments") as f32,
                    bounds: floor,
                    friction: 0.5,
                    ..XpbdParams::default()
                }
            }
            SceneKind::Collisions => XpbdParams {
                substeps: self.count("substeps"),
                gravity: self.gravity(),
                radius: PILE_RADIUS,
                bounds: Bounds {
                    floor: Some(0.0),
                    half_width: Some(PILE_HALF_WIDTH),
                },
                friction: self.value("friction"),
                self_collision: true,
                ..XpbdParams::default()
            },
        }
    }

    pub fn build(&self) -> Scene {
        let params = self.xpbd_params();
        match self.kind {
            SceneKind::NBody => Scene::NBody(NBodyScene {
                particles: cluster(self.count("bodies"), self.value("spin"), SEED),
                params: self.nbody_params(),
            }),
            SceneKind::Cloth => {
                let mut cloth = cloth(self.count("resolution"), CLOTH_SIZE, CLOTH_MASS);
                cloth.particles.translate(Vec3::Y * CLOTH_HEIGHT);
                Scene::Xpbd(XpbdScene {
                    particles: cloth.particles,
                    constraints: cloth.constraints,
                    triangles: cloth.triangles,
                    look: Look::Cloth,
                    params,
                })
            }
            SceneKind::Rope => {
                let segments = self.count("segments");
                let start = Vec3::new(-0.5 * ROPE_LENGTH, ROPE_HEIGHT, 0.0);
                let points = (0..=segments)
                    .map(|i| start + Vec3::X * (ROPE_LENGTH * i as f32 / segments as f32));
                let (mut particles, mut constraints) = chain(points, ROPE_MASS / segments as f32);
                particles.inv_mass[0] = 0.0;
                constraints.extend(tethers(&particles));
                Scene::Xpbd(XpbdScene {
                    constraints: ColouredConstraints::new(&particles.inv_mass, constraints),
                    particles,
                    triangles: Vec::new(),
                    look: Look::Chain,
                    params,
                })
            }
            SceneKind::Collisions => {
                let particles = pile(self.count("count"), PILE_RADIUS, PILE_HALF_WIDTH, SEED);
                Scene::Xpbd(XpbdScene {
                    constraints: ColouredConstraints::default(),
                    particles,
                    triangles: Vec::new(),
                    look: Look::Spheres,
                    params,
                })
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Spheres,
    Chain,
    Cloth,
}

#[derive(Clone, Debug)]
pub enum Scene {
    Xpbd(XpbdScene),
    NBody(NBodyScene),
}

#[derive(Clone, Debug)]
pub struct XpbdScene {
    pub particles: Particles,
    pub constraints: ColouredConstraints,
    pub triangles: Vec<[u32; 3]>,
    pub look: Look,
    pub params: XpbdParams,
}

#[derive(Clone, Debug)]
pub struct NBodyScene {
    pub particles: Particles,
    pub params: NBodyParams,
}

pub struct Cloth {
    pub particles: Particles,
    pub constraints: ColouredConstraints,
    pub triangles: Vec<[u32; 3]>,
}

/// Square cloth in the xz-plane centred on the origin, pinned at both
/// corners of the -z edge, with stretch, shear and bend links.
pub fn cloth(resolution: u32, size: f32, mass: f32) -> Cloth {
    let n = resolution.max(2);
    let spacing = size / (n - 1) as f32;
    let inv_mass = (n * n) as f32 / mass;
    let mut particles = Particles::with_capacity((n * n) as usize);
    for row in 0..n {
        for col in 0..n {
            let pos = Vec3::new(
                col as f32 * spacing - 0.5 * size,
                0.0,
                row as f32 * spacing - 0.5 * size,
            );
            let pinned = row == 0 && (col == 0 || col == n - 1);
            particles.push(pos, Vec3::ZERO, if pinned { 0.0 } else { inv_mass });
        }
    }

    let id = |row: u32, col: u32| row * n + col;
    // Link kinds in the outer loop give each kind its own pair of colours.
    let links = [
        (0, 1, Material::Stretch),
        (1, 0, Material::Stretch),
        (1, 1, Material::Shear),
        (1, -1, Material::Shear),
        (0, 2, Material::Bend),
        (2, 0, Material::Bend),
    ];
    let mut constraints = Vec::new();
    for (dr, dc, material) in links {
        for row in 0..n {
            for col in 0..n {
                let (r, c) = (row as i64 + dr, col as i64 + dc);
                if (0..n as i64).contains(&r) && (0..n as i64).contains(&c) {
                    constraints.push(DistanceConstraint::between(
                        &particles,
                        id(row, col),
                        id(r as u32, c as u32),
                        material,
                    ));
                }
            }
        }
    }

    constraints.extend(tethers(&particles));

    let mut triangles = Vec::with_capacity(2 * ((n - 1) * (n - 1)) as usize);
    for row in 0..n - 1 {
        for col in 0..n - 1 {
            triangles.push([id(row, col), id(row + 1, col), id(row, col + 1)]);
            triangles.push([id(row + 1, col), id(row + 1, col + 1), id(row, col + 1)]);
        }
    }

    Cloth {
        constraints: ColouredConstraints::new(&particles.inv_mass, constraints),
        particles,
        triangles,
    }
}

/// One tether from every moving particle to every pinned one, with the
/// current (rest-shape) distance as its limit. Exact for straight chains and
/// flat cloth, where rest geodesics are straight lines.
pub fn tethers(particles: &Particles) -> Vec<DistanceConstraint> {
    let pins: Vec<u32> = (0..particles.len() as u32)
        .filter(|&i| particles.inv_mass[i as usize] == 0.0)
        .collect();
    pins.iter()
        .flat_map(|&pin| {
            (0..particles.len() as u32)
                .filter(|&i| particles.inv_mass[i as usize] > 0.0)
                .map(move |i| DistanceConstraint::between(particles, i, pin, Material::Tether))
        })
        .collect()
}

/// Free particles at `points`, each consecutive pair joined by a stretch link.
pub fn chain(
    points: impl IntoIterator<Item = Vec3>,
    particle_mass: f32,
) -> (Particles, Vec<DistanceConstraint>) {
    let mut particles = Particles::default();
    for p in points {
        particles.push(p, Vec3::ZERO, 1.0 / particle_mass);
    }
    let constraints = (1..particles.len() as u32)
        .map(|i| DistanceConstraint::between(&particles, i - 1, i, Material::Stretch))
        .collect();
    (particles, constraints)
}

/// `count` particles on a jittered lattice above the floor, inside walls at
/// `±half_width`.
pub fn pile(count: u32, radius: f32, half_width: f32, seed: u64) -> Particles {
    let mut rng = Rng::new(seed);
    let spacing = 2.2 * radius;
    let fits = ((2.0 * (half_width - radius)) / spacing).floor().max(1.0) as u32;
    let side = ((count as f32 / 24.0).sqrt().ceil() as u32).clamp(1, fits);
    let centre = 0.5 * (side - 1) as f32;
    let mut particles = Particles::with_capacity(count as usize);
    for i in 0..count {
        let (x, z, y) = (i % side, (i / side) % side, i / (side * side));
        let lattice = Vec3::new(x as f32 - centre, y as f32, z as f32 - centre) * spacing;
        let jitter = rng.in_cube(0.05 * radius);
        let pos = lattice + jitter + Vec3::Y * (0.3 + radius);
        particles.push(pos, Vec3::ZERO, 1.0 / PILE_PARTICLE_MASS);
    }
    particles
}

/// Uniform ball of radius 1 and total mass 1 in solid-body rotation about +y
/// at `spin` times the circular speed for G = 1, with zero net momentum.
pub fn cluster(count: u32, spin: f32, seed: u64) -> Particles {
    let mut rng = Rng::new(seed);
    let n = count.max(2);
    let mut particles = Particles::with_capacity(n as usize);
    for _ in 0..n {
        let p = rng.in_ball();
        let v = Vec3::new(p.z, 0.0, -p.x) * spin;
        particles.push(p, v, n as f32);
    }
    let mean_pos = particles.pos.iter().sum::<Vec3>() / n as f32;
    let mean_vel = particles.vel.iter().sum::<Vec3>() / n as f32;
    particles.translate(-mean_pos);
    for v in &mut particles.vel {
        *v -= mean_vel;
    }
    particles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scene_builds_with_default_settings() {
        for kind in SceneKind::ALL {
            let scene = Settings::new(kind).build();
            let particles = match &scene {
                Scene::Xpbd(s) => &s.particles,
                Scene::NBody(s) => &s.particles,
            };
            assert!(!particles.is_empty(), "{} is empty", kind.id());
        }
    }

    #[test]
    fn settings_clamp_and_report_rebuilds() {
        let mut settings = Settings::new(SceneKind::Cloth);
        assert_eq!(settings.set("resolution", 1000.0), Some(Change::Rebuild));
        assert_eq!(settings.get("resolution"), Some(96.0));
        assert_eq!(settings.set("drag", 0.5), Some(Change::Live));
        assert_eq!(settings.set("nope", 1.0), None);
        assert_eq!(settings.set("drag", f32::NAN), None);
    }

    #[test]
    fn cloth_has_the_expected_links() {
        let n = 9;
        let cloth = cloth(n, 1.0, 0.5);
        let stretch = 2 * n * (n - 1);
        let shear = 2 * (n - 1) * (n - 1);
        let bend = 2 * n * (n - 2);
        let tethers = 2 * (n * n - 2);
        assert_eq!(
            cloth.constraints.len() as u32,
            stretch + shear + bend + tethers
        );
        assert_eq!(cloth.triangles.len() as u32, 2 * (n - 1) * (n - 1));
    }

    #[test]
    fn pile_starts_without_overlaps() {
        let p = pile(2000, PILE_RADIUS, PILE_HALF_WIDTH, 1);
        for i in 0..p.len() {
            for j in i + 1..p.len() {
                assert!(p.pos[i].distance(p.pos[j]) > 2.0 * PILE_RADIUS);
            }
        }
    }

    #[test]
    fn cluster_has_zero_net_momentum() {
        let p = cluster(1000, 0.7, 3);
        let momentum: Vec3 = p.vel.iter().sum();
        assert!(momentum.length() < 1e-3);
    }
}
