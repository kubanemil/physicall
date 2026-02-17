use std::cell::RefCell;
use std::f32::consts::FRAC_PI_2;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wgpu::util::DeviceExt;
use glam::{Mat4, Vec3};
use web_sys::{HtmlCanvasElement, MouseEvent, WheelEvent};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

// ============================================================================
// Camera System
// ============================================================================

/// Default camera parameters
const DEFAULT_RADIUS: f32 = 3.0;
const DEFAULT_AZIMUTH: f32 = std::f32::consts::FRAC_PI_4; // 45°
const DEFAULT_POLAR: f32 = std::f32::consts::FRAC_PI_6;   // 30°

/// Safe limit for polar angle to avoid gimbal lock
const SAFE_FRAC_PI_2: f32 = FRAC_PI_2 - 0.0001;

/// Orbit camera that rotates around a target point using spherical coordinates.
struct OrbitCamera {
    /// Point the camera orbits around
    target: Vec3,
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
    fn new() -> Self {
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
    fn eye(&self) -> Vec3 {
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
    fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye(), self.target, Vec3::Y)
    }

    /// Rotate the camera horizontally (around Y axis).
    fn rotate_azimuth(&mut self, delta: f32) {
        self.azimuth += delta;
    }

    /// Rotate the camera vertically, clamped to prevent gimbal lock.
    fn rotate_polar(&mut self, delta: f32) {
        self.polar = (self.polar + delta).clamp(-SAFE_FRAC_PI_2, SAFE_FRAC_PI_2);
    }

    /// Zoom in/out by changing radius, clamped to min/max.
    fn zoom(&mut self, delta: f32) {
        self.radius = (self.radius - delta).clamp(self.min_radius, self.max_radius);
    }

    /// Reset camera to default position.
    fn reset(&mut self) {
        self.target = Vec3::ZERO;
        self.radius = DEFAULT_RADIUS;
        self.azimuth = DEFAULT_AZIMUTH;
        self.polar = DEFAULT_POLAR;
    }
}

/// Perspective projection parameters.
struct Projection {
    aspect: f32,
    fov_y: f32,
    z_near: f32,
    z_far: f32,
}

impl Projection {
    /// Creates a new projection with default FOV and clip planes.
    fn new(width: u32, height: u32) -> Self {
        Self {
            aspect: width as f32 / height.max(1) as f32,
            fov_y: 45.0_f32.to_radians(),
            z_near: 0.1,
            z_far: 100.0,
        }
    }

    /// Update aspect ratio on window resize.
    fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height.max(1) as f32;
    }

    /// Calculate the perspective projection matrix for WebGPU (depth [0,1]).
    fn matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, self.aspect, self.z_near, self.z_far)
    }
}

/// GPU-compatible camera uniform data.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    /// Creates a new camera uniform with identity matrix.
    fn new() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        }
    }

    /// Update the view-projection matrix from camera and projection.
    fn update(&mut self, camera: &OrbitCamera, projection: &Projection) {
        let view = camera.view_matrix();
        let proj = projection.matrix();
        self.view_proj = (proj * view).to_cols_array_2d();
    }
}

// ============================================================================
// Ray Unprojection (for click-to-spawn)
// ============================================================================

use glam::Vec4;

