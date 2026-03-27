//! Helper for converting a [`GameMap`] into a Lua table representation.
//!
//! Both the evaluator and validator receive the map as a Lua table with the
//! following structure:
//! ```lua
//! {
//!   chunks_wide = <number>,
//!   chunks_tall = <number>,
//!   tiles       = { "grass", "water", ... }  -- flat, row-major, 1-indexed
//! }
//! ```
//! The `tiles` array has `chunks_wide * 32 * chunks_tall * 32` entries.

use mlua::{Lua, Table};
use rpg_engine::map::game_map::{GameMap, MapCoord};

/// Converts a [`GameMap`] into a Lua table that can be passed to evaluation
/// or validation scripts.
///
/// The `tiles` field is a flat 1-indexed Lua array of tile kind strings in
/// global row-major order (row by row, left to right, top to bottom).
///
/// # Errors
/// Returns an [`mlua::Error`] if table creation or field assignment fails.
pub fn game_map_to_lua_table<'lua>(lua: &'lua Lua, map: &GameMap) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("chunks_wide", map.chunks_wide())?;
    t.set("chunks_tall", map.chunks_tall())?;

    let w = map.tile_width();
    let h = map.tile_height();
    let tiles = lua.create_table_with_capacity(0, (w * h) as usize)?;

    let mut idx: i64 = 1;
    for gy in 0..h {
        for gx in 0..w {
            // Coordinates are always valid here — they are derived from map dimensions
            let kind = map
                .get_tile(MapCoord::new(gx, gy))
                .expect("map coordinate must be valid")
                .kind
                .as_str();
            tiles.set(idx, kind)?;
            idx += 1;
        }
    }

    t.set("tiles", tiles)?;
    Ok(t)
}
