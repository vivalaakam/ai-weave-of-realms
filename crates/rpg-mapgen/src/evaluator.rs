//! Map evaluator — scores a [`GameMap`] using a Lua script.
//!
//! The Lua script must return a function with the signature:
//! ```lua
//! function evaluate(map) -> number
//! ```
//! A higher score indicates a better map.  The assembler can use this to
//! select the best candidate from multiple generated maps.

use std::path::Path;

use mlua::{Function, Lua};
use tracing::{debug, instrument};

use rpg_engine::map::game_map::GameMap;

use crate::error::Error;
use crate::map_table::game_map_to_lua_table;

// ─── MapEvaluator ─────────────────────────────────────────────────────────────

/// Evaluates a [`GameMap`] by calling a Lua `evaluate(map)` function.
pub struct MapEvaluator {
    /// The Lua VM that owns `func` and all intermediate values.
    lua: Lua,
    /// The compiled `evaluate` function loaded from the script file.
    func: Function,
}

impl MapEvaluator {
    /// Loads an evaluator script from the given file path.
    ///
    /// The script must evaluate to (return) a Lua function.
    ///
    /// # Errors
    /// Returns [`Error::ScriptLoad`] if the file cannot be read or the script
    /// fails to compile / does not return a function.
    pub fn from_script(path: &Path) -> Result<Self, Error> {
        let source = std::fs::read_to_string(path)?;
        let lua = Lua::new();

        let func: Function = lua
            .load(&source)
            .set_name(path.to_string_lossy().as_ref())
            .eval()
            .map_err(|source| Error::ScriptLoad {
                path: path.to_string_lossy().into_owned(),
                source,
            })?;

        debug!(path = %path.display(), "evaluator script loaded");
        Ok(Self { lua, func })
    }

    /// Scores the given map by calling the Lua `evaluate` function.
    ///
    /// # Returns
    /// A non-negative score where a higher value indicates a better map.
    ///
    /// # Errors
    /// Returns [`Error::LuaExecution`] if the Lua function raises an error.
    #[instrument(skip(self, map))]
    pub fn evaluate(&self, map: &GameMap) -> Result<f64, Error> {
        let map_table = game_map_to_lua_table(&self.lua, map).map_err(|source| {
            Error::LuaExecution {
                function: "game_map_to_lua_table".into(),
                source,
            }
        })?;

        let score: f64 = self
            .func
            .call(map_table)
            .map_err(|source| Error::LuaExecution {
                function: "evaluate".into(),
                source,
            })?;

        debug!(score, "map evaluated");
        Ok(score)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Function;
    use rpg_engine::map::chunk::{Chunk, ChunkCoord};
    use rpg_engine::map::tile::{Tile, Tiles};
    use crate::test_utils::init_tracing;

    fn make_evaluator(script: &str) -> MapEvaluator {
        let lua = Lua::new();
        let func: Function = lua.load(script).eval().unwrap();
        MapEvaluator { lua, func }
    }

    fn make_uniform_map(kind: Tiles) -> GameMap {
        let chunks: Vec<Chunk> = (0..9u32)
            .map(|i| Chunk::filled(ChunkCoord::new(i % 3, i / 3), Tile::new(kind)))
            .collect();
        GameMap::new(3, 3, chunks, [0u8; 32]).unwrap()
    }

    #[test]
    fn all_grass_map_scores_above_zero() {
        init_tracing();
        let ev = make_evaluator(
            r#"
            return function(map)
                local count = 0
                for _, k in ipairs(map.tiles) do
                    if k == "meadow" then count = count + 1 end
                end
                return count / #map.tiles * 100
            end
        "#,
        );
        let map = make_uniform_map(Tiles::Meadow);
        let score = ev.evaluate(&map).unwrap();
        assert!(score > 0.0, "all-grass map should score > 0, got {score}");
    }

    #[test]
    fn all_water_map_scores_zero() {
        init_tracing();
        let ev = make_evaluator(
            r#"
            return function(map)
                local passable = 0
                for _, k in ipairs(map.tiles) do
                    if k ~= "water" and k ~= "mountain" then
                        passable = passable + 1
                    end
                end
                return passable / #map.tiles * 100
            end
        "#,
        );
        let map = make_uniform_map(Tiles::Water);
        let score = ev.evaluate(&map).unwrap();
        assert_eq!(score, 0.0, "all-water map should score 0, got {score}");
    }

    #[test]
    fn map_table_has_correct_tile_count() {
        init_tracing();
        let ev = make_evaluator("return function(map) return #map.tiles end");
        let map = make_uniform_map(Tiles::Meadow);
        let count = ev.evaluate(&map).unwrap() as u32;
        assert_eq!(count, map.tile_width() * map.tile_height());
    }
}
