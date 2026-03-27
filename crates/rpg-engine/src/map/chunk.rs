//! Chunk — a 32×32 tile region of the game map.

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::map::tile::Tile;

/// Width and height of a chunk in tiles.
pub const CHUNK_SIZE: usize = 32;

/// Total number of tiles in a single chunk (`CHUNK_SIZE × CHUNK_SIZE`).
pub const CHUNK_TILE_COUNT: usize = CHUNK_SIZE * CHUNK_SIZE;

// ─── ChunkCoord ───────────────────────────────────────────────────────────────

/// Grid coordinates of a chunk within the game map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkCoord {
    /// Horizontal chunk index (column), starting at 0.
    pub x: u32,
    /// Vertical chunk index (row), starting at 0.
    pub y: u32,
}

impl ChunkCoord {
    /// Creates a new chunk coordinate.
    pub fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }

    /// Returns a byte representation suitable for seed derivation.
    ///
    /// Format: `b"chunk_{x}_{y}"`.
    pub fn to_seed_context(self) -> Vec<u8> {
        format!("chunk_{}_{}", self.x, self.y).into_bytes()
    }
}

// ─── Chunk ────────────────────────────────────────────────────────────────────

/// A [`CHUNK_SIZE`]×[`CHUNK_SIZE`] (32×32) contiguous region of isometric tiles.
///
/// Tiles are stored in row-major order: `tiles[y * CHUNK_SIZE + x]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// Grid position of this chunk within the parent [`super::game_map::GameMap`].
    pub coord: ChunkCoord,
    /// Flat vector of all [`CHUNK_TILE_COUNT`] tiles, in row-major order.
    tiles: Vec<Tile>,
}

impl Chunk {
    /// Creates a new chunk at the given coordinate, filling all tiles with `fill`.
    pub fn filled(coord: ChunkCoord, fill: Tile) -> Self {
        Self {
            coord,
            tiles: vec![fill; CHUNK_TILE_COUNT],
        }
    }

    /// Creates a new chunk from a flat tile vector.
    ///
    /// # Errors
    /// Returns [`Error::InvalidTileKind`] if `tiles` does not contain exactly
    /// [`CHUNK_TILE_COUNT`] elements.
    pub fn from_vec(coord: ChunkCoord, tiles: Vec<Tile>) -> Result<Self, Error> {
        if tiles.len() != CHUNK_TILE_COUNT {
            return Err(Error::InvalidTileKind(format!(
                "expected {} tiles, got {}",
                CHUNK_TILE_COUNT,
                tiles.len()
            )));
        }
        Ok(Self { coord, tiles })
    }

    /// Returns a reference to the tile at local coordinates `(x, y)`.
    ///
    /// # Errors
    /// Returns [`Error::OutOfBounds`] if `x >= CHUNK_SIZE` or `y >= CHUNK_SIZE`.
    pub fn get(&self, x: usize, y: usize) -> Result<&Tile, Error> {
        if x >= CHUNK_SIZE || y >= CHUNK_SIZE {
            return Err(Error::OutOfBounds(format!(
                "({x}, {y}) in chunk of size {CHUNK_SIZE}"
            )));
        }
        Ok(&self.tiles[y * CHUNK_SIZE + x])
    }

    /// Returns a mutable reference to the tile at local coordinates `(x, y)`.
    ///
    /// # Errors
    /// Returns [`Error::OutOfBounds`] if `x >= CHUNK_SIZE` or `y >= CHUNK_SIZE`.
    pub fn get_mut(&mut self, x: usize, y: usize) -> Result<&mut Tile, Error> {
        if x >= CHUNK_SIZE || y >= CHUNK_SIZE {
            return Err(Error::OutOfBounds(format!(
                "({x}, {y}) in chunk of size {CHUNK_SIZE}"
            )));
        }
        Ok(&mut self.tiles[y * CHUNK_SIZE + x])
    }

    /// Returns a flat slice over all tiles in row-major order.
    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    /// Returns a mutable flat slice over all tiles in row-major order.
    pub fn tiles_mut(&mut self) -> &mut [Tile] {
        &mut self.tiles
    }

    /// Returns the width of the chunk in tiles (always [`CHUNK_SIZE`]).
    pub fn width(&self) -> usize {
        CHUNK_SIZE
    }

    /// Returns the height of the chunk in tiles (always [`CHUNK_SIZE`]).
    pub fn height(&self) -> usize {
        CHUNK_SIZE
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::tile::TileKind;

    #[test]
    fn chunk_filled_correct_size() {
        let chunk = Chunk::filled(ChunkCoord::new(0, 0), Tile::default());
        assert_eq!(chunk.tiles().len(), CHUNK_TILE_COUNT);
    }

    #[test]
    fn chunk_get_round_trip() {
        let mut chunk = Chunk::filled(ChunkCoord::new(1, 2), Tile::default());
        chunk.get_mut(3, 5).unwrap().kind = TileKind::Water;
        assert_eq!(chunk.get(3, 5).unwrap().kind, TileKind::Water);
    }

    #[test]
    fn chunk_get_out_of_bounds_returns_error() {
        let chunk = Chunk::filled(ChunkCoord::new(0, 0), Tile::default());
        assert!(chunk.get(CHUNK_SIZE, 0).is_err());
        assert!(chunk.get(0, CHUNK_SIZE).is_err());
    }

    #[test]
    fn chunk_from_vec_wrong_size_returns_error() {
        let tiles = vec![Tile::default(); 10];
        assert!(Chunk::from_vec(ChunkCoord::new(0, 0), tiles).is_err());
    }

    #[test]
    fn coord_seed_context_is_unique() {
        let a = ChunkCoord::new(0, 1).to_seed_context();
        let b = ChunkCoord::new(1, 0).to_seed_context();
        assert_ne!(a, b);
    }
}
