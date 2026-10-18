//! WebAssembly bindings for the physicall demo. `App` owns the WebGPU device,
//! the running simulation and the renderer; the page forwards input to it and
//! calls `frame` once per animation frame.

mod camera;
mod catalog;
mod picking;
mod render;
mod sim;

use glam::{Vec2, Vec3, Vec4Swizzles};
use physicall_core::clock::FixedClock;
use physicall_core::scene::{self, Change, SceneKind, Settings};
use physicall_gpu::nbody::NBodyKernels;
use physicall_gpu::xpbd::XpbdKernels;
use wasm_bindgen::prelude::*;

use crate::camera::OrbitCamera;
use crate::render::{RenderUniforms, Renderer};
use crate::sim::{Kernels, Simulation, Snapshot};

const MAX_STEPS_PER_FRAME: u32 = 8;
const GRAB_PIXELS: f32 = 24.0;
const READBACK_EVERY: u64 = 4;

#[wasm_bindgen(start)]
fn start() {
    console_error_panic_hook::set_once();
}

/// The scenes and their parameters, as JSON.
#[wasm_bindgen]
pub fn scene_catalog() -> String {
    catalog::json()
}

/// Diagnostics from the latest GPU readback.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub kinetic: f64,
    pub potential: f64,
    pub total: f64,
    /// `(E - E0) / |E0|` since the scene was built.
    pub drift: f64,
    pub momentum: f64,
    pub particles: u32,
    pub steps: u32,
    pub time: f64,
}

enum Drag {
    Orbit,
    Pan,
    Grab {
        index: u32,
        plane: Vec3,
        target: Vec3,
    },
}

struct Pointer {
    last: Vec2,
    drag: Drag,
}

#[wasm_bindgen]
pub struct App {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    kernels: Kernels,
    settings: Settings,
    sim: Simulation,
    camera: OrbitCamera,
    clock: FixedClock,
    speed: f64,
    paused: bool,
    snapshot: Option<Snapshot>,
    initial_energy: Option<f64>,
    stats: Stats,
    pointer: Option<Pointer>,
    hovered: Option<u32>,
    time: f64,
    frames: u64,
}

fn js(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

#[cfg(target_arch = "wasm32")]
fn surface_target(
    canvas: web_sys::HtmlCanvasElement,
) -> Result<wgpu::SurfaceTarget<'static>, JsError> {
    Ok(wgpu::SurfaceTarget::Canvas(canvas))
}

#[cfg(not(target_arch = "wasm32"))]
fn surface_target(
    _canvas: web_sys::HtmlCanvasElement,
) -> Result<wgpu::SurfaceTarget<'static>, JsError> {
    Err(JsError::new("physicall-web draws to a browser canvas"))
}

fn frame_uniforms(
    camera: &OrbitCamera,
    size: Vec2,
    sim: &Simulation,
    highlight: Option<u32>,
) -> RenderUniforms {
    let camera_to_world = camera.view().inverse();
    RenderUniforms {
        view_proj: (camera.projection(size.x / size.y) * camera.view()).to_cols_array_2d(),
        right: camera_to_world.x_axis.xyz().extend(0.0).to_array(),
        up: camera_to_world.y_axis.xyz().extend(0.0).to_array(),
        eye: camera.eye().extend(1.0).to_array(),
        ..sim.uniforms(highlight)
    }
}

fn framing(kind: SceneKind) -> OrbitCamera {
    match kind {
        SceneKind::NBody => OrbitCamera::new(Vec3::ZERO, 5.0, 0.6, 0.45),
        SceneKind::Cloth => OrbitCamera::new(Vec3::new(0.0, 1.2, -0.4), 2.2, 0.75, 0.25),
        SceneKind::Rope => OrbitCamera::new(Vec3::new(-1.0, 1.1, 0.0), 5.2, 0.1, 0.12),
        SceneKind::Collisions => OrbitCamera::new(Vec3::new(0.0, 0.25, 0.0), 2.2, 0.6, 0.5),
    }
}

