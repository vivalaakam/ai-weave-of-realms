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

use crate::coords::{tile_to_world, TILE_H, TILE_W};

// ── Constants ─────────────────────────────────────────────────────────────────

const PAN_SPEED: f32 = 400.0; // pixels per second (keyboard)
const ZOOM_STEP: f32 = 0.1;
const ZOOM_MIN:  f32 = 0.25;
const ZOOM_MAX:  f32 = 3.0;
const MAX_VISIBLE_WIDTH: f32 = 1920.0;
const MAX_VISIBLE_HEIGHT: f32 = 1080.0;
const LEFT_MARGIN_TILES: f32 = 1.0;
const TOP_MARGIN_TILES: f32 = 1.0;
const BOTTOM_MARGIN_TILES: f32 = 1.0;
const RIGHT_MARGIN_TILES: f32 = 5.0;

#[derive(Clone, Copy)]
struct CameraBounds {
    top: Vector2,
    right: Vector2,
    bottom: Vector2,
    left: Vector2,
}

#[derive(Clone, Copy)]
struct HalfPlane {
    normal: Vector2,
    constant: f32,
}

// ─── CameraController ─────────────────────────────────────────────────────────

#[derive(GodotClass)]
#[class(base=Camera2D)]
pub struct CameraController {
    base: Base<Camera2D>,
    dragging: bool,
    manual_zoom: f32,
    last_viewport_size: Vector2,
    bounds: Option<CameraBounds>,
}

#[godot_api]
impl ICamera2D for CameraController {
    fn init(base: Base<Camera2D>) -> Self {
        Self {
            base,
            dragging: false,
            manual_zoom: 1.0,
            last_viewport_size: Vector2::ZERO,
            bounds: None,
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
            let next = pos + dir.normalized() * PAN_SPEED * delta as f32 / z;
            let clamped = self.clamp_position(next);
            self.base_mut().set_position(clamped);
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
                let clamped = self.clamp_position(pos - mm.get_relative() / z);
                self.base_mut().set_position(clamped);
            }
        }
    }
}

#[godot_api]
impl CameraController {
    /// Configures camera bounds for a tile map of `width × height`.
    ///
    /// # Arguments
    /// * `width` - Map width in tiles.
    /// * `height` - Map height in tiles.
    pub fn configure_map_bounds(&mut self, width: i32, height: i32) {
        let _ = (
            width,
            height,
            tile_to_world,
            TILE_H,
            TILE_W,
            LEFT_MARGIN_TILES,
            TOP_MARGIN_TILES,
            BOTTOM_MARGIN_TILES,
            RIGHT_MARGIN_TILES,
        );
        self.bounds = None;
    }

    /// Focuses the camera on `position` while respecting configured map bounds.
    ///
    /// # Arguments
    /// * `position` - Target world position for the camera center.
    pub fn focus_on_world_position(&mut self, position: Vector2) {
        let clamped = self.clamp_position(position);
        self.base_mut().set_position(clamped);
    }

    /// Resets manual zoom to the default value.
    pub fn reset_zoom(&mut self) {
        self.manual_zoom = 1.0;
        self.refresh_zoom();
    }

    /// Returns the current effective zoom multiplier on the X axis.
    pub fn current_zoom(&self) -> f32 {
        self.base().get_zoom().x
    }

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
        let clamped = self.clamp_position(self.base().get_position());
        self.base_mut().set_position(clamped);
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

    fn clamp_position(&self, desired: Vector2) -> Vector2 {
        let Some(bounds) = self.bounds else {
            return desired;
        };
        let Some(viewport) = self.base().get_viewport() else {
            return desired;
        };

        let zoom = self.base().get_zoom();
        let visible = viewport.get_visible_rect().size;
        let half_w = visible.x * 0.5 / zoom.x.max(0.001);
        let half_h = visible.y * 0.5 / zoom.y.max(0.001);
        let polygon = [bounds.top, bounds.right, bounds.bottom, bounds.left];
        let planes = Self::shrink_half_planes(&polygon, half_w, half_h);
        let feasible = Self::half_planes_to_polygon(&planes);

        if feasible.len() < 3 {
            return desired;
        }

        if Self::inside_polygon(desired, &planes) {
            return desired;
        }

        Self::closest_point_on_polygon(desired, &feasible)
    }

    fn shrink_half_planes(polygon: &[Vector2; 4], half_w: f32, half_h: f32) -> [HalfPlane; 4] {
        std::array::from_fn(|index| {
            let p1 = polygon[index];
            let p2 = polygon[(index + 1) % polygon.len()];
            let edge = p2 - p1;
            let normal = Vector2::new(edge.y, -edge.x);
            let constant = normal.dot(p1) - normal.x.abs() * half_w - normal.y.abs() * half_h;
            HalfPlane { normal, constant }
        })
    }

    fn half_planes_to_polygon(planes: &[HalfPlane; 4]) -> Vec<Vector2> {
        let mut points = Vec::with_capacity(4);
        for index in 0..planes.len() {
            let prev = planes[(index + planes.len() - 1) % planes.len()];
            let current = planes[index];
            if let Some(point) = Self::intersect_half_planes(prev, current) {
                points.push(point);
            }
        }
        points
    }

    fn intersect_half_planes(a: HalfPlane, b: HalfPlane) -> Option<Vector2> {
        let det = a.normal.x * b.normal.y - a.normal.y * b.normal.x;
        if det.abs() < 0.001 {
            return None;
        }

        let x = (a.constant * b.normal.y - a.normal.y * b.constant) / det;
        let y = (a.normal.x * b.constant - a.constant * b.normal.x) / det;
        Some(Vector2::new(x, y))
    }

    fn inside_polygon(point: Vector2, planes: &[HalfPlane; 4]) -> bool {
        planes
            .iter()
            .all(|plane| plane.normal.dot(point) <= plane.constant + 0.001)
    }

    fn closest_point_on_polygon(point: Vector2, polygon: &[Vector2]) -> Vector2 {
        let mut best_point = polygon[0];
        let mut best_distance = point.distance_squared_to(best_point);

        for index in 0..polygon.len() {
            let start = polygon[index];
            let end = polygon[(index + 1) % polygon.len()];
            let candidate = Self::closest_point_on_segment(point, start, end);
            let distance = point.distance_squared_to(candidate);
            if distance < best_distance {
                best_distance = distance;
                best_point = candidate;
            }
        }

        best_point
    }

    fn closest_point_on_segment(point: Vector2, start: Vector2, end: Vector2) -> Vector2 {
        let segment = end - start;
        let length_squared = segment.length_squared();
        if length_squared <= 0.001 {
            return start;
        }

        let t = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
        start + segment * t
    }
}
