//! Map types: tiles, chunks, and the assembled game map.
//!
//! # Structure
//! - [`tile`] — [`TileKind`] enum and [`Tile`] struct
//! - [`chunk`] — [`Chunk`] (32×32 tiles) and [`ChunkCoord`]
//! - [`game_map`] — [`GameMap`] (N×M chunks) and [`MapCoord`]

pub mod chunk;
pub mod game_map;
pub mod tile;

pub use chunk::{Chunk, ChunkCoord, CHUNK_SIZE, CHUNK_TILE_COUNT};
pub use game_map::{GameMap, MapCoord};
pub use tile::{Tile, TileKind};