/// Converts screen coordinates to a world-space ray.
/// Returns (ray_origin, ray_direction).
fn screen_to_world_ray(
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
fn ray_plane_intersection(
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
fn ray_sphere_intersection(
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

// ============================================================================
// Input Handling
// ============================================================================

/// Threshold in pixels for distinguishing click from drag.
const CLICK_THRESHOLD: i32 = 5;

/// Tracks mouse input state between events.
struct InputState {
    is_dragging: bool,
    last_x: i32,
    last_y: i32,
    rotate_delta: (f32, f32),
    zoom_delta: f32,
    // Click detection for spawning
    mouse_down_pos: Option<(i32, i32)>,  // Position at mousedown
    pending_spawn: Option<(f32, f32)>,   // Screen coords for spawn (client coords)
    total_drag_distance: i32,            // Accumulated movement since mousedown
    // Particle interaction
    pending_pick: Option<(f32, f32)>,          // Screen pos for pick test on mousedown
    dragging_particle_idx: Option<u32>,        // Currently dragged particle index
    current_mouse_screen_pos: Option<(f32, f32)>, // Current mouse position for drag & hover
    pending_delete: Option<(f32, f32)>,        // Screen coords for middle-click delete
}

impl InputState {
    fn new() -> Self {
        Self {
            is_dragging: false,
            last_x: 0,
            last_y: 0,
            rotate_delta: (0.0, 0.0),
            zoom_delta: 0.0,
            mouse_down_pos: None,
            pending_spawn: None,
            total_drag_distance: 0,
            pending_pick: None,
            dragging_particle_idx: None,
            current_mouse_screen_pos: None,
            pending_delete: None,
        }
    }

    /// Returns and clears accumulated input deltas.
    fn take_deltas(&mut self) -> ((f32, f32), f32) {
        let rotate = self.rotate_delta;
        let zoom = self.zoom_delta;
        self.rotate_delta = (0.0, 0.0);
        self.zoom_delta = 0.0;
        (rotate, zoom)
    }

    /// Returns and clears pending spawn position.
    fn take_pending_spawn(&mut self) -> Option<(f32, f32)> {
        self.pending_spawn.take()
    }
}

// Global input state accessible from event closures
thread_local! {
    static INPUT_STATE: Rc<RefCell<InputState>> = Rc::new(RefCell::new(InputState::new()));
}

/// Set up mouse event listeners for camera control, click-to-spawn, and particle interaction.
fn setup_mouse_events(
    canvas: &HtmlCanvasElement,
    state: Rc<RefCell<InputState>>,
) -> Result<(), JsValue> {
    // Prevent context menu on right-click to allow middle-click detection
    {
        let closure = Closure::<dyn FnMut(_)>::new(move |event: MouseEvent| {
            event.prevent_default();
        });
        canvas.add_event_listener_with_callback("contextmenu", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Mousedown - start dragging, record position, set up pick/delete
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: MouseEvent| {
            let mut s = state.borrow_mut();
            let button = event.button();

            if button == 0 {
                // Left button - camera orbit or particle drag
                s.is_dragging = true;
                s.last_x = event.client_x();
                s.last_y = event.client_y();
                s.mouse_down_pos = Some((event.offset_x(), event.offset_y()));
                s.total_drag_distance = 0;
                // Set pending pick for render loop to determine if we clicked a particle
                s.pending_pick = Some((event.offset_x() as f32, event.offset_y() as f32));
            } else if button == 1 {
                // Middle button - delete particle
                event.prevent_default();
                s.pending_delete = Some((event.offset_x() as f32, event.offset_y() as f32));
            }
        });
        canvas.add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Mousemove - accumulate rotation delta and track mouse position
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: MouseEvent| {
            let mut s = state.borrow_mut();
            // Always update mouse position for hover detection and drag target
            s.current_mouse_screen_pos = Some((event.offset_x() as f32, event.offset_y() as f32));

            if s.is_dragging {
                let dx = event.client_x() - s.last_x;
                let dy = event.client_y() - s.last_y;
                // Accumulate camera rotation (ignored if particle dragging)
                s.rotate_delta.0 += dx as f32;
                s.rotate_delta.1 += dy as f32;
                s.total_drag_distance += dx.abs() + dy.abs();
                s.last_x = event.client_x();
                s.last_y = event.client_y();
            }
        });
        canvas.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Mouseup - stop dragging, detect clicks, release particle
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: MouseEvent| {
            if event.button() != 0 {
                return; // Only handle left button release
            }
            let mut s = state.borrow_mut();
            if s.is_dragging {
                s.is_dragging = false;

                if s.dragging_particle_idx.is_some() {
                    // Was dragging a particle - release it (momentum preserved by physics)
                    s.dragging_particle_idx = None;
                } else if s.total_drag_distance < CLICK_THRESHOLD {
                    // Was a click on empty space - spawn particle
                    if let Some((down_x, down_y)) = s.mouse_down_pos {
                        s.pending_spawn = Some((down_x as f32, down_y as f32));
                    }
                }
                s.mouse_down_pos = None;
            }
        });
        canvas.add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Also listen on window for mouseup outside canvas
    {
        let state = state.clone();
        let window = web_sys::window().expect("No window");
        let closure = Closure::<dyn FnMut(_)>::new(move |_: MouseEvent| {
            let mut s = state.borrow_mut();
            s.is_dragging = false;
            s.mouse_down_pos = None;
            s.dragging_particle_idx = None;
        });
        window.add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    Ok(())
}

/// Set up wheel event listener for zoom control.
fn setup_wheel_event(
    canvas: &HtmlCanvasElement,
    state: Rc<RefCell<InputState>>,
) -> Result<(), JsValue> {
    let closure = Closure::<dyn FnMut(_)>::new(move |event: WheelEvent| {
        event.prevent_default(); // Prevent page scroll

        let delta = event.delta_y();
        // Normalize delta based on delta_mode
        let normalized = match event.delta_mode() {
            0 => delta / 100.0, // Pixels
            1 => delta / 3.0,   // Lines
            _ => delta / 100.0,
        };

        state.borrow_mut().zoom_delta += normalized as f32;
    });

    // Must use non-passive listener to call preventDefault
    let options = web_sys::AddEventListenerOptions::new();
    options.set_passive(false);

    canvas.add_event_listener_with_callback_and_add_event_listener_options(
        "wheel",
        closure.as_ref().unchecked_ref(),
        &options,
    )?;
    closure.forget();

    Ok(())
}

// ============================================================================
// Depth Buffer
// ============================================================================

/// Manages the depth texture for proper 3D rendering with depth testing.
struct DepthTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl DepthTexture {
    /// Depth buffer format - 32-bit float for precision.
    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    /// Creates a new depth texture with the given dimensions.
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self { texture, view }
    }
}

// ============================================================================
// Rendering
// ============================================================================

/// Vertex with position and color for triangle rendering.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        0 => Float32x3,  // position @ location(0)
        1 => Float32x3,  // color @ location(1)
    ];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Line vertices for coordinate axes: X (red), Y (green), Z (blue).
/// Extended from -2 to +2 on each axis for better visibility.
const LINE_VERTICES: &[Vertex] = &[
    // X axis (red) - from -2 to +2
    Vertex { position: [-2.0, 0.0, 0.0], color: [0.5, 0.0, 0.0] },  // Darker for negative
    Vertex { position: [ 0.0, 0.0, 0.0], color: [0.5, 0.0, 0.0] },
    Vertex { position: [ 0.0, 0.0, 0.0], color: [1.0, 0.0, 0.0] },  // Brighter for positive
    Vertex { position: [ 2.0, 0.0, 0.0], color: [1.0, 0.0, 0.0] },
    // Y axis (green) - from -2 to +2
    Vertex { position: [0.0, -2.0, 0.0], color: [0.0, 0.5, 0.0] },
    Vertex { position: [0.0,  0.0, 0.0], color: [0.0, 0.5, 0.0] },
    Vertex { position: [0.0,  0.0, 0.0], color: [0.0, 1.0, 0.0] },
    Vertex { position: [0.0,  2.0, 0.0], color: [0.0, 1.0, 0.0] },
    // Z axis (blue) - from -2 to +2
    Vertex { position: [0.0, 0.0, -2.0], color: [0.0, 0.0, 0.5] },
    Vertex { position: [0.0, 0.0,  0.0], color: [0.0, 0.0, 0.5] },
    Vertex { position: [0.0, 0.0,  0.0], color: [0.0, 0.0, 1.0] },
    Vertex { position: [0.0, 0.0,  2.0], color: [0.0, 0.0, 1.0] },
];

// ============================================================================
// Point Rendering (Instanced Quads)
// ============================================================================

/// Quad vertex for point rendering - just 2D position for quad corners.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct QuadVertex {
    position: [f32; 2],
}

impl QuadVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![
        0 => Float32x2,  // quad position @ location(0)
    ];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<QuadVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Quad vertices: 4 corners of a unit quad (-1 to 1).
const QUAD_VERTICES: &[QuadVertex] = &[
    QuadVertex { position: [-1.0, -1.0] },
    QuadVertex { position: [ 1.0, -1.0] },
    QuadVertex { position: [-1.0,  1.0] },
    QuadVertex { position: [ 1.0,  1.0] },
];

/// Quad indices: 2 triangles forming the quad.
const QUAD_INDICES: &[u16] = &[0, 1, 2, 2, 1, 3];

/// Per-instance data for particle rendering.
/// Used by both compute shader (storage buffer) and render shader (vertex buffer).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Particle {
    position: [f32; 3],
    size: f32,
    velocity: [f32; 3],
    _padding: f32,  // Align to 32 bytes
}

impl Particle {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        1 => Float32x3,  // world_pos @ location(1)
        2 => Float32,    // size @ location(2)
        3 => Float32x3,  // velocity @ location(3)
    ];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Particle>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Simulation parameters uniform buffer.
