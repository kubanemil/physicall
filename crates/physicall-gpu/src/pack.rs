use glam::Vec3;

/// `xyz` and `w` interleaved as WGSL `vec4<f32>`.
pub fn pack(xyz: &[Vec3], w: &[f32]) -> Vec<[f32; 4]> {
    xyz.iter()
        .zip(w)
        .map(|(v, &w)| v.extend(w).to_array())
        .collect()
}

pub fn pack_zero_w(xyz: &[Vec3]) -> Vec<[f32; 4]> {
    xyz.iter().map(|v| v.extend(0.0).to_array()).collect()
}

/// Splits mapped `vec4<f32>` bytes into `xyz` and `w`.
pub fn unpack(bytes: &[u8]) -> (Vec<Vec3>, Vec<f32>) {
    let values: Vec<[f32; 4]> = bytemuck::pod_collect_to_vec(bytes);
    values
        .iter()
        .map(|&[x, y, z, w]| (Vec3::new(x, y, z), w))
        .unzip()
}
