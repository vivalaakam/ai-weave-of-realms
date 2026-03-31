//! Isometric coordinate conversion utilities shared across GDExtension nodes.

use godot::prelude::*;

/// Tile pixel width (matches tileset).
pub const TILE_W: f32 = 64.0;
/// Tile pixel height (matches tileset).
pub const TILE_H: f32 = 32.0;

/// Converts tile grid coordinates to isometric world position.
pub fn tile_to_world(tx: i32, ty: i32) -> Vector2 {
    Vector2::new(
        (tx - ty) as f32 * TILE_W * 0.5,
        (tx + ty) as f32 * TILE_H * 0.5,
    )
}

/// Converts an isometric world position to the nearest tile grid coordinate.
pub fn world_to_tile(pos: Vector2) -> Vector2i {
    let tx = (((pos.x / (TILE_W * 0.5)) + (pos.y / (TILE_H * 0.5))).round() * 0.5) as i32;
    let ty = (((pos.y / (TILE_H * 0.5)) - (pos.x / (TILE_W * 0.5))).round() * 0.5) as i32;
    Vector2i::new(tx, ty)
}