/// Layout must match SimParams in particle_compute.wgsl and point.wgsl.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SimParams {
    delta_time: f32,
    gravitational_constant: f32,
    damping: f32,
    particle_count: u32,
    softening_sq: f32,
    drag_strength: f32,
    dragged_particle_idx: i32,
    hovered_particle_idx: i32,
    drag_target: [f32; 3],
    restitution: f32,
    floor_y: f32,
    cell_size: f32,
    num_cells: u32,
    _padding: f32,
}

/// Physics simulation constants for N-body simulation
const PARTICLE_COUNT: u32 = 500;          // Increased with spatial hash collision
const MAX_PARTICLES: u32 = 2_000;         // Pre-allocated buffer capacity
const PARTICLE_SIZE: f32 = 0.03;          // Larger for fewer particles
const GRAVITATIONAL_CONSTANT: f32 = 0.3;  // Tunable G (lower for stability)
const SOFTENING_SQ: f32 = 0.1;            // Prevents singularity at close range
const DAMPING: f32 = 1.0;                 // No damping for orbital conservation
const DRAG_STRENGTH: f32 = 50.0;          // Spring force multiplier for mouse drag
const RESTITUTION: f32 = 0.8;             // Slightly inelastic collisions
const FLOOR_Y: f32 = -2.0;               // Floor boundary y-coordinate
const NUM_CELLS: u32 = 4096;             // Spatial hash table size (power of 2)
const MAX_PER_CELL: u32 = 16;            // Max particles per hash cell bucket
const CELL_SIZE: f32 = 2.0 * PARTICLE_SIZE; // Grid cell size (0.06)

/// Creates initial particles in a ring distribution for N-body simulation.
/// Particles are arranged in a disk in the XZ plane with tangential velocities
/// to create orbital motion.
fn create_initial_particles(count: u32) -> Vec<Particle> {
    let mut particles = Vec::with_capacity(count as usize);

    // Ring parameters
    let inner_radius = 0.8;
    let outer_radius = 2.0;

    // Tangential velocity for roughly circular orbits
    // v = sqrt(G * M_enclosed / r) approximated empirically
    let base_orbital_speed = 0.4;

    for i in 0..count {
        let t = i as f32 / count as f32;
        let angle = t * std::f32::consts::PI * 2.0 * 3.0; // 3 rotations for spiral

        // Distribute radius from inner to outer
        let radius = inner_radius + (outer_radius - inner_radius) * t;

        // Position in XZ plane (y = 0) with small vertical variation
        let y_variation = 0.1 * ((i as f32 * 0.5).sin());
        let x = radius * angle.cos();
        let z = radius * angle.sin();
        let y = y_variation;

        // Tangential velocity (perpendicular to radius in XZ plane)
        // Scale velocity with sqrt(1/radius) for Keplerian-ish motion
        let speed = base_orbital_speed * (1.0 / radius).sqrt();
        let vx = -speed * angle.sin(); // Perpendicular to radius
        let vz = speed * angle.cos();
        let vy = 0.0;

        particles.push(Particle {
            position: [x, y, z],
            size: PARTICLE_SIZE,
            velocity: [vx, vy, vz],
            _padding: 0.0,
        });
    }

    particles
}

// ============================================================================
// Sphere Rendering (Instanced Icosphere)
// ============================================================================

/// Vertex for sphere mesh with position and normal.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SphereVertex {
    position: [f32; 3],
    normal: [f32; 3],
}

impl SphereVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        0 => Float32x3,  // position @ location(0)
        1 => Float32x3,  // normal @ location(1)
    ];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SphereVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Per-instance data for sphere rendering.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SphereInstance {
    position: [f32; 3],
    radius: f32,
    color: [f32; 3],
    _padding: f32,
}

impl SphereInstance {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        2 => Float32x3,  // world_pos @ location(2)
        3 => Float32,    // radius @ location(3)
        4 => Float32x3,  // color @ location(4)
    ];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SphereInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Generates an icosphere mesh with the given number of subdivisions.
/// Returns (vertices, indices).
/// Level 0 = 12 vertices, 20 triangles (base icosahedron)
/// Level 1 = 42 vertices, 80 triangles
/// Level 2 = 162 vertices, 320 triangles
fn generate_icosphere(subdivisions: u32) -> (Vec<SphereVertex>, Vec<u32>) {
    use std::collections::BTreeMap;

    // Golden ratio for icosahedron construction
    let t = (1.0 + 5.0_f32.sqrt()) / 2.0;

    // Normalize to unit sphere
    let len = (1.0 + t * t).sqrt();
    let a = 1.0 / len;
    let b = t / len;

    // 12 vertices of icosahedron (3 orthogonal golden rectangles)
    let mut vertices: Vec<[f32; 3]> = vec![
        [-a,  b,  0.0], [ a,  b,  0.0], [-a, -b,  0.0], [ a, -b,  0.0],
        [ 0.0, -a,  b], [ 0.0,  a,  b], [ 0.0, -a, -b], [ 0.0,  a, -b],
        [ b,  0.0, -a], [ b,  0.0,  a], [-b,  0.0, -a], [-b,  0.0,  a],
    ];

    // 20 triangles of icosahedron
    let mut indices: Vec<[u32; 3]> = vec![
        // 5 faces around vertex 0
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        // 5 adjacent faces
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        // 5 faces around vertex 3
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        // 5 adjacent faces
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];

    // Subdivide
    for _ in 0..subdivisions {
        let mut new_indices: Vec<[u32; 3]> = Vec::new();
        let mut edge_cache: BTreeMap<(u32, u32), u32> = BTreeMap::new();

        for tri in &indices {
            let v0 = tri[0];
            let v1 = tri[1];
            let v2 = tri[2];

            // Get or create midpoint vertices
            let m01 = get_midpoint(v0, v1, &mut vertices, &mut edge_cache);
            let m12 = get_midpoint(v1, v2, &mut vertices, &mut edge_cache);
            let m20 = get_midpoint(v2, v0, &mut vertices, &mut edge_cache);

            // Create 4 new triangles
            new_indices.push([v0, m01, m20]);
            new_indices.push([v1, m12, m01]);
            new_indices.push([v2, m20, m12]);
            new_indices.push([m01, m12, m20]);
        }

        indices = new_indices;
    }

    // Convert to SphereVertex (normal = position for unit sphere)
    let sphere_vertices: Vec<SphereVertex> = vertices
        .iter()
        .map(|&pos| SphereVertex {
            position: pos,
            normal: pos, // For unit sphere, normal equals position
        })
        .collect();

    // Flatten indices
    let flat_indices: Vec<u32> = indices.iter().flat_map(|tri| tri.iter().copied()).collect();

    (sphere_vertices, flat_indices)
}

