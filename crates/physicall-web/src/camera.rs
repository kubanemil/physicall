use glam::{Mat4, Vec2, Vec3, Vec4Swizzles};

const FOV_Y: f32 = std::f32::consts::FRAC_PI_4;
const POLAR_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 0.01;

/// Orbits `target` on a sphere of radius `distance`.
#[derive(Clone, Copy, Debug)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub distance: f32,
    azimuth: f32,
    polar: f32,
}

impl OrbitCamera {
    pub fn new(target: Vec3, distance: f32, azimuth: f32, polar: f32) -> Self {
        Self {
            target,
            distance,
            azimuth,
            polar,
        }
    }

    pub fn eye(&self) -> Vec3 {
        let (sin_p, cos_p) = self.polar.sin_cos();
        let (sin_a, cos_a) = self.azimuth.sin_cos();
        self.target + Vec3::new(cos_p * sin_a, sin_p, cos_p * cos_a) * self.distance
    }

    pub fn view(&self) -> Mat4 {
        glam::camera::rh::view::look_at_mat4(self.eye(), self.target, Vec3::Y)
    }

    pub fn projection(&self, aspect: f32) -> Mat4 {
        glam::camera::rh::proj::directx::perspective(
            FOV_Y,
            aspect,
            0.01 * self.distance,
            100.0 * self.distance,
        )
    }

    /// Rotates by a drag of `delta` pixels on a canvas `height` pixels tall.
    pub fn orbit(&mut self, delta: Vec2, height: f32) {
        let scale = std::f32::consts::PI / height.max(1.0);
        self.azimuth -= delta.x * scale;
        self.polar = (self.polar + delta.y * scale).clamp(-POLAR_LIMIT, POLAR_LIMIT);
    }

    /// Slides the target so the point under the cursor follows it.
    pub fn pan(&mut self, delta: Vec2, height: f32) {
        let view = self.view().inverse();
        let right = view.x_axis.xyz();
        let up = view.y_axis.xyz();
        let scale = 2.0 * self.distance * (0.5 * FOV_Y).tan() / height.max(1.0);
        self.target += (up * delta.y - right * delta.x) * scale;
    }

    pub fn zoom(&mut self, factor: f32) {
        self.distance = (self.distance * factor).clamp(0.2, 50.0);
    }

    /// World-space ray through a pixel, as (origin, unit direction).
    pub fn ray(&self, pixel: Vec2, size: Vec2) -> (Vec3, Vec3) {
        let ndc = Vec2::new(2.0 * pixel.x / size.x - 1.0, 1.0 - 2.0 * pixel.y / size.y);
        let inverse = (self.projection(size.x / size.y) * self.view()).inverse();
        let near = inverse.project_point3(ndc.extend(0.0));
        let far = inverse.project_point3(ndc.extend(1.0));
        (near, (far - near).normalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_through_the_centre_hits_the_target() {
        let camera = OrbitCamera::new(Vec3::new(0.0, 1.0, 0.0), 3.0, 0.6, 0.35);
        let size = Vec2::new(800.0, 600.0);
        let (origin, dir) = camera.ray(size * 0.5, size);
        let along = (camera.target - origin).dot(dir);
        assert!((origin + dir * along).distance(camera.target) < 1e-3);
    }
}
