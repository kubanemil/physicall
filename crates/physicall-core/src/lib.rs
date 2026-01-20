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
// Input Handling
// ============================================================================

/// Tracks mouse input state between events.
struct InputState {
    is_dragging: bool,
    last_x: i32,
    last_y: i32,
    rotate_delta: (f32, f32),
    zoom_delta: f32,
}

impl InputState {
    fn new() -> Self {
        Self {
            is_dragging: false,
            last_x: 0,
            last_y: 0,
            rotate_delta: (0.0, 0.0),
            zoom_delta: 0.0,
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
}

// Global input state accessible from event closures
thread_local! {
    static INPUT_STATE: Rc<RefCell<InputState>> = Rc::new(RefCell::new(InputState::new()));
}

/// Set up mouse event listeners for camera control.
fn setup_mouse_events(
    canvas: &HtmlCanvasElement,
    state: Rc<RefCell<InputState>>,
) -> Result<(), JsValue> {
    // Mousedown - start dragging
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: MouseEvent| {
            if event.button() == 0 {
                // Left button
                let mut s = state.borrow_mut();
                s.is_dragging = true;
                s.last_x = event.client_x();
                s.last_y = event.client_y();
            }
        });
        canvas.add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Mousemove - accumulate rotation delta while dragging
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: MouseEvent| {
            let mut s = state.borrow_mut();
            if s.is_dragging {
                let dx = event.client_x() - s.last_x;
                let dy = event.client_y() - s.last_y;
                s.rotate_delta.0 += dx as f32;
                s.rotate_delta.1 += dy as f32;
                s.last_x = event.client_x();
                s.last_y = event.client_y();
            }
        });
        canvas.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Mouseup on window - stop dragging (catches releases outside canvas)
    {
        let state = state.clone();
        let window = web_sys::window().expect("No window");
        let closure = Closure::<dyn FnMut(_)>::new(move |_: MouseEvent| {
            state.borrow_mut().is_dragging = false;
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

/// Per-instance data for point rendering.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct PointInstance {
    position: [f32; 3],
    size: f32,
    color: [f32; 3],
    _padding: f32,  // Align to 32 bytes
}

impl PointInstance {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        1 => Float32x3,  // world_pos @ location(1)
        2 => Float32,    // size @ location(2)
        3 => Float32x3,  // color @ location(3)
    ];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PointInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,  // Per-instance, not per-vertex
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Creates demo point instances in a spherical distribution.
fn create_demo_points() -> Vec<PointInstance> {
    let mut points = Vec::new();
    let num_points = 200;

    // Use golden ratio spiral for even distribution on sphere
    let golden_ratio = (1.0 + 5.0_f32.sqrt()) / 2.0;
    let angle_increment = std::f32::consts::PI * 2.0 * golden_ratio;

    for i in 0..num_points {
        let t = i as f32 / (num_points - 1) as f32;
        let inclination = (1.0 - 2.0 * t).acos();
        let azimuth = angle_increment * i as f32;

        // Spherical to Cartesian, radius varies for visual interest
        let base_radius = 1.2;
        let radius_variation = 0.3 * ((i as f32 * 0.1).sin());
        let r = base_radius + radius_variation;

        let x = r * inclination.sin() * azimuth.cos();
        let y = r * inclination.sin() * azimuth.sin();
        let z = r * inclination.cos();

        // Color based on position (normalized to 0-1)
        let color_r = (x / 2.0 + 0.5).clamp(0.2, 1.0);
        let color_g = (y / 2.0 + 0.5).clamp(0.2, 1.0);
        let color_b = (z / 2.0 + 0.5).clamp(0.2, 1.0);

        // Vary size
        let size = 0.015 + 0.015 * ((i as f32 * 0.2).sin().abs());

        points.push(PointInstance {
            position: [x, y, z],
            size,
            color: [color_r, color_g, color_b],
            _padding: 0.0,
        });
    }

    points
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
    // Point rendering (instanced quads)
    point_pipeline: wgpu::RenderPipeline,
    quad_vertex_buffer: wgpu::Buffer,
    quad_index_buffer: wgpu::Buffer,
    point_instance_buffer: wgpu::Buffer,
    point_instance_count: u32,
    // Sphere rendering (instanced icosphere)
    sphere_pipeline: wgpu::RenderPipeline,
    sphere_vertex_buffer: wgpu::Buffer,
    sphere_index_buffer: wgpu::Buffer,
    sphere_index_count: u32,
    sphere_instance_buffer: wgpu::Buffer,
    sphere_instance_count: u32,
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
        // Point Pipeline Setup (Instanced Quads)
        // ====================================================================

        // Load point shader
        let point_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Point Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../../shaders/point.wgsl").into()),
        });

        // Create point render pipeline with two vertex buffers (quad + instance)
        let point_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Point Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &point_shader,
                entry_point: Some("vs_main"),
                buffers: &[QuadVertex::desc(), PointInstance::desc()],
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

        // Create point instance buffer with demo data
        let demo_points = create_demo_points();
        let point_instance_count = demo_points.len() as u32;

        let point_instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Point Instance Buffer"),
            contents: bytemuck::cast_slice(&demo_points),
            usage: wgpu::BufferUsages::VERTEX,
        });

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
            point_pipeline,
            quad_vertex_buffer,
            quad_index_buffer,
            point_instance_buffer,
            point_instance_count,
            sphere_pipeline,
            sphere_vertex_buffer,
            sphere_index_buffer,
            sphere_index_count,
            sphere_instance_buffer,
            sphere_instance_count,
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

    /// Renders a single frame.
    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
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

            // Draw points (instanced quads)
            render_pass.set_pipeline(&self.point_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.point_instance_buffer.slice(..));
            render_pass.set_index_buffer(self.quad_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..6, 0, 0..self.point_instance_count);

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
    // Get input deltas and apply to camera
    let (rotate, zoom) = INPUT_STATE.with(|input| input.borrow_mut().take_deltas());

    STATE.with(|s| {
        let mut state_ref = s.borrow_mut();
        if let Some(state) = state_ref.as_mut() {
            // Update camera from input
            state.update_camera(rotate, zoom);

            match state.render() {
                Ok(()) => {}
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    // Reconfigure surface on lost/outdated
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