/// Gets or creates a midpoint vertex between two vertices, normalized to unit sphere.
fn get_midpoint(
    v0: u32,
    v1: u32,
    vertices: &mut Vec<[f32; 3]>,
    cache: &mut std::collections::BTreeMap<(u32, u32), u32>,
) -> u32 {
    // Use sorted key to ensure consistent lookup regardless of edge direction
    let key = if v0 < v1 { (v0, v1) } else { (v1, v0) };

    if let Some(&idx) = cache.get(&key) {
        return idx;
    }

    // Calculate midpoint
    let p0 = vertices[v0 as usize];
    let p1 = vertices[v1 as usize];
    let mid = [
        (p0[0] + p1[0]) / 2.0,
        (p0[1] + p1[1]) / 2.0,
        (p0[2] + p1[2]) / 2.0,
    ];

    // Normalize to unit sphere
    let len = (mid[0] * mid[0] + mid[1] * mid[1] + mid[2] * mid[2]).sqrt();
    let normalized = [mid[0] / len, mid[1] / len, mid[2] / len];

    // Add new vertex
    let new_idx = vertices.len() as u32;
    vertices.push(normalized);
    cache.insert(key, new_idx);

    new_idx
}

/// Creates demo sphere instances for testing.
fn create_demo_spheres() -> Vec<SphereInstance> {
    vec![
        // Central large sphere (white)
        SphereInstance {
            position: [0.0, 0.0, -0.5],
            radius: 0.3,
            color: [0.9, 0.9, 0.9],
            _padding: 0.0,
        },
        // Red sphere
        SphereInstance {
            position: [0.8, 0.2, 0.0],
            radius: 0.2,
            color: [1.0, 0.3, 0.3],
            _padding: 0.0,
        },
        // Green sphere
        SphereInstance {
            position: [-0.7, -0.3, 0.2],
            radius: 0.25,
            color: [0.3, 1.0, 0.3],
            _padding: 0.0,
        },
        // Blue sphere
        SphereInstance {
            position: [0.3, 0.8, 0.3],
            radius: 0.15,
            color: [0.3, 0.3, 1.0],
            _padding: 0.0,
        },
        // Yellow sphere
        SphereInstance {
            position: [-0.5, 0.6, -0.3],
            radius: 0.18,
            color: [1.0, 1.0, 0.3],
            _padding: 0.0,
        },
        // Cyan sphere
        SphereInstance {
            position: [0.6, -0.6, 0.4],
            radius: 0.22,
            color: [0.3, 1.0, 1.0],
            _padding: 0.0,
        },
        // Magenta sphere
        SphereInstance {
            position: [-0.8, 0.0, 0.5],
            radius: 0.12,
            color: [1.0, 0.3, 1.0],
            _padding: 0.0,
        },
        // Orange sphere
        SphereInstance {
            position: [0.0, -0.8, -0.2],
            radius: 0.2,
            color: [1.0, 0.6, 0.2],
            _padding: 0.0,
        },
    ]
}

/// Holds all wgpu resources needed for rendering.
struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: (u32, u32),
    // Camera system
    camera: OrbitCamera,
    projection: Projection,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    // Depth buffer
    depth_texture: DepthTexture,
    // Line rendering
    line_pipeline: wgpu::RenderPipeline,
    line_vertex_buffer: wgpu::Buffer,
    line_vertex_count: u32,
    // Particle system (compute + render)
    particle_buffers: [wgpu::Buffer; 2],  // Double-buffered for ping-pong
    active_particle_count: u32,           // Current number of active particles
    max_particles: u32,                   // Buffer capacity
    compute_pipeline: wgpu::ComputePipeline,
    hash_clear_pipeline: wgpu::ComputePipeline,
    hash_insert_pipeline: wgpu::ComputePipeline,
    compute_bind_groups: [wgpu::BindGroup; 2],
    hash_bind_group: wgpu::BindGroup,
    cell_count_buffer: wgpu::Buffer,
    cell_entries_buffer: wgpu::Buffer,
    sim_params_buffer: wgpu::Buffer,
    frame_index: u32,
    last_frame_time: Option<f64>,
    // Point rendering (instanced quads)
    point_pipeline: wgpu::RenderPipeline,
    quad_vertex_buffer: wgpu::Buffer,
    quad_index_buffer: wgpu::Buffer,
    // Sphere rendering (instanced icosphere)
    sphere_pipeline: wgpu::RenderPipeline,
    sphere_vertex_buffer: wgpu::Buffer,
    sphere_index_buffer: wgpu::Buffer,
    sphere_index_count: u32,
    sphere_instance_buffer: wgpu::Buffer,
    sphere_instance_count: u32,
    // UI Controls state
    paused: bool,
    gravity: f32,
    time_scale: f32,
    initial_particles: Vec<Particle>,
    initial_particle_count: u32,
    // CPU-side particle positions for picking
    particle_positions: Vec<Vec3>,
    // Particle interaction state
    hovered_particle_idx: Option<u32>,
    dragged_particle_idx: Option<u32>,
    drag_target: Option<Vec3>,
    // Point rendering bind group (includes camera + sim_params)
    point_bind_group: wgpu::BindGroup,
}

