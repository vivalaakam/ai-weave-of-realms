//! `TileHighlight` GDExtension node — draws a diamond outline on the hovered tile.
//!
//! Place as a `Node2D` child in the scene tree (`type="TileHighlight"`).
//! The node redraws only when the hovered tile changes.

use godot::classes::{INode2D, Node2D};
use godot::prelude::*;

use crate::coords::{tile_to_world, world_to_tile, TILE_H, TILE_W};

// ─── TileHighlight ────────────────────────────────────────────────────────────

#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct TileHighlight {
    base: Base<Node2D>,
    hovered_tile: Vector2i,
}

#[godot_api]
impl INode2D for TileHighlight {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            hovered_tile: Vector2i::new(-1, -1),
        }
    }

    fn process(&mut self, _delta: f64) {
        let new_tile = self.compute_hovered_tile();
        if new_tile != self.hovered_tile {
            self.hovered_tile = new_tile;
            self.base_mut().queue_redraw();
        }
    }

    fn draw(&mut self) {
        let tile = self.hovered_tile;
        if tile.x < 0 || tile.y < 0 {
            return;
        }

        let origin = tile_to_world(tile.x, tile.y);
        let hw = TILE_W * 0.5; // 32
        let hh = TILE_H * 0.5; // 16

        // Diamond: top → right → bottom → left → top
        let mut pts = PackedVector2Array::new();
        pts.push(origin + Vector2::new(0.0,  -hh));
        pts.push(origin + Vector2::new(hw,   0.0));
        pts.push(origin + Vector2::new(0.0,   hh));
        pts.push(origin + Vector2::new(-hw,  0.0));
        pts.push(origin + Vector2::new(0.0,  -hh)); // close

        let color = Color::from_rgba(1.0, 1.0, 1.0, 0.45);
        self.base_mut().draw_polyline(&pts, color);
    }
}

#[godot_api]
impl TileHighlight {
    /// Returns the tile coordinate currently under the mouse cursor.
    #[func]
    pub fn get_hovered_tile(&self) -> Vector2i {
        self.hovered_tile
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn compute_hovered_tile(&self) -> Vector2i {
        let Some(vp) = self.base().get_viewport() else {
            return Vector2i::new(-1, -1);
        };
        let Some(camera) = vp.get_camera_2d() else {
            return Vector2i::new(-1, -1);
        };
        let center = camera.get_screen_center_position();
        let mouse  = vp.get_mouse_position();
        let half   = vp.get_visible_rect().size * 0.5;
        let world  = center + mouse - half;
        let tile   = world_to_tile(world);
        if tile.x >= 0 && tile.y >= 0 { tile } else { Vector2i::new(-1, -1) }
    }
}
