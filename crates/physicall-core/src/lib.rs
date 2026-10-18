//! GPU-free physics for physicall: the XPBD and N-body reference solvers, the
//! spatial hash they share with the GPU kernels, diagnostics and demo scenes.

pub mod clock;
pub mod constraint;
pub mod contact;
pub mod diagnostics;
pub mod hash;
pub mod nbody;
pub mod particles;
pub mod rng;
pub mod scene;
pub mod xpbd;

pub use glam::Vec3;