impl State {
    /// Creates a new State with WebGPU initialized for the given canvas.
    async fn new(canvas: web_sys::HtmlCanvasElement) -> Self {
        let width = canvas.width();
        let height = canvas.height();

        // Create wgpu instance with WebGPU backend
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });

        // Create surface from canvas
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .expect("Failed to create surface from canvas");

        // Request adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find a suitable GPU adapter");

        // Request device and queue
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .expect("Failed to create GPU device");

        // Get surface capabilities and configure
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        // Initialize camera system
        let camera = OrbitCamera::new();
        let projection = Projection::new(width, height);
        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update(&camera, &projection);

        // Create camera uniform buffer
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create camera bind group layout
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // Create camera bind group
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // Create pipeline layout with camera bind group
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout],
            ..Default::default()
        });

        // Create depth texture
        let depth_texture = DepthTexture::new(&device, width, height);

        // ====================================================================
        // Line Pipeline Setup
        // ====================================================================

        // Load line shader
        let line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Line Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../../shaders/line.wgsl").into()),
        });

        // Create line render pipeline (uses same layout as triangle pipeline)
        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Line Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &line_shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &line_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Lines have no faces to cull
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthTexture::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        // Create line vertex buffer
        let line_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Line Vertex Buffer"),
            contents: bytemuck::cast_slice(LINE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let line_vertex_count = LINE_VERTICES.len() as u32;

        // ====================================================================
        // Simulation Parameters (needed by point pipeline)
        // ====================================================================

        // Create simulation parameters uniform buffer
        let sim_params = SimParams {
            delta_time: 1.0 / 60.0,
            gravitational_constant: GRAVITATIONAL_CONSTANT,
            damping: DAMPING,
            particle_count: PARTICLE_COUNT,
            softening_sq: SOFTENING_SQ,
            drag_strength: 0.0,
            dragged_particle_idx: -1,
            hovered_particle_idx: -1,
            drag_target: [0.0; 3],
            restitution: RESTITUTION,
            floor_y: FLOOR_Y,
            cell_size: CELL_SIZE,
            num_cells: NUM_CELLS,
            _padding: 0.0,
        };
        let sim_params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Simulation Params Buffer"),
            contents: bytemuck::cast_slice(&[sim_params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create point bind group layout (camera + sim_params for point shader)
        let point_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Point Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let point_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Point Bind Group"),
            layout: &point_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: sim_params_buffer.as_entire_binding(),
                },
            ],
        });

        let point_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Point Pipeline Layout"),
                bind_group_layouts: &[&point_bind_group_layout],
                ..Default::default()
            });

        // ====================================================================
        // Point Pipeline Setup (Instanced Quads)
        // ====================================================================

        // Load point shader
        let point_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Point Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../../shaders/point.wgsl").into()),
        });

        // Create point render pipeline with two vertex buffers (quad + particle instance)
        let point_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Point Pipeline"),
            layout: Some(&point_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &point_shader,
                entry_point: Some("vs_main"),
                buffers: &[QuadVertex::desc(), Particle::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &point_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Don't cull - quads should be visible from both sides
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthTexture::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        // Create quad vertex buffer (shared geometry for all points)
        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(QUAD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Create quad index buffer
        let quad_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Index Buffer"),
            contents: bytemuck::cast_slice(QUAD_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        // ====================================================================
        // Particle System Setup (Compute + Render Buffer Sharing)
        // ====================================================================

        // Create initial particles (clone for reset capability)
        let initial_particles = create_initial_particles(PARTICLE_COUNT);
        let initial_particle_count = initial_particles.len() as u32;
        let active_particle_count = initial_particle_count;

        // Buffer usage for compute-render sharing (COPY_SRC needed for delete-by-swap)
        let particle_buffer_usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;

        // Pre-allocate buffers for MAX_PARTICLES capacity
        let buffer_size = (MAX_PARTICLES as usize) * std::mem::size_of::<Particle>();

        // Create double-buffered particle storage (ping-pong pattern)
        // Initialize with zeros, then write initial particles
        let particle_buffers = [
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Particle Buffer 0"),
                size: buffer_size as u64,
                usage: particle_buffer_usage,
                mapped_at_creation: false,
            }),
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Particle Buffer 1"),
                size: buffer_size as u64,
                usage: particle_buffer_usage,
                mapped_at_creation: false,
            }),
        ];

        // Write initial particles to both buffers
        queue.write_buffer(&particle_buffers[0], 0, bytemuck::cast_slice(&initial_particles));
        queue.write_buffer(&particle_buffers[1], 0, bytemuck::cast_slice(&initial_particles));

        // Create compute bind group layout (group 0: particles + sim_params)
        let compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Compute Bind Group Layout"),
                entries: &[
                    // Binding 0: Input particles (read-only storage)
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Binding 1: Output particles (read-write storage)
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Binding 2: Simulation parameters (uniform)
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // Create hash bind group layout (group 1: spatial hash data)
        let hash_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Hash Bind Group Layout"),
                entries: &[
                    // Binding 0: Cell counts (read_write for clear/insert, read for physics)
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Binding 1: Cell entries (read_write for insert, read for physics)
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // Create spatial hash GPU buffers
        let cell_count_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cell Count Buffer"),
            size: (NUM_CELLS * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let cell_entries_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cell Entries Buffer"),
            size: (NUM_CELLS * MAX_PER_CELL * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        // Create hash bind group
        let hash_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Hash Bind Group"),
            layout: &hash_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cell_count_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: cell_entries_buffer.as_entire_binding(),
                },
            ],
        });

        // Shared compute pipeline layout: [compute_group, hash_group]
        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Compute Pipeline Layout"),
                bind_group_layouts: &[&compute_bind_group_layout, &hash_bind_group_layout],
                ..Default::default()
            });

        // Load shaders
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/particle_compute.wgsl").into(),
            ),
        });
        let hash_clear_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Hash Clear Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/hash_clear.wgsl").into(),
            ),
        });
        let hash_insert_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Hash Insert Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/hash_insert.wgsl").into(),
            ),
        });

        // Create compute pipelines (all share the same 2-group layout)
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Particle Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let hash_clear_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Hash Clear Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &hash_clear_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let hash_insert_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Hash Insert Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &hash_insert_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Create two bind groups for ping-pong pattern (group 0)
        // bind_group_0: read from buffer[0], write to buffer[1]
        // bind_group_1: read from buffer[1], write to buffer[0]
        let compute_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Compute Bind Group 0"),
                layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: particle_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: particle_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: sim_params_buffer.as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Compute Bind Group 1"),
                layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: particle_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: particle_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: sim_params_buffer.as_entire_binding(),
                    },
                ],
            }),
        ];

        // ====================================================================
        // Sphere Pipeline Setup (Instanced Icosphere)
        // ====================================================================

        // Generate icosphere mesh (level 2 = 162 vertices, 320 triangles)
        let (sphere_vertices, sphere_indices) = generate_icosphere(2);
        let sphere_index_count = sphere_indices.len() as u32;

        // Load sphere shader
        let sphere_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sphere Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../../shaders/sphere.wgsl").into()),
        });

        // Create sphere render pipeline
        let sphere_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sphere Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &sphere_shader,
                entry_point: Some("vs_main"),
                buffers: &[SphereVertex::desc(), SphereInstance::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &sphere_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back), // Cull back faces for solid spheres
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthTexture::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        // Create sphere vertex buffer
        let sphere_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sphere Vertex Buffer"),
            contents: bytemuck::cast_slice(&sphere_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Create sphere index buffer
        let sphere_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sphere Index Buffer"),
            contents: bytemuck::cast_slice(&sphere_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Create sphere instance buffer with demo data
        let demo_spheres = create_demo_spheres();
        let sphere_instance_count = demo_spheres.len() as u32;

        let sphere_instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sphere Instance Buffer"),
            contents: bytemuck::cast_slice(&demo_spheres),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            surface,
            device,
            queue,
            config,
            size: (width, height),
            camera,
            projection,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            depth_texture,
            line_pipeline,
            line_vertex_buffer,
            line_vertex_count,
            particle_buffers,
            active_particle_count,
            max_particles: MAX_PARTICLES,
            compute_pipeline,
            hash_clear_pipeline,
            hash_insert_pipeline,
            compute_bind_groups,
            hash_bind_group,
            cell_count_buffer,
            cell_entries_buffer,
            sim_params_buffer,
            frame_index: 0,
            last_frame_time: None,
            point_pipeline,
            quad_vertex_buffer,
            quad_index_buffer,
            sphere_pipeline,
            sphere_vertex_buffer,
            sphere_index_buffer,
            sphere_index_count,
            sphere_instance_buffer,
            sphere_instance_count,
            // UI Controls state
            paused: false,
            gravity: GRAVITATIONAL_CONSTANT,
            time_scale: 1.0,
            initial_particles: initial_particles.clone(),
            initial_particle_count,
            // CPU-side particle positions for picking
            particle_positions: initial_particles
                .iter()
                .map(|p| Vec3::new(p.position[0], p.position[1], p.position[2]))
                .collect(),
            // Particle interaction
            hovered_particle_idx: None,
            dragged_particle_idx: None,
            drag_target: None,
            point_bind_group,
        }
    }

    /// Reconfigures the surface with new dimensions.
    fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.size = (width, height);
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.projection.resize(width, height);
            // Recreate depth texture with new dimensions
            self.depth_texture = DepthTexture::new(&self.device, width, height);
        }
    }

    /// Spawns a new particle at the given world position.
    fn spawn_particle(&mut self, world_pos: Vec3) {
        if self.active_particle_count >= self.max_particles {
            return; // Buffer full
        }

        let particle = Particle {
            position: [world_pos.x, world_pos.y, world_pos.z],
            size: PARTICLE_SIZE,
            velocity: [0.0, 0.0, 0.0],
            _padding: 0.0,
        };

        // Write to both buffers at active_particle_count offset
        let offset = (self.active_particle_count as usize) * std::mem::size_of::<Particle>();
        self.queue.write_buffer(
            &self.particle_buffers[0],
            offset as u64,
            bytemuck::bytes_of(&particle),
        );
        self.queue.write_buffer(
            &self.particle_buffers[1],
            offset as u64,
            bytemuck::bytes_of(&particle),
        );

        self.active_particle_count += 1;
        self.particle_positions.push(world_pos);
    }

    /// Finds the nearest particle hit by a ray from the given screen coordinates.
    /// Returns the particle index, or None if no particle was hit.
    fn pick_particle(&self, screen_x: f32, screen_y: f32) -> Option<u32> {
        let (width, height) = self.size;
        let (ray_origin, ray_dir) = screen_to_world_ray(
            screen_x, screen_y, width, height, &self.camera, &self.projection,
        );

        let pick_radius = PARTICLE_SIZE * 2.0;
        let mut best_dist = f32::MAX;
        let mut best_idx: Option<u32> = None;

        for (i, pos) in self.particle_positions.iter().enumerate() {
            if let Some(dist) = ray_sphere_intersection(ray_origin, ray_dir, *pos, pick_radius) {
                if dist < best_dist {
                    best_dist = dist;
                    best_idx = Some(i as u32);
                }
            }
        }

        best_idx
    }

    /// Deletes a particle by swapping with the last particle (O(1)).
    fn delete_particle(&mut self, idx: u32) {
        if self.active_particle_count == 0 || idx >= self.active_particle_count {
            return;
        }

        let last_idx = self.active_particle_count - 1;

        if idx < last_idx {
            // Copy last particle to deleted slot in both GPU buffers
            let particle_size = std::mem::size_of::<Particle>() as u64;
            let src_offset = last_idx as u64 * particle_size;
            let dst_offset = idx as u64 * particle_size;

            let mut encoder = self.device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor {
                    label: Some("Delete Particle Encoder"),
                },
            );

            encoder.copy_buffer_to_buffer(
                &self.particle_buffers[0], src_offset,
                &self.particle_buffers[0], dst_offset,
                particle_size,
            );
            encoder.copy_buffer_to_buffer(
                &self.particle_buffers[1], src_offset,
                &self.particle_buffers[1], dst_offset,
                particle_size,
            );

            self.queue.submit(std::iter::once(encoder.finish()));

            // Update CPU positions: swap last into deleted slot
            self.particle_positions[idx as usize] = self.particle_positions[last_idx as usize];
        }

        self.particle_positions.pop();
        self.active_particle_count -= 1;
    }

    /// Directly sets a particle's position (used when paused + dragging).
    fn update_particle_position_direct(&mut self, idx: u32, pos: Vec3) {
        if idx >= self.active_particle_count {
            return;
        }

        self.particle_positions[idx as usize] = pos;

        // Create particle data with new position, zero velocity
        let particle = Particle {
            position: [pos.x, pos.y, pos.z],
            size: PARTICLE_SIZE,
            velocity: [0.0, 0.0, 0.0],
            _padding: 0.0,
        };

        let offset = (idx as usize) * std::mem::size_of::<Particle>();
        self.queue.write_buffer(
            &self.particle_buffers[0],
            offset as u64,
            bytemuck::bytes_of(&particle),
        );
        self.queue.write_buffer(
            &self.particle_buffers[1],
            offset as u64,
            bytemuck::bytes_of(&particle),
        );
    }

    /// Renders a single frame.
    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // Calculate delta time
        let now = web_sys::window()
            .expect("No window")
            .performance()
            .expect("No performance")
            .now();
        let dt = match self.last_frame_time {
            Some(last) => ((now - last) / 1000.0) as f32, // ms to seconds
            None => 1.0 / 60.0, // Assume 60fps for first frame
        };
        self.last_frame_time = Some(now);
        // Clamp dt to avoid instability on lag spikes
        let dt = dt.min(1.0 / 30.0);

        // Apply time scale and pause state
        let effective_dt = if self.paused { 0.0 } else { dt * self.time_scale };

        // Update simulation parameters with user-controllable values and drag/hover state
        let (drag_idx, drag_str, drag_tgt) = match (self.dragged_particle_idx, self.drag_target) {
            (Some(idx), Some(pos)) => (idx as i32, DRAG_STRENGTH, [pos.x, pos.y, pos.z]),
            _ => (-1_i32, 0.0, [0.0; 3]),
        };
        let hover_idx = self.hovered_particle_idx.map(|i| i as i32).unwrap_or(-1);

        let sim_params = SimParams {
            delta_time: effective_dt,
            gravitational_constant: self.gravity,
            damping: DAMPING,
            particle_count: self.active_particle_count,
            softening_sq: SOFTENING_SQ,
            drag_strength: drag_str,
            dragged_particle_idx: drag_idx,
            hovered_particle_idx: hover_idx,
            drag_target: drag_tgt,
            restitution: RESTITUTION,
            floor_y: FLOOR_Y,
            cell_size: CELL_SIZE,
            num_cells: NUM_CELLS,
            _padding: 0.0,
        };
        self.queue.write_buffer(
            &self.sim_params_buffer,
            0,
            bytemuck::cast_slice(&[sim_params]),
        );

        // Update camera uniform buffer
        self.camera_uniform.update(&self.camera, &self.projection);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Determine which buffers to use for ping-pong
        let read_idx = (self.frame_index % 2) as usize;
        let write_idx = ((self.frame_index + 1) % 2) as usize;

        // Compute passes: spatial hash build + physics (skip when paused)
        if !self.paused {
            let particle_workgroups = (self.active_particle_count + 255) / 256;
            let hash_workgroups = (NUM_CELLS + 255) / 256;

            // Pass 1: Clear spatial hash
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Hash Clear Pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.hash_clear_pipeline);
                pass.set_bind_group(0, &self.compute_bind_groups[read_idx], &[]);
                pass.set_bind_group(1, &self.hash_bind_group, &[]);
                pass.dispatch_workgroups(hash_workgroups, 1, 1);
            }

            // Pass 2: Insert particles into hash
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Hash Insert Pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.hash_insert_pipeline);
                pass.set_bind_group(0, &self.compute_bind_groups[read_idx], &[]);
                pass.set_bind_group(1, &self.hash_bind_group, &[]);
                pass.dispatch_workgroups(particle_workgroups, 1, 1);
            }

            // Pass 3: N-body gravity + hash-based collision + integration
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Physics Compute Pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.compute_pipeline);
                pass.set_bind_group(0, &self.compute_bind_groups[read_idx], &[]);
                pass.set_bind_group(1, &self.hash_bind_group, &[]);
                pass.dispatch_workgroups(particle_workgroups, 1, 1);
            }
        }
        // Implicit memory barrier here - wgpu ensures compute writes complete before render reads

        // Render pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // Draw lines (coordinate axes)
            render_pass.set_pipeline(&self.line_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.line_vertex_buffer.slice(..));
            render_pass.draw(0..self.line_vertex_count, 0..1);

            // Draw particles (instanced quads)
            // When paused, read from read_idx (no compute ran); otherwise read from write_idx
            let particle_buffer_idx = if self.paused { read_idx } else { write_idx };
            render_pass.set_pipeline(&self.point_pipeline);
            render_pass.set_bind_group(0, &self.point_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.particle_buffers[particle_buffer_idx].slice(..));
            render_pass.set_index_buffer(self.quad_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..6, 0, 0..self.active_particle_count);

            // Draw spheres (instanced icosphere)
            render_pass.set_pipeline(&self.sphere_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.sphere_vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.sphere_instance_buffer.slice(..));
            render_pass.set_index_buffer(self.sphere_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.sphere_index_count, 0, 0..self.sphere_instance_count);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        // Increment frame index for next frame's ping-pong (only when not paused)
        if !self.paused {
            self.frame_index = self.frame_index.wrapping_add(1);
        }

        Ok(())
    }

    /// Update camera from input deltas.
    fn update_camera(&mut self, rotate: (f32, f32), zoom: f32) {
        const ROTATE_SPEED: f32 = 0.005;
        const ZOOM_SPEED: f32 = 0.5;

        self.camera.rotate_azimuth(-rotate.0 * ROTATE_SPEED);
        self.camera.rotate_polar(-rotate.1 * ROTATE_SPEED);
        self.camera.zoom(zoom * ZOOM_SPEED);
    }

    /// Resets the simulation, optionally with a new particle count.
    fn reset_simulation(&mut self, count: Option<u32>) {
        let particles = if let Some(c) = count {
            let c = c.min(self.max_particles);
            let new_particles = create_initial_particles(c);
            self.initial_particles = new_particles.clone();
            self.initial_particle_count = c;
            new_particles
        } else {
            self.initial_particles.clone()
        };

        // Write particles to both buffers
        self.queue.write_buffer(
            &self.particle_buffers[0],
            0,
            bytemuck::cast_slice(&particles),
        );
        self.queue.write_buffer(
            &self.particle_buffers[1],
            0,
            bytemuck::cast_slice(&particles),
        );

        // Reset particle count and frame index
        self.active_particle_count = self.initial_particle_count;
        self.frame_index = 0;

        // Reset CPU positions
        self.particle_positions = particles
            .iter()
            .map(|p| Vec3::new(p.position[0], p.position[1], p.position[2]))
            .collect();

        // Clear interaction state
        self.hovered_particle_idx = None;
        self.dragged_particle_idx = None;
        self.drag_target = None;
    }
}

