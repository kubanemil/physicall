use glam::{Mat4, Vec2, Vec3};

/// Index of the particle drawn closest to `pixel`, if any lies within
/// `max_pixels` of it. Nearer particles win ties.
pub fn pick(
    positions: &[Vec3],
    view_proj: Mat4,
    pixel: Vec2,
    size: Vec2,
    max_pixels: f32,
) -> Option<usize> {
    let mut best: Option<(usize, f32, f32)> = None;
    for (i, p) in positions.iter().enumerate() {
        let clip = view_proj * p.extend(1.0);
        if clip.w <= 0.0 {
            continue;
        }
        let ndc = clip.truncate() / clip.w;
        let screen = Vec2::new((ndc.x + 1.0) * 0.5 * size.x, (1.0 - ndc.y) * 0.5 * size.y);
        let d = screen.distance(pixel);
        if d > max_pixels {
            continue;
        }
        let better = match best {
            None => true,
            Some((_, bd, bz)) => d + 2.0 < bd || ((d - bd).abs() <= 2.0 && ndc.z < bz),
        };
        if better {
            best = Some((i, d, ndc.z));
        }
    }
    best.map(|(i, _, _)| i)
}

/// Where a ray meets the plane through `point` with normal `normal`.
pub fn ray_plane(origin: Vec3, dir: Vec3, point: Vec3, normal: Vec3) -> Option<Vec3> {
    let denom = dir.dot(normal);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (point - origin).dot(normal) / denom;
    (t > 0.0).then(|| origin + dir * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::OrbitCamera;

    #[test]
    fn picks_the_particle_under_the_cursor() {
        let camera = OrbitCamera::new(Vec3::ZERO, 3.0, 0.6, 0.35);
        let size = Vec2::new(800.0, 600.0);
        let view_proj = camera.projection(size.x / size.y) * camera.view();
        let positions = [
            Vec3::new(0.5, 0.0, 0.0),
            Vec3::ZERO,
            Vec3::new(-0.5, 0.2, 0.0),
        ];
        assert_eq!(pick(&positions, view_proj, size * 0.5, size, 10.0), Some(1));
        assert_eq!(
            pick(&positions[..1], view_proj, size * 0.5, size, 10.0),
            None
        );
    }
}
