use crate::layout::{CONSTS_WGSL, STRUCTS_WGSL};

#[doc(hidden)]
pub const fn total_len(parts: &[&str]) -> usize {
    let mut len = 0;
    let mut i = 0;
    while i < parts.len() {
        len += parts[i].len();
        i += 1;
    }
    len
}

#[doc(hidden)]
pub const fn join<const N: usize>(parts: &[&str]) -> [u8; N] {
    let mut out = [0u8; N];
    let mut at = 0;
    let mut i = 0;
    while i < parts.len() {
        let bytes = parts[i].as_bytes();
        let mut j = 0;
        while j < bytes.len() {
            out[at] = bytes[j];
            at += 1;
            j += 1;
        }
        i += 1;
    }
    out
}

/// Concatenates `&str` constants at compile time (`concat!` only takes literals).
#[macro_export]
macro_rules! concat_str {
    ($($part:expr),+ $(,)?) => {{
        const PARTS: &[&str] = &[$($part),+];
        const LEN: usize = $crate::shader::total_len(PARTS);
        const BYTES: [u8; LEN] = $crate::shader::join::<LEN>(PARTS);
        match core::str::from_utf8(&BYTES) {
            Ok(s) => s,
            Err(_) => panic!("shader source is not UTF-8"),
        }
    }};
}

pub const PRELUDE: &str = concat_str!(CONSTS_WGSL, STRUCTS_WGSL);

const XPBD_COMMON: &str = include_str!("../shaders/xpbd_common.wgsl");

pub const INTEGRATE: &str = concat_str!(
    PRELUDE,
    XPBD_COMMON,
    include_str!("../shaders/integrate.wgsl")
);
pub const SOLVE: &str = concat_str!(PRELUDE, XPBD_COMMON, include_str!("../shaders/solve.wgsl"));
pub const GRID: &str = concat_str!(PRELUDE, XPBD_COMMON, include_str!("../shaders/grid.wgsl"));
pub const NBODY: &str = concat_str!(PRELUDE, include_str!("../shaders/nbody.wgsl"));

pub fn module(device: &wgpu::Device, label: &str, source: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_compute_shader_validates() {
        for (name, source) in [
            ("integrate", INTEGRATE),
            ("solve", SOLVE),
            ("grid", GRID),
            ("nbody", NBODY),
        ] {
            let module = naga::front::wgsl::parse_str(source)
                .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(source)));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::empty(),
            )
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(source)));
        }
    }
}