// Thread-local storage for the State
thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Helper to get canvas element by ID.
fn get_canvas(canvas_id: &str) -> web_sys::HtmlCanvasElement {
    let window = web_sys::window().expect("No global window exists");
    let document = window.document().expect("No document on window");
    document
        .get_element_by_id(canvas_id)
        .expect("Canvas element not found")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("Element is not a canvas")
}

/// Called automatically when WASM loads.
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    console_log!("WASM initialized");
}

/// Initializes WebGPU with the given canvas ID.
/// Called from JavaScript after WebGPU support is confirmed.
#[wasm_bindgen]
pub async fn init_webgpu(canvas_id: &str) -> Result<(), JsValue> {
    let canvas = get_canvas(canvas_id);

    // Set up mouse input handlers
    INPUT_STATE.with(|input| {
        let input_rc = input.clone();
        setup_mouse_events(&canvas, input_rc.clone()).expect("Failed to setup mouse events");
        setup_wheel_event(&canvas, input_rc).expect("Failed to setup wheel event");
    });

    let state = State::new(canvas).await;

    STATE.with(|s| {
        *s.borrow_mut() = Some(state);
    });

    console_log!("WebGPU initialized successfully");
    Ok(())
}

/// Renders a single frame.
/// Called from JavaScript via requestAnimationFrame.
#[wasm_bindgen]
pub fn render() -> Result<(), JsValue> {
    // Get all input state
    let (rotate, zoom, pending_spawn, pending_pick, pending_delete,
         current_mouse_pos, mut dragging_idx) = INPUT_STATE.with(|input| {
        let mut input = input.borrow_mut();
        let deltas = input.take_deltas();
        let spawn = input.take_pending_spawn();
        let pick = input.pending_pick.take();
        let delete = input.pending_delete.take();
        let mouse_pos = input.current_mouse_screen_pos;
        let dragging = input.dragging_particle_idx;
        (deltas.0, deltas.1, spawn, pick, delete, mouse_pos, dragging)
    });

    STATE.with(|s| {
        let mut state_ref = s.borrow_mut();
        if let Some(state) = state_ref.as_mut() {
            let mut suppress_spawn = false;

            // Process pending pick test (from mousedown)
            if let Some((px, py)) = pending_pick {
                if let Some(idx) = state.pick_particle(px, py) {
                    // Clicked on a particle - start dragging
                    dragging_idx = Some(idx);
                    suppress_spawn = true;
                    INPUT_STATE.with(|input| {
                        let mut input = input.borrow_mut();
                        input.dragging_particle_idx = Some(idx);
                        input.pending_spawn = None;
                    });
                }
            }

            // Process pending delete (middle-click)
            if let Some((dx, dy)) = pending_delete {
                if let Some(idx) = state.pick_particle(dx, dy) {
                    // If we're dragging this particle, stop
                    if dragging_idx == Some(idx) {
                        dragging_idx = None;
                        INPUT_STATE.with(|input| {
                            input.borrow_mut().dragging_particle_idx = None;
                        });
                    }
                    state.delete_particle(idx);
                }
            }

            if let Some(idx) = dragging_idx {
                // Particle drag mode - don't move camera
                if let Some((mx, my)) = current_mouse_pos {
                    let (width, height) = state.size;
                    let (ray_origin, ray_dir) = screen_to_world_ray(
                        mx, my, width, height, &state.camera, &state.projection,
                    );
                    let plane_normal = (state.camera.eye() - state.camera.target).normalize();
                    if let Some(world_pos) = ray_plane_intersection(
                        ray_origin, ray_dir, state.camera.target, plane_normal,
                    ) {
                        state.drag_target = Some(world_pos);
                        state.dragged_particle_idx = Some(idx);

                        // If paused, directly update particle position
                        if state.paused {
                            state.update_particle_position_direct(idx, world_pos);
                        }
                    }
                }
            } else {
                // Not particle dragging - update camera, hover, and handle spawns
                state.drag_target = None;
                state.dragged_particle_idx = None;

                // Update camera from input
                state.update_camera(rotate, zoom);

                // Update hover detection
                if let Some((mx, my)) = current_mouse_pos {
                    state.hovered_particle_idx = state.pick_particle(mx, my);
                } else {
                    state.hovered_particle_idx = None;
                }

                // Handle click-to-spawn (only if we didn't click on a particle)
                if !suppress_spawn {
                    if let Some((screen_x, screen_y)) = pending_spawn {
                        let (width, height) = state.size;
                        let (ray_origin, ray_dir) = screen_to_world_ray(
                            screen_x, screen_y, width, height,
                            &state.camera, &state.projection,
                        );
                        let plane_normal =
                            (state.camera.eye() - state.camera.target).normalize();
                        if let Some(world_pos) = ray_plane_intersection(
                            ray_origin, ray_dir, state.camera.target, plane_normal,
                        ) {
                            state.spawn_particle(world_pos);
                        }
                    }
                }
            }

            match state.render() {
                Ok(()) => {}
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    let (width, height) = state.size;
                    state.resize(width, height);
                }
                Err(wgpu::SurfaceError::OutOfMemory) => {
                    console_log!("OutOfMemory error during render");
                    return Err(JsValue::from_str("Out of GPU memory"));
                }
                Err(e) => {
                    console_log!("Render error: {:?}", e);
                }
            }
        }
        Ok(())
    })
}

