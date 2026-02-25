use std::f32::consts::FRAC_PI_2;
use glam::{Mat4, Vec3};

/// Default camera parameters
const DEFAULT_RADIUS: f32 = 3.0;
const DEFAULT_AZIMUTH: f32 = std::f32::consts::FRAC_PI_4; // 45°
const DEFAULT_POLAR: f32 = std::f32::consts::FRAC_PI_6;   // 30°

/// Safe limit for polar angle to avoid gimbal lock
const SAFE_FRAC_PI_2: f32 = FRAC_PI_2 - 0.0001;

/// Orbit camera that rotates around a target point using spherical coordinates.
pub(crate) struct OrbitCamera {
    /// Point the camera orbits around
    pub(crate) target: Vec3,
    /// Distance from target (zoom level)
    radius: f32,
    /// Horizontal angle in radians
    azimuth: f32,
    /// Vertical angle in radians (clamped to avoid gimbal lock)
    polar: f32,
    /// Minimum zoom distance
    min_radius: f32,
    /// Maximum zoom distance
    max_radius: f32,
}

impl OrbitCamera {
    /// Creates a new orbit camera with default settings.
    pub(crate) fn new() -> Self {
        Self {
            target: Vec3::ZERO,
            radius: DEFAULT_RADIUS,
            azimuth: DEFAULT_AZIMUTH,
            polar: DEFAULT_POLAR,
            min_radius: 0.5,
            max_radius: 50.0,
        }
    }

    /// Calculate the camera's eye position in world space from spherical coordinates.
    pub(crate) fn eye(&self) -> Vec3 {
        let cos_polar = self.polar.cos();
        let sin_polar = self.polar.sin();
        let cos_azimuth = self.azimuth.cos();
        let sin_azimuth = self.azimuth.sin();

        Vec3::new(
            self.target.x + self.radius * cos_polar * sin_azimuth,
            self.target.y + self.radius * sin_polar,
            self.target.z + self.radius * cos_polar * cos_azimuth,
        )
    }

    /// Calculate the view matrix (world to camera space).
    pub(crate) fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye(), self.target, Vec3::Y)
    }

    /// Rotate the camera horizontally (around Y axis).
    pub(crate) fn rotate_azimuth(&mut self, delta: f32) {
        self.azimuth += delta;
    }

    /// Rotate the camera vertically, clamped to prevent gimbal lock.
    pub(crate) fn rotate_polar(&mut self, delta: f32) {
        self.polar = (self.polar + delta).clamp(-SAFE_FRAC_PI_2, SAFE_FRAC_PI_2);
    }

    /// Zoom in/out by changing radius, clamped to min/max.
    pub(crate) fn zoom(&mut self, delta: f32) {
        self.radius = (self.radius - delta).clamp(self.min_radius, self.max_radius);
    }

    /// Reset camera to default position.
    pub(crate) fn reset(&mut self) {
        self.target = Vec3::ZERO;
        self.radius = DEFAULT_RADIUS;
        self.azimuth = DEFAULT_AZIMUTH;
        self.polar = DEFAULT_POLAR;
    }
}

/// Perspective projection parameters.
pub(crate) struct Projection {
    aspect: f32,
    fov_y: f32,
    z_near: f32,
    z_far: f32,
}

impl Projection {
    /// Creates a new projection with default FOV and clip planes.
    pub(crate) fn new(width: u32, height: u32) -> Self {
        Self {
            aspect: width as f32 / height.max(1) as f32,
            fov_y: 45.0_f32.to_radians(),
            z_near: 0.1,
            z_far: 100.0,
        }
    }

    /// Update aspect ratio on window resize.
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height.max(1) as f32;
    }

    /// Calculate the perspective projection matrix for WebGPU (depth [0,1]).
    pub(crate) fn matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, self.aspect, self.z_near, self.z_far)
    }
}

/// GPU-compatible camera uniform data.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    /// Creates a new camera uniform with identity matrix.
    pub(crate) fn new() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        }
    }

    /// Update the view-projection matrix from camera and projection.
    pub(crate) fn update(&mut self, camera: &OrbitCamera, projection: &Projection) {
        let view = camera.view_matrix();
        let proj = projection.matrix();
        self.view_proj = (proj * view).to_cols_array_2d();
    }
}
