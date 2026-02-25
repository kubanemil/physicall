use glam::{Vec3, Vec4};

use crate::camera::{OrbitCamera, Projection};

/// Converts screen coordinates to a world-space ray.
/// Returns (ray_origin, ray_direction).
pub(crate) fn screen_to_world_ray(
    screen_x: f32,
    screen_y: f32,
    canvas_width: u32,
    canvas_height: u32,
    camera: &OrbitCamera,
    projection: &Projection,
) -> (Vec3, Vec3) {
    // Convert screen coords to NDC (-1 to 1)
    let ndc_x = (2.0 * screen_x / canvas_width as f32) - 1.0;
    let ndc_y = 1.0 - (2.0 * screen_y / canvas_height as f32); // Y flipped

    // Get inverse view-projection matrix
    let view = camera.view_matrix();
    let proj = projection.matrix();
    let view_proj = proj * view;
    let inv_view_proj = view_proj.inverse();

    // Unproject near and far points (NDC z: 0 = near, 1 = far for WebGPU)
    let near_ndc = Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
    let far_ndc = Vec4::new(ndc_x, ndc_y, 1.0, 1.0);

    let near_world = inv_view_proj * near_ndc;
    let far_world = inv_view_proj * far_ndc;

    // Perspective divide
    let near_world = Vec3::new(
        near_world.x / near_world.w,
        near_world.y / near_world.w,
        near_world.z / near_world.w,
    );
    let far_world = Vec3::new(
        far_world.x / far_world.w,
        far_world.y / far_world.w,
        far_world.z / far_world.w,
    );

    // Ray from near to far
    let direction = (far_world - near_world).normalize();

    (near_world, direction)
}

/// Computes ray-plane intersection.
/// Returns the intersection point if the ray hits the plane.
pub(crate) fn ray_plane_intersection(
    ray_origin: Vec3,
    ray_dir: Vec3,
    plane_point: Vec3,
    plane_normal: Vec3,
) -> Option<Vec3> {
    let denom = ray_dir.dot(plane_normal);
    if denom.abs() < 0.0001 {
        return None; // Ray parallel to plane
    }

    let t = (plane_point - ray_origin).dot(plane_normal) / denom;
    if t < 0.0 {
        return None; // Intersection behind ray origin
    }

    Some(ray_origin + ray_dir * t)
}

/// Computes ray-sphere intersection.
/// Returns the distance to the nearest intersection point, or None if no hit.
pub(crate) fn ray_sphere_intersection(
    ray_origin: Vec3,
    ray_dir: Vec3,
    sphere_center: Vec3,
    sphere_radius: f32,
) -> Option<f32> {
    // Vector from ray origin to sphere center
    let oc = ray_origin - sphere_center;

    // Quadratic coefficients: at² + bt + c = 0
    let a = ray_dir.dot(ray_dir);
    let b = 2.0 * oc.dot(ray_dir);
    let c = oc.dot(oc) - sphere_radius * sphere_radius;

    let discriminant = b * b - 4.0 * a * c;

    if discriminant < 0.0 {
        return None; // No intersection
    }

    let sqrt_disc = discriminant.sqrt();

    // Find the nearest positive root
    let t1 = (-b - sqrt_disc) / (2.0 * a);
    let t2 = (-b + sqrt_disc) / (2.0 * a);

    if t1 > 0.0 {
        Some(t1)
    } else if t2 > 0.0 {
        Some(t2)
    } else {
        None // Both intersections behind ray origin
    }
}