/// Resizes the rendering surface.
/// Called from JavaScript when the canvas size changes.
#[wasm_bindgen]
pub fn resize(width: u32, height: u32) {
    STATE.with(|s| {
        let mut state_ref = s.borrow_mut();
        if let Some(state) = state_ref.as_mut() {
            state.resize(width, height);
        }
    });
}

/// Resets the camera to its default position.
/// Called from JavaScript when user presses reset key.
#[wasm_bindgen]
pub fn reset_camera() {
    STATE.with(|s| {
        let mut state_ref = s.borrow_mut();
        if let Some(state) = state_ref.as_mut() {
            state.camera.reset();
        }
    });
}

/// Sets the simulation pause state.
/// When paused, particles freeze but camera can still orbit.
#[wasm_bindgen]
pub fn set_paused(paused: bool) {
    STATE.with(|s| {
        let mut state_ref = s.borrow_mut();
        if let Some(state) = state_ref.as_mut() {
            state.paused = paused;
        }
    });
}

/// Returns the current pause state.
#[wasm_bindgen]
pub fn get_paused() -> bool {
    STATE.with(|s| {
        let state_ref = s.borrow();
        state_ref.as_ref().map(|state| state.paused).unwrap_or(false)
    })
}

/// Resets the simulation to its initial state.
/// Preserves pause state and camera position.
#[wasm_bindgen]
pub fn reset_simulation() {
    // Clear input drag state
    INPUT_STATE.with(|input| {
        let mut input = input.borrow_mut();
        input.dragging_particle_idx = None;
        input.pending_pick = None;
        input.pending_delete = None;
    });

    STATE.with(|s| {
        let mut state_ref = s.borrow_mut();
        if let Some(state) = state_ref.as_mut() {
            state.reset_simulation(None);
        }
    });
}

