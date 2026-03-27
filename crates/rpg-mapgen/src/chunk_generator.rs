//! Chunk generator — loads a Lua script and produces a single 32×32 [`Chunk`].
//!
//! # Lua script contract
//! The script must return a function with the signature:
//! ```lua
//! function generate_chunk(rng, x, y) -> table[1024]
//! ```
//! where `rng` is a [`LuaRng`] userdata, `x`/`y` are the chunk grid indices,
//! and the return value is a 1-indexed Lua table of exactly 1024 tile kind strings.

use std::path::Path;

use mlua::{Function, Lua, Table};
use tracing::{debug, instrument};

use rpg_engine::map::chunk::{Chunk, ChunkCoord, CHUNK_TILE_COUNT};
use rpg_engine::map::tile::{Tile, Tiles};
use rpg_engine::rng::derive_seed;

use crate::error::Error;
use crate::rng_userdata::LuaRng;

// ─── ChunkGenerator ──────────────────────────────────────────────────────────

/// Generates a single 32×32 [`Chunk`] by calling a Lua script function.
///
/// The generator owns its own [`Lua`] VM and reuses it across all
/// `generate` calls, avoiding repeated VM initialisation overhead.
pub struct ChunkGenerator {
    /// The Lua VM that owns `func` and all userdata created during generation.
    lua: Lua,
    /// The compiled `generate_chunk` function loaded from the script file.
    func: Function,
}

impl ChunkGenerator {
    /// Loads a chunk generator script from the given path.
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

        debug!(path = %path.display(), "chunk generator script loaded");
        Ok(Self { lua, func })
    }

    /// Generates a single chunk at `coord` using the supplied `map_seed`.
    ///
    /// The chunk seed is deterministically derived as
    /// `derive_seed(map_seed, coord.to_seed_context())`, so each
    /// `(map_seed, coord)` pair always produces the same chunk.
    ///
    /// # Arguments
    /// * `coord`    - Grid position of the chunk within the parent map.
    /// * `map_seed` - The 32-byte map-level seed.
    ///
    /// # Errors
    /// Returns [`Error::LuaExecution`] if the Lua function fails, or
    /// [`Error::InvalidChunkData`] if the returned table is malformed.
    #[instrument(skip(self, map_seed), fields(cx = coord.x, cy = coord.y))]
    pub fn generate(&self, coord: ChunkCoord, map_seed: &[u8; 32]) -> Result<Chunk, Error> {
        let chunk_seed = derive_seed(map_seed, &coord.to_seed_context());
        let rng = self
            .lua
            .create_userdata(LuaRng::new(chunk_seed))
            .map_err(|source| Error::LuaExecution {
                function: "create_userdata(LuaRng)".into(),
                source,
            })?;

        let tiles_table: Table = self
            .func
            .call((rng, coord.x, coord.y))
            .map_err(|source| Error::LuaExecution {
                function: "generate_chunk".into(),
                source,
            })?;

        debug!(cx = coord.x, cy = coord.y, "chunk generated, parsing tiles");
        parse_tiles_table(tiles_table, coord)
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Parses a 1-indexed Lua table of tile kind strings into a [`Chunk`].
///
/// # Errors
/// Returns [`Error::InvalidChunkData`] if the table length is wrong or
/// contains an unrecognised tile kind string.
fn parse_tiles_table(table: Table, coord: ChunkCoord) -> Result<Chunk, Error> {
    let mut tiles = Vec::with_capacity(CHUNK_TILE_COUNT);

    for i in 1..=(CHUNK_TILE_COUNT as i64) {
        let kind_str: String = table.get(i).map_err(|_| {
            Error::InvalidChunkData(format!("missing tile at Lua index {i}"))
        })?;

        let kind = Tiles::from_str(&kind_str).map_err(|_| {
            Error::InvalidChunkData(format!("unknown tile kind '{kind_str}' at index {i}"))
        })?;

        tiles.push(Tile::new(kind));
    }

    Chunk::from_vec(coord, tiles).map_err(Error::from)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rpg_engine::rng::keccak256;
    use crate::test_utils::init_tracing;

    /// Inline minimal generator script for tests (no filesystem dependency).
    fn make_inline_generator() -> ChunkGenerator {
        let source = r#"
            return function(rng, x, y)
                local tiles = {}
                for i = 1, 32 * 32 do
                    tiles[i] = "meadow"
                end
                return tiles
            end
        "#;
        let lua = Lua::new();
        let func: Function = lua.load(source).eval().unwrap();
        ChunkGenerator { lua, func }
    }

    #[test]
    fn generates_correct_tile_count() {
        init_tracing();
        let gen = make_inline_generator();
        let seed = keccak256("test-seed");
        let chunk = gen.generate(ChunkCoord::new(0, 0), &seed).unwrap();
        assert_eq!(chunk.tiles().len(), CHUNK_TILE_COUNT);
    }

    #[test]
    fn same_seed_same_chunk() {
        init_tracing();
        let gen = make_inline_generator();
        let seed = keccak256("deterministic");
        let a = gen.generate(ChunkCoord::new(1, 2), &seed).unwrap();
        let b = gen.generate(ChunkCoord::new(1, 2), &seed).unwrap();
        assert_eq!(a.tiles(), b.tiles());
    }

    #[test]
    fn from_script_with_default_lua() {
        init_tracing();
        // Inline script that uses rng to fill with varied terrain
        let source = r#"
            return function(rng, x, y)
                local kinds = {"meadow", "water", "forest", "mountain", "road", "ruins"}
                local tiles = {}
                for i = 1, 32 * 32 do
                    local idx = rng:random_range_u32(1, 7)
                    tiles[i] = kinds[idx]
                end
                return tiles
            end
        "#;
        let lua = Lua::new();
        let func: Function = lua.load(source).eval().unwrap();
        let gen = ChunkGenerator { lua, func };

        let seed = keccak256("rng-test");
        let chunk = gen.generate(ChunkCoord::new(0, 0), &seed).unwrap();
        assert_eq!(chunk.tiles().len(), CHUNK_TILE_COUNT);
    }

    #[test]
    fn invalid_tile_kind_returns_error() {
        init_tracing();
        let source = r#"
            return function(rng, x, y)
                local tiles = {}
                for i = 1, 32 * 32 do
                    tiles[i] = "lava"
                end
                return tiles
            end
        "#;
        let lua = Lua::new();
        let func: Function = lua.load(source).eval().unwrap();
        let gen = ChunkGenerator { lua, func };

        let seed = keccak256("bad-tiles");
        let result = gen.generate(ChunkCoord::new(0, 0), &seed);
        assert!(matches!(result, Err(Error::InvalidChunkData(_))));
    }
}
