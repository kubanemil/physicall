use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, MouseEvent, WheelEvent};

/// Threshold in pixels for distinguishing click from drag.
pub(crate) const CLICK_THRESHOLD: i32 = 5;

/// Tracks mouse input state between events.
pub(crate) struct InputState {
    pub(crate) is_dragging: bool,
    pub(crate) last_x: i32,
    pub(crate) last_y: i32,
    pub(crate) rotate_delta: (f32, f32),
    pub(crate) zoom_delta: f32,
    // Click detection for spawning
    pub(crate) mouse_down_pos: Option<(i32, i32)>,
    pub(crate) pending_spawn: Option<(f32, f32)>,
    pub(crate) total_drag_distance: i32,
    // Particle interaction
    pub(crate) pending_pick: Option<(f32, f32)>,
    pub(crate) dragging_particle_idx: Option<u32>,
    pub(crate) current_mouse_screen_pos: Option<(f32, f32)>,
    pub(crate) pending_delete: Option<(f32, f32)>,
}

impl InputState {
    pub(crate) fn new() -> Self {
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
    pub(crate) fn take_deltas(&mut self) -> ((f32, f32), f32) {
        let rotate = self.rotate_delta;
        let zoom = self.zoom_delta;
        self.rotate_delta = (0.0, 0.0);
        self.zoom_delta = 0.0;
        (rotate, zoom)
    }

    /// Returns and clears pending spawn position.
    pub(crate) fn take_pending_spawn(&mut self) -> Option<(f32, f32)> {
        self.pending_spawn.take()
    }
}

// Global input state accessible from event closures
thread_local! {
    pub(crate) static INPUT_STATE: Rc<RefCell<InputState>> = Rc::new(RefCell::new(InputState::new()));
}

/// Set up mouse event listeners for camera control, click-to-spawn, and particle interaction.
pub(crate) fn setup_mouse_events(
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
pub(crate) fn setup_wheel_event(
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