/// Resets the simulation with a new particle count.
/// Count is clamped to [1, MAX_PARTICLES].
#[wasm_bindgen]
pub fn reset_simulation_with_count(count: u32) {
    INPUT_STATE.with(|input| {
        let mut input = input.borrow_mut();
        input.dragging_particle_idx = None;
        input.pending_pick = None;
        input.pending_delete = None;
    });

    STATE.with(|s| {
        let mut state_ref = s.borrow_mut();
        if let Some(state) = state_ref.as_mut() {
            state.reset_simulation(Some(count.max(1)));
        }
    });
}

/// Sets the gravitational constant.
/// Value is clamped to [-2.0, 2.0].
#[wasm_bindgen]
pub fn set_gravity(value: f32) {
    STATE.with(|s| {
        let mut state_ref = s.borrow_mut();
        if let Some(state) = state_ref.as_mut() {
            state.gravity = value.clamp(-2.0, 2.0);
        }
    });
}

/// Sets the simulation time scale.
/// Value is clamped to [0.0, 3.0].
#[wasm_bindgen]
pub fn set_time_scale(value: f32) {
    STATE.with(|s| {
        let mut state_ref = s.borrow_mut();
        if let Some(state) = state_ref.as_mut() {
            state.time_scale = value.clamp(0.0, 3.0);
        }
    });
}

/// Returns the current number of active particles.
#[wasm_bindgen]
pub fn get_particle_count() -> u32 {
    STATE.with(|s| {
        let state_ref = s.borrow();
        state_ref.as_ref().map(|state| state.active_particle_count).unwrap_or(0)
    })
}
