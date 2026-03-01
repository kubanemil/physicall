use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

mod camera;
mod input;
mod particle;
mod ray;
mod render;
mod sphere;
mod state;

use input::{setup_mouse_events, setup_wheel_event, INPUT_STATE};
use ray::{ray_plane_intersection, screen_to_world_ray};
use state::State;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
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

/// Returns the capped device pixel ratio (matching the JS cap of 2).
fn capped_dpr() -> f32 {
    let dpr = web_sys::window()
        .map(|w| w.device_pixel_ratio())
        .unwrap_or(1.0) as f32;
    dpr.min(2.0)
}

/// Scales CSS-pixel mouse coordinates to physical-pixel coordinates.
/// Mouse events report CSS pixels, but the canvas backing store uses physical pixels.
fn scale_to_physical(coords: (f32, f32)) -> (f32, f32) {
    let dpr = capped_dpr();
    (coords.0 * dpr, coords.1 * dpr)
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
            if let Some(pick) = pending_pick.map(scale_to_physical) {
                let (px, py) = pick;
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
            if let Some((dx, dy)) = pending_delete.map(scale_to_physical) {
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
                if let Some((mx, my)) = current_mouse_pos.map(scale_to_physical) {
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
                if let Some((mx, my)) = current_mouse_pos.map(scale_to_physical) {
                    state.hovered_particle_idx = state.pick_particle(mx, my);
                } else {
                    state.hovered_particle_idx = None;
                }

                // Handle click-to-spawn (only if we didn't click on a particle)
                if !suppress_spawn {
                    if let Some((screen_x, screen_y)) = pending_spawn.map(scale_to_physical) {
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

/// Sets the initial orbital speed for particles.
/// Takes effect on next reset. Value is clamped to [0.0, 2.0].
#[wasm_bindgen]
pub fn set_initial_speed(value: f32) {
    STATE.with(|s| {
        let mut state_ref = s.borrow_mut();
        if let Some(state) = state_ref.as_mut() {
            state.initial_speed = value.clamp(0.0, 2.0);
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
