//! Structs shared by Rust and WGSL. Each is declared once; the macro emits
//! the `#[repr(C)]` Rust type, its WGSL source for the shader prelude, and
//! compile-time asserts that Rust offsets and size follow WGSL layout rules.

#[doc(hidden)]
#[macro_export]
macro_rules! wgsl_type {
    (f32) => {
        "f32"
    };
    (u32) => {
        "u32"
    };
    (i32) => {
        "i32"
    };
    ([f32; 4]) => {
        "vec4<f32>"
    };
    ([u32; 4]) => {
        "vec4<u32>"
    };
    ([[f32; 4]; 4]) => {
        "mat4x4<f32>"
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! wgsl_align {
    (f32) => {
        4
    };
    (u32) => {
        4
    };
    (i32) => {
        4
    };
    ([f32; 4]) => {
        16
    };
    ([u32; 4]) => {
        16
    };
    ([[f32; 4]; 4]) => {
        16
    };
}

/// Declares structs shared with WGSL and a `&str` constant holding their WGSL
/// definitions. Only scalars, `vec4` and `mat4x4` are allowed, so uniform and
/// storage layouts coincide; every struct must be a multiple of 16 bytes.
#[macro_export]
macro_rules! gpu_structs {
    (
        const $prelude:ident;
        $(
            $(#[$meta:meta])*
            pub struct $name:ident { $(pub $field:ident: $ty:tt,)* }
        )*
    ) => {
        $(
            $(#[$meta])*
            #[repr(C)]
            #[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
            pub struct $name { $(pub $field: $ty,)* }

            const _: () = {
                let mut offset = 0usize;
                $(
                    offset = offset.next_multiple_of($crate::wgsl_align!($ty));
                    assert!(
                        offset == core::mem::offset_of!($name, $field),
                        concat!(stringify!($name), ".", stringify!($field), ": Rust and WGSL offsets differ"),
                    );
                    offset += core::mem::size_of::<$ty>();
                )*
                assert!(
                    offset.next_multiple_of(16) == core::mem::size_of::<$name>(),
                    concat!(stringify!($name), ": size must match WGSL and be a multiple of 16"),
                );
            };
        )*

        pub const $prelude: &str = concat!($(
            "struct ", stringify!($name), " {\n",
            $("    ", stringify!($field), ": ", $crate::wgsl_type!($ty), ",\n",)*
            "}\n\n",
        )*);
    };
}

/// Declares integer constants visible to both Rust and WGSL.
macro_rules! shared_consts {
    (const $prelude:ident; $($name:ident: u32 = $value:literal;)*) => {
        $(pub const $name: u32 = $value;)*
        pub const $prelude: &str = concat!($("const ", stringify!($name), ": u32 = ", stringify!($value), "u;\n",)* "\n");
    };
}

shared_consts! {
    const CONSTS_WGSL;
    WORKGROUP_SIZE: u32 = 64;
    NBODY_TILE: u32 = 128;
    SCAN_THREADS: u32 = 256;
    HASH_PRIME_X: u32 = 73856093;
    HASH_PRIME_Y: u32 = 19349663;
    HASH_PRIME_Z: u32 = 83492791;
    FLAG_FLOOR: u32 = 1;
    FLAG_WALLS: u32 = 2;
    MATERIAL_TETHER: u32 = 3;
}

const _: () = {
    let p = physicall_core::hash::PRIMES;
    assert!(p[0] == HASH_PRIME_X && p[1] == HASH_PRIME_Y && p[2] == HASH_PRIME_Z);
    assert!(physicall_core::constraint::Material::Tether as u32 == MATERIAL_TETHER);
};

gpu_structs! {
    const STRUCTS_WGSL;

    pub struct XpbdUniforms {
        pub gravity: [f32; 4],
        pub alpha: [f32; 4],
        pub h: f32,
        pub inv_h: f32,
        pub damping: f32,
        pub radius: f32,
        pub friction: f32,
        pub relaxation: f32,
        pub floor: f32,
        pub half_width: f32,
        pub count: u32,
        pub table_size: u32,
        pub flags: u32,
        pub pad: u32,
    }

    pub struct ColourRange {
        pub first: u32,
        pub count: u32,
        pub pad0: u32,
        pub pad1: u32,
    }

    pub struct GpuConstraint {
        pub a: u32,
        pub b: u32,
        pub rest: f32,
        pub material: u32,
    }

    pub struct NBodyUniforms {
        pub h: f32,
        pub g: f32,
        pub eps2: f32,
        pub count: u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses the prelude with naga, the compiler wgpu uses, and checks its
    /// computed layout against Rust's.
    fn naga_layout(name: &str) -> (u32, Vec<(String, u32)>) {
        let source = format!("{CONSTS_WGSL}{STRUCTS_WGSL}");
        let module = naga::front::wgsl::parse_str(&source)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
        module
            .types
            .iter()
            .find_map(|(_, ty)| match (&ty.name, &ty.inner) {
                (Some(n), naga::TypeInner::Struct { members, span }) if n == name => Some((
                    *span,
                    members
                        .iter()
                        .map(|m| (m.name.clone().unwrap_or_default(), m.offset))
                        .collect(),
                )),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{name} missing from the prelude"))
    }

    macro_rules! assert_layout {
        ($ty:ident { $($field:ident),* }) => {{
            let (size, members) = naga_layout(stringify!($ty));
            assert_eq!(size as usize, core::mem::size_of::<$ty>(), "{} size", stringify!($ty));
            let rust: Vec<(String, u32)> = vec![$((stringify!($field).to_string(), core::mem::offset_of!($ty, $field) as u32)),*];
            assert_eq!(members, rust, "{} offsets", stringify!($ty));
        }};
    }

    #[test]
    fn rust_and_wgsl_layouts_match() {
        assert_layout!(XpbdUniforms {
            gravity,
            alpha,
            h,
            inv_h,
            damping,
            radius,
            friction,
            relaxation,
            floor,
            half_width,
            count,
            table_size,
            flags,
            pad
        });
        assert_layout!(ColourRange {
            first,
            count,
            pad0,
            pad1
        });
        assert_layout!(GpuConstraint {
            a,
            b,
            rest,
            material
        });
        assert_layout!(NBodyUniforms { h, g, eps2, count });
    }
}
