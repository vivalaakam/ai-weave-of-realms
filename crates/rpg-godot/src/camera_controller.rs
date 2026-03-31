//! `CameraController` GDExtension node — isometric camera with pan and zoom.
//!
//! Attach as a `Camera2D` child in the scene tree (`type="CameraController"`).
//!
//! ## Controls
//! | Action | Gesture |
//! |--------|---------|
//! | Pan | Middle-mouse drag **or** arrow keys |
//! | Zoom | Scroll wheel |

use godot::classes::{Camera2D, ICamera2D, Input, InputEvent, InputEventMouseButton,
                     InputEventMouseMotion};
use godot::global::MouseButton;
use godot::prelude::*;

// ── Constants ─────────────────────────────────────────────────────────────────

const PAN_SPEED: f32 = 400.0; // pixels per second (keyboard)
const ZOOM_STEP: f32 = 0.1;
const ZOOM_MIN:  f32 = 0.25;
const ZOOM_MAX:  f32 = 3.0;

// ─── CameraController ─────────────────────────────────────────────────────────

#[derive(GodotClass)]
#[class(base=Camera2D)]
pub struct CameraController {
    base: Base<Camera2D>,
    dragging: bool,
}

#[godot_api]
impl ICamera2D for CameraController {
    fn init(base: Base<Camera2D>) -> Self {
        Self { base, dragging: false }
    }

    fn process(&mut self, delta: f64) {
        let input = Input::singleton();
        let mut dir = Vector2::ZERO;

        if input.is_action_pressed("ui_left")  { dir.x -= 1.0; }
        if input.is_action_pressed("ui_right") { dir.x += 1.0; }
        if input.is_action_pressed("ui_up")    { dir.y -= 1.0; }
        if input.is_action_pressed("ui_down")  { dir.y += 1.0; }

        if dir != Vector2::ZERO {
            let z = self.base().get_zoom().x;
            let pos = self.base().get_position();
            self.base_mut().set_position(
                pos + dir.normalized() * PAN_SPEED * delta as f32 / z,
            );
        }
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if let Ok(mb) = event.clone().try_cast::<InputEventMouseButton>() {
            match mb.get_button_index() {
                MouseButton::MIDDLE => {
                    self.dragging = mb.is_pressed();
                }
                MouseButton::WHEEL_UP => {
                    self.apply_zoom(ZOOM_STEP);
                }
                MouseButton::WHEEL_DOWN => {
                    self.apply_zoom(-ZOOM_STEP);
                }
                _ => {}
            }
        }

        if let Ok(mm) = event.try_cast::<InputEventMouseMotion>() {
            if self.dragging {
                let z   = self.base().get_zoom().x;
                let pos = self.base().get_position();
                self.base_mut().set_position(pos - mm.get_relative() / z);
            }
        }
    }
}

#[godot_api]
impl CameraController {
    fn apply_zoom(&mut self, delta: f32) {
        let z = (self.base().get_zoom().x + delta).clamp(ZOOM_MIN, ZOOM_MAX);
        self.base_mut().set_zoom(Vector2::new(z, z));
    }
}
