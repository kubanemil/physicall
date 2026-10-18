//! wgpu compute pipelines that run the `physicall-core` solvers on the GPU,
//! kernel for kernel, plus the shared Rust/WGSL layouts and async readback.

pub mod layout;
pub mod nbody;
pub mod pack;
pub mod readback;
pub mod shader;
mod util;
pub mod xpbd;

pub use wgpu;
