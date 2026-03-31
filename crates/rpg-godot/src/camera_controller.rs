//! `CameraController` GDExtension node — isometric camera with pan and zoom.
//!
//! Attach as a `Camera2D` child in the scene tree (`type="CameraController"`).
//!
//! ## Controls
//! | Action | Gesture |
//! |--------|---------|
//! | Pan | Middle-mouse drag **or** arrow keys |
//! | Zoom | Scroll wheel or `-` / `=` |

use godot::classes::{Camera2D, ICamera2D, Input, InputEvent, InputEventKey, InputEventMouseButton,
                     InputEventMouseMotion};
use godot::global::{Key, MouseButton};
use godot::prelude::*;

// ── Constants ─────────────────────────────────────────────────────────────────

const PAN_SPEED: f32 = 400.0; // pixels per second (keyboard)
const ZOOM_STEP: f32 = 0.1;
const ZOOM_MIN:  f32 = 0.25;
const ZOOM_MAX:  f32 = 3.0;
const MAX_VISIBLE_WIDTH: f32 = 1920.0;
const MAX_VISIBLE_HEIGHT: f32 = 1080.0;

// ─── CameraController ─────────────────────────────────────────────────────────

#[derive(GodotClass)]
#[class(base=Camera2D)]
pub struct CameraController {
    base: Base<Camera2D>,
    dragging: bool,
    manual_zoom: f32,
    last_viewport_size: Vector2,
}

#[godot_api]
impl ICamera2D for CameraController {
    fn init(base: Base<Camera2D>) -> Self {
        Self {
            base,
            dragging: false,
            manual_zoom: 1.0,
            last_viewport_size: Vector2::ZERO,
        }
    }

    fn ready(&mut self) {
        self.refresh_zoom();
    }

    fn process(&mut self, delta: f64) {
        self.refresh_zoom_if_needed();

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

    fn input(&mut self, event: Gd<InputEvent>) {
        if let Ok(mb) = event.clone().try_cast::<InputEventMouseButton>() {
            match mb.get_button_index() {
                MouseButton::MIDDLE => {
                    self.dragging = mb.is_pressed();
                }
                MouseButton::WHEEL_UP => {
                    self.apply_zoom(-ZOOM_STEP);
                }
                MouseButton::WHEEL_DOWN => {
                    self.apply_zoom(ZOOM_STEP);
                }
                _ => {}
            }
        }

        if let Ok(key) = event.clone().try_cast::<InputEventKey>() {
            if key.is_pressed() && !key.is_echo() {
                let physical = key.get_physical_keycode();
                let logical = key.get_keycode();

                if physical == Key::MINUS || logical == Key::MINUS {
                    self.apply_zoom(ZOOM_STEP);
                } else if physical == Key::EQUAL || logical == Key::EQUAL {
                    self.apply_zoom(-ZOOM_STEP);
                }
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
        self.manual_zoom = (self.manual_zoom + delta).clamp(ZOOM_MIN, ZOOM_MAX);
        self.refresh_zoom();
    }

    fn refresh_zoom_if_needed(&mut self) {
        let Some(viewport) = self.base().get_viewport() else {
            return;
        };
        let size = viewport.get_visible_rect().size;
        if size != self.last_viewport_size {
            self.refresh_zoom();
        }
    }

    fn refresh_zoom(&mut self) {
        let overflow_scale = self.viewport_overflow_scale();
        let zoom = self.manual_zoom * overflow_scale;
        self.base_mut().set_zoom(Vector2::new(zoom, zoom));
    }

    fn viewport_overflow_scale(&mut self) -> f32 {
        let Some(viewport) = self.base().get_viewport() else {
            return 1.0;
        };
        let size = viewport.get_visible_rect().size;
        self.last_viewport_size = size;

        let width_scale = (size.x / MAX_VISIBLE_WIDTH).max(1.0);
        let height_scale = (size.y / MAX_VISIBLE_HEIGHT).max(1.0);
        width_scale.max(height_scale)
    }
}
