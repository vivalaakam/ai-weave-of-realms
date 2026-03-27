//! Helper for converting a [`GameMap`] into a Lua table representation.
//!
//! Both the evaluator and validator receive the map as a Lua table:
//! ```lua
//! {
//!   chunks_wide  = <number>,
//!   chunks_tall  = <number>,
//!   tile_width   = <number>,   -- chunks_wide * 32
//!   tile_height  = <number>,   -- chunks_tall * 32
//!   chunk_size   = 32,
//!   tiles        = { "meadow", "water", ... },  -- flat, row-major, 1-indexed
//!   get          = function(x, y) -> string,    -- safe 0-based accessor
//! }
//! ```

use mlua::{Lua, Table};
use rpg_engine::map::game_map::{GameMap, MapCoord};

/// Converts a [`GameMap`] into a Lua table for use by evaluation/validation scripts.
///
/// The `tiles` field is a flat 1-indexed Lua array in global row-major order.
/// The `get(x, y)` method provides a safe 0-based accessor (returns `"out_of_bounds"`
/// for coordinates outside the map).
///
/// # Errors
/// Returns an [`mlua::Error`] if Lua table creation or field assignment fails.
pub fn game_map_to_lua_table<'lua>(lua: &'lua Lua, map: &GameMap) -> mlua::Result<Table> {
    let t = lua.create_table()?;

    let tw = map.tile_width();
    let th = map.tile_height();

    t.set("chunks_wide", map.chunks_wide())?;
    t.set("chunks_tall", map.chunks_tall())?;
    t.set("tile_width", tw)?;
    t.set("tile_height", th)?;
    t.set("chunk_size", rpg_engine::map::chunk::CHUNK_SIZE as u32)?;

    // Build flat 1-indexed tile array in global row-major order
    let tiles = lua.create_table_with_capacity(0, (tw * th) as usize)?;
    let mut idx: i64 = 1;
    for gy in 0..th {
        for gx in 0..tw {
            let kind = map
                .get_tile(MapCoord::new(gx, gy))
                .expect("map coordinate must be valid")
                .kind
                .as_str();
            tiles.set(idx, kind)?;
            idx += 1;
        }
    }
    t.set("tiles", tiles.clone())?;

    // Add a safe 0-based get(x, y) accessor as a Lua function stored on the table
    // Uses the already-built `tiles` table and captured `tw`/`th`.
    let get_fn = lua.create_function(move |_, (tiles_ref, x, y): (Table, i64, i64)| {
        if x < 0 || y < 0 || x >= tw as i64 || y >= th as i64 {
            return Ok("out_of_bounds".to_string());
        }
        let lua_idx = y * tw as i64 + x + 1;
        let kind: String = tiles_ref.get(lua_idx).unwrap_or_else(|_| "unknown".to_string());
        Ok(kind)
    })?;

    // Wrap into a closure that already captures the tiles table
    let tiles_for_get = tiles.clone();
    let get_closure = lua.create_function(move |_, (x, y): (i64, i64)| {
        if x < 0 || y < 0 || x >= tw as i64 || y >= th as i64 {
            return Ok("out_of_bounds".to_string());
        }
        let lua_idx = y * tw as i64 + x + 1;
        let kind: String = tiles_for_get
            .get(lua_idx)
            .unwrap_or_else(|_| "unknown".to_string());
        Ok(kind)
    })?;

    // Suppress the unused `get_fn` warning — it was superseded by `get_closure`
    drop(get_fn);
    t.set("get", get_closure)?;

    Ok(t)
}