#[wasm_bindgen]
impl App {
    pub async fn create(canvas: web_sys::HtmlCanvasElement, scene: &str) -> Result<App, JsError> {
        let kind = SceneKind::from_id(scene).unwrap_or(SceneKind::Cloth);
        let (width, height) = (canvas.width().max(1), canvas.height().max(1));
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance
            .create_surface(surface_target(canvas)?)
            .map_err(js)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(js)?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("physicall"),
                ..Default::default()
            })
            .await
            .map_err(js)?;
        let config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| JsError::new("this GPU cannot draw to the canvas"))?;
        surface.configure(&device, &config);

        let renderer = Renderer::new(&device, config.format, width, height);
        let kernels = Kernels {
            xpbd: XpbdKernels::new(&device),
            nbody: NBodyKernels::new(&device),
        };
        let settings = Settings::new(kind);
        let sim = Simulation::new(&device, &queue, &kernels, &renderer, &settings.build());
        Ok(App {
            device,
            queue,
            surface,
            config,
            renderer,
            kernels,
            settings,
            sim,
            camera: framing(kind),
            clock: FixedClock::new(f64::from(scene::STEP), MAX_STEPS_PER_FRAME),
            speed: 1.0,
            paused: false,
            snapshot: None,
            initial_energy: None,
            stats: Stats::default(),
            pointer: None,
            hovered: None,
            time: 0.0,
            frames: 0,
        })
    }

    pub fn load_scene(&mut self, id: &str) -> Result<(), JsError> {
        let kind =
            SceneKind::from_id(id).ok_or_else(|| JsError::new(&format!("unknown scene {id}")))?;
        self.settings = Settings::new(kind);
        self.camera = framing(kind);
        self.rebuild();
        Ok(())
    }

    pub fn set_param(&mut self, key: &str, value: f32) -> Result<(), JsError> {
        match self.settings.set(key, value) {
            Some(Change::Live) => self.sim.apply(&self.queue, &self.settings),
            Some(Change::Rebuild) => self.rebuild(),
            None => return Err(JsError::new(&format!("{key} = {value} is not valid here"))),
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        self.rebuild();
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Simulated seconds per real second.
    pub fn set_speed(&mut self, speed: f64) {
        self.speed = speed.clamp(0.05, 4.0);
    }

    pub fn reset_camera(&mut self) {
        self.camera = framing(self.settings.kind());
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        if (width, height) == (self.config.width, self.config.height) {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.renderer
            .resize(&self.device, self.config.format, width, height);
    }

    /// Advances by `elapsed` real seconds in fixed steps, then draws.
    pub fn frame(&mut self, elapsed: f64) -> Result<(), JsError> {
        if let Some(snapshot) = self.sim.take_snapshot() {
            self.update_stats(&snapshot);
            self.snapshot = Some(snapshot);
        }
        let steps = if self.paused {
            0
        } else {
            self.clock.advance(elapsed * self.speed)
        };
        if let Some(Pointer {
            drag: Drag::Grab { index, target, .. },
            ..
        }) = &self.pointer
        {
            self.sim.place(&self.queue, *index, *target, 0.0);
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(JsError::new("the canvas surface failed validation"));
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.sim.encode(&mut encoder, steps);
        self.renderer.draw(
            &self.queue,
            &mut encoder,
            &view,
            &self.sim.drawable,
            &self.render_uniforms(),
        );
        if self.frames.is_multiple_of(READBACK_EVERY) {
            self.sim.record_readback(&mut encoder);
        }
        self.queue.submit([encoder.finish()]);
        self.sim.map_readback();
        self.queue.present(frame);

        self.time += f64::from(steps) * f64::from(scene::STEP);
        self.stats.steps = steps;
        self.frames += 1;
        Ok(())
    }

    /// Pointer coordinates are canvas pixels (device pixels, not CSS).
    pub fn pointer_down(&mut self, x: f32, y: f32, button: u32, shift: bool) {
        let at = Vec2::new(x, y);
        let drag = match (button, shift) {
            (0, false) => self.grab(at).unwrap_or(Drag::Orbit),
            _ => Drag::Pan,
        };
        self.hovered = None;
        self.pointer = Some(Pointer { last: at, drag });
    }

    pub fn pointer_move(&mut self, x: f32, y: f32) {
        let at = Vec2::new(x, y);
        let (origin, dir) = self.camera.ray(at, self.size());
        let eye = self.camera.eye();
        let height = self.size().y;
        let Some(pointer) = &mut self.pointer else {
            self.hovered = self.pick(at).map(|i| i as u32);
            return;
        };
        let delta = at - pointer.last;
        pointer.last = at;
        match &mut pointer.drag {
            Drag::Orbit => self.camera.orbit(delta, height),
            Drag::Pan => self.camera.pan(delta, height),
            Drag::Grab { plane, target, .. } => {
                let normal = (eye - *plane).normalize();
                if let Some(hit) = picking::ray_plane(origin, dir, *plane, normal) {
                    *target = hit;
                }
            }
        }
    }

    pub fn pointer_up(&mut self) {
        if let Some(Pointer {
            drag: Drag::Grab { index, target, .. },
            ..
        }) = self.pointer.take()
        {
            self.sim.place(
                &self.queue,
                index,
                target,
                self.sim.inv_mass[index as usize],
            );
        }
    }

    pub fn pointer_leave(&mut self) {
        self.hovered = None;
    }

    pub fn wheel(&mut self, delta: f32) {
        self.camera.zoom((delta * 0.001).exp());
    }
}

impl App {
    fn rebuild(&mut self) {
        let scene = self.settings.build();
        self.sim = Simulation::new(
            &self.device,
            &self.queue,
            &self.kernels,
            &self.renderer,
            &scene,
        );
        self.snapshot = None;
        self.initial_energy = None;
        self.stats = Stats::default();
        self.pointer = None;
        self.hovered = None;
        self.time = 0.0;
        self.clock.reset();
    }

    fn size(&self) -> Vec2 {
        Vec2::new(self.config.width as f32, self.config.height as f32)
    }

    fn view_proj(&self) -> glam::Mat4 {
        let size = self.size();
        self.camera.projection(size.x / size.y) * self.camera.view()
    }

    fn pick(&self, at: Vec2) -> Option<usize> {
        if !self.sim.can_grab() {
            return None;
        }
        let snapshot = self.snapshot.as_ref()?;
        let index = picking::pick(
            &snapshot.pos,
            self.view_proj(),
            at,
            self.size(),
            GRAB_PIXELS,
        )?;
        (self.sim.inv_mass[index] > 0.0).then_some(index)
    }

    fn grab(&self, at: Vec2) -> Option<Drag> {
        let index = self.pick(at)?;
        let p = self.snapshot.as_ref()?.pos[index];
        Some(Drag::Grab {
            index: index as u32,
            plane: p,
            target: p,
        })
    }

    fn highlight(&self) -> Option<u32> {
        match &self.pointer {
            Some(Pointer {
                drag: Drag::Grab { index, .. },
                ..
            }) => Some(*index),
            _ => self.hovered,
        }
    }

    fn render_uniforms(&self) -> RenderUniforms {
        frame_uniforms(&self.camera, self.size(), &self.sim, self.highlight())
    }

    fn update_stats(&mut self, snapshot: &Snapshot) {
        let d = self.sim.diagnostics(snapshot);
        let total = d.total();
        let e0 = *self.initial_energy.get_or_insert(total);
        self.stats = Stats {
            kinetic: d.kinetic,
            potential: d.potential,
            total,
            drift: if e0 == 0.0 {
                0.0
            } else {
                (total - e0) / e0.abs()
            },
            momentum: d.momentum.length(),
            particles: snapshot.pos.len() as u32,
            steps: self.stats.steps,
            time: self.time,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDTH: u32 = 640;
    const HEIGHT: u32 = 400;

    fn gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            assert!(
                std::env::var_os("PHYSICALL_REQUIRE_GPU").is_none(),
                "PHYSICALL_REQUIRE_GPU is set but no GPU adapter was found"
            );
            eprintln!(
                "skipping render test: no GPU adapter; set PHYSICALL_REQUIRE_GPU=1 to fail instead"
            );
            return None;
        };
        pollster::block_on(adapter.request_device(&Default::default())).ok()
    }

    /// Renders `frames` frames of a scene offscreen and returns RGBA pixels
    /// plus the diagnostics from the last readback.
    pub(crate) fn render(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        settings: &Settings,
        frames: u32,
    ) -> (Vec<u8>, physicall_core::diagnostics::Diagnostics) {
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let renderer = Renderer::new(device, format, WIDTH, HEIGHT);
        let kernels = Kernels {
            xpbd: XpbdKernels::new(device),
            nbody: NBodyKernels::new(device),
        };
        let sim = Simulation::new(device, queue, &kernels, &renderer, &settings.build());
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("target"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let pixels = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pixels"),
            size: u64::from(4 * WIDTH * HEIGHT),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let camera = framing(settings.kind());
        let size = Vec2::new(WIDTH as f32, HEIGHT as f32);
        let mut snapshot = None;
        for frame in 0..frames {
            let mut encoder = device.create_command_encoder(&Default::default());
            sim.encode(&mut encoder, 1);
            renderer.draw(
                queue,
                &mut encoder,
                &view,
                &sim.drawable,
                &frame_uniforms(&camera, size, &sim, None),
            );
            if frame + 1 == frames {
                sim.record_readback(&mut encoder);
                encoder.copy_texture_to_buffer(
                    target.as_image_copy(),
                    wgpu::TexelCopyBufferInfo {
                        buffer: &pixels,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * WIDTH),
                            rows_per_image: None,
                        },
                    },
                    target.size(),
                );
            }
            queue.submit([encoder.finish()]);
            sim.map_readback();
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
            snapshot = sim.take_snapshot().or(snapshot);
        }
        let snapshot =
            snapshot.unwrap_or_else(|| panic!("{} produced no readback", settings.kind().id()));
        let rgba = physicall_gpu::readback::read_buffers(device, queue, &[&pixels]);
        (rgba, sim.diagnostics(&snapshot))
    }

    #[test]
    fn every_scene_simulates_and_draws_without_gpu_errors() {
        let Some((device, queue)) = gpu() else { return };
        for kind in SceneKind::ALL {
            let mut settings = Settings::new(kind);
            for (key, value) in [("bodies", 1024.0), ("count", 2048.0), ("resolution", 24.0)] {
                settings.set(key, value);
            }
            let (rgba, diagnostics) = render(&device, &queue, &settings, 20);
            let background = rgba[..4].to_vec();
            let drawn = rgba
                .chunks(4)
                .filter(|px| *px != background.as_slice())
                .count();
            assert!(drawn > 1000, "{} drew only {drawn} pixels", kind.id());
            assert!(
                diagnostics.total().is_finite() && diagnostics.momentum.is_finite(),
                "{} diagnostics: {diagnostics:?}",
                kind.id()
            );
        }
    }
}
