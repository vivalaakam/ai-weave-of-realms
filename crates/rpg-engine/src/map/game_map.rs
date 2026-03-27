//! [`GameMap`] — the full assembled game map composed of N×M chunks.

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::map::chunk::{Chunk, ChunkCoord, CHUNK_SIZE};
use crate::map::tile::Tile;

// ─── MapCoord ─────────────────────────────────────────────────────────────────

/// Absolute tile coordinates within the full game map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MapCoord {
    /// Horizontal tile index from the left edge of the map.
    pub x: u32,
    /// Vertical tile index from the top edge of the map.
    pub y: u32,
}

impl MapCoord {
    /// Creates a new map coordinate.
    pub fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }

    /// Decomposes this coordinate into a chunk coordinate and a local in-chunk
    /// `(lx, ly)` offset.
    pub fn to_chunk_local(self) -> (ChunkCoord, usize, usize) {
        let cx = self.x as usize / CHUNK_SIZE;
        let cy = self.y as usize / CHUNK_SIZE;
        let lx = self.x as usize % CHUNK_SIZE;
        let ly = self.y as usize % CHUNK_SIZE;
        (ChunkCoord::new(cx as u32, cy as u32), lx, ly)
    }
}

// ─── GameMap ──────────────────────────────────────────────────────────────────

/// The full game map composed of `chunks_wide × chunks_tall` chunks.
///
/// Total tile dimensions: `(chunks_wide * 32) × (chunks_tall * 32)`.
/// Chunks are stored in row-major order: `chunks[cy * chunks_wide + cx]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameMap {
    /// Number of chunks along the horizontal axis.
    chunks_wide: u32,
    /// Number of chunks along the vertical axis.
    chunks_tall: u32,
    /// Flat array of all chunks in row-major order.
    chunks: Vec<Chunk>,
    /// The 32-byte seed this map was generated from.
    pub seed: [u8; 32],
}

impl GameMap {
    /// Creates a new [`GameMap`] from a flat `Vec<Chunk>`.
    ///
    /// # Arguments
    /// * `chunks_wide` - Number of chunks per row.
    /// * `chunks_tall` - Number of chunk rows.
    /// * `chunks`      - Pre-generated chunks in row-major order.
    /// * `seed`        - The map seed used during generation.
    ///
    /// # Errors
    /// Returns [`Error::InvalidState`] if `chunks.len() != chunks_wide * chunks_tall`.
    pub fn new(
        chunks_wide: u32,
        chunks_tall: u32,
        chunks: Vec<Chunk>,
        seed: [u8; 32],
    ) -> Result<Self, Error> {
        let expected = (chunks_wide * chunks_tall) as usize;
        if chunks.len() != expected {
            return Err(Error::InvalidState(format!(
                "expected {expected} chunks, got {}",
                chunks.len()
            )));
        }
        Ok(Self {
            chunks_wide,
            chunks_tall,
            chunks,
            seed,
        })
    }

    /// Returns the number of chunks along the horizontal axis.
    pub fn chunks_wide(&self) -> u32 {
        self.chunks_wide
    }

    /// Returns the number of chunks along the vertical axis.
    pub fn chunks_tall(&self) -> u32 {
        self.chunks_tall
    }

    /// Returns the total width of the map in tiles.
    pub fn tile_width(&self) -> u32 {
        self.chunks_wide * CHUNK_SIZE as u32
    }

    /// Returns the total height of the map in tiles.
    pub fn tile_height(&self) -> u32 {
        self.chunks_tall * CHUNK_SIZE as u32
    }

    /// Returns a reference to the chunk at the given chunk grid coordinate.
    ///
    /// # Errors
    /// Returns [`Error::OutOfBounds`] if the coordinate is outside the map.
    pub fn get_chunk(&self, coord: ChunkCoord) -> Result<&Chunk, Error> {
        let idx = self.chunk_index(coord)?;
        Ok(&self.chunks[idx])
    }

    /// Returns a mutable reference to the chunk at the given chunk grid coordinate.
    ///
    /// # Errors
    /// Returns [`Error::OutOfBounds`] if the coordinate is outside the map.
    pub fn get_chunk_mut(&mut self, coord: ChunkCoord) -> Result<&mut Chunk, Error> {
        let idx = self.chunk_index(coord)?;
        Ok(&mut self.chunks[idx])
    }

    /// Returns all chunks as a flat slice in row-major order.
    pub fn chunks(&self) -> &[Chunk] {
        &self.chunks
    }

    /// Returns a reference to the tile at the given absolute map coordinate.
    ///
    /// # Errors
    /// Returns [`Error::OutOfBounds`] if the coordinate is outside the map.
    pub fn get_tile(&self, coord: MapCoord) -> Result<&Tile, Error> {
        let (chunk_coord, lx, ly) = coord.to_chunk_local();
        self.get_chunk(chunk_coord)?.get(lx, ly)
    }

    /// Returns a mutable reference to the tile at the given absolute map coordinate.
    ///
    /// # Errors
    /// Returns [`Error::OutOfBounds`] if the coordinate is outside the map.
    pub fn get_tile_mut(&mut self, coord: MapCoord) -> Result<&mut Tile, Error> {
        let (chunk_coord, lx, ly) = coord.to_chunk_local();
        self.get_chunk_mut(chunk_coord)?.get_mut(lx, ly)
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// Computes the flat index of a chunk in the `chunks` vector.
    ///
    /// # Errors
    /// Returns [`Error::OutOfBounds`] if the coordinate is outside the map.
    fn chunk_index(&self, coord: ChunkCoord) -> Result<usize, Error> {
        if coord.x >= self.chunks_wide || coord.y >= self.chunks_tall {
            return Err(Error::OutOfBounds(format!(
                "chunk ({}, {}) outside {}×{} map",
                coord.x, coord.y, self.chunks_wide, self.chunks_tall
            )));
        }
        Ok((coord.y * self.chunks_wide + coord.x) as usize)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::tile::{Tile, TileKind};

    fn make_map(cw: u32, ct: u32) -> GameMap {
        let chunks = (0..cw * ct)
            .map(|i| {
                Chunk::filled(
                    ChunkCoord::new(i % cw, i / cw),
                    Tile::new(TileKind::Grass),
                )
            })
            .collect();
        GameMap::new(cw, ct, chunks, [0u8; 32]).unwrap()
    }

    #[test]
    fn tile_dimensions_are_correct() {
        let map = make_map(3, 3);
        assert_eq!(map.tile_width(), 96);
        assert_eq!(map.tile_height(), 96);
    }

    #[test]
    fn get_chunk_out_of_bounds_returns_error() {
        let map = make_map(3, 3);
        assert!(map.get_chunk(ChunkCoord::new(3, 0)).is_err());
    }

    #[test]
    fn get_tile_returns_correct_tile() {
        let mut map = make_map(3, 3);
        let coord = MapCoord::new(33, 1); // chunk (1,0), local (1,1)
        map.get_tile_mut(coord).unwrap().kind = TileKind::Water;
        assert_eq!(map.get_tile(coord).unwrap().kind, TileKind::Water);
    }

    #[test]
    fn chunk_count_mismatch_returns_error() {
        let chunks = vec![Chunk::filled(ChunkCoord::new(0, 0), Tile::default())];
        assert!(GameMap::new(3, 3, chunks, [0u8; 32]).is_err());
    }

    #[test]
    fn map_coord_to_chunk_local() {
        let coord = MapCoord::new(65, 33);
        let (cc, lx, ly) = coord.to_chunk_local();
        assert_eq!(cc, ChunkCoord::new(2, 1));
        assert_eq!(lx, 1);
        assert_eq!(ly, 1);
    }
}
