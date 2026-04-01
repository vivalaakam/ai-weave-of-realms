//! ChunkGrid — временная структура для генерации и stitching.
//!
//! Используется только внутри rpg-mapgen.  После генерации и stitching
//! преобразуется в плоский [`GameMap`] через [`ChunkGrid::into_game_map`].

use rpg_engine::error::Error as EngineError;
use rpg_engine::map::chunk::{Chunk, ChunkCoord, CHUNK_SIZE};
use rpg_engine::map::game_map::GameMap;

use crate::error::Error;

/// Temporary N×M grid of chunks used during map generation and stitching.
///
/// After generation and stitching, call [`ChunkGrid::into_game_map`] to flatten
/// the grid into a single contiguous [`GameMap`] for use by the game engine.
pub(crate) struct ChunkGrid {
    /// Number of chunks along the horizontal axis.
    pub chunks_wide: u32,
    /// Number of chunks along the vertical axis.
    pub chunks_tall: u32,
    /// Chunks in row-major order: `chunks[cy * chunks_wide + cx]`.
    pub(crate) chunks: Vec<Chunk>,
    /// The 32-byte seed this grid was generated from.
    pub seed: [u8; 32],
}

impl ChunkGrid {
    /// Creates a [`ChunkGrid`] from a pre-generated flat `Vec<Chunk>`.
    ///
    /// # Errors
    /// Returns [`Error::Engine`] if `chunks.len() != chunks_wide * chunks_tall`.
    pub fn new(
        chunks_wide: u32,
        chunks_tall: u32,
        chunks: Vec<Chunk>,
        seed: [u8; 32],
    ) -> Result<Self, Error> {
        let expected = (chunks_wide * chunks_tall) as usize;
        if chunks.len() != expected {
            return Err(Error::Engine(EngineError::InvalidChunksSize {
                expected,
                got: chunks.len(),
            }));
        }
        Ok(Self {
            chunks_wide,
            chunks_tall,
            chunks,
            seed,
        })
    }

    /// Returns a reference to the chunk at `coord`.
    ///
    /// # Errors
    /// Returns [`EngineError::OutOfBounds`] if the coordinate is outside the grid.
    // TODO: used by future pathfinding / inspection passes
    #[allow(dead_code)]
    pub fn get_chunk(&self, coord: ChunkCoord) -> Result<&Chunk, EngineError> {
        let idx = self.chunk_index(coord)?;
        Ok(&self.chunks[idx])
    }

    /// Returns a mutable reference to the chunk at `coord`.
    ///
    /// # Errors
    /// Returns [`EngineError::OutOfBounds`] if the coordinate is outside the grid.
    // TODO: used by future pathfinding / inspection passes
    #[allow(dead_code)]
    pub fn get_chunk_mut(&mut self, coord: ChunkCoord) -> Result<&mut Chunk, EngineError> {
        let idx = self.chunk_index(coord)?;
        Ok(&mut self.chunks[idx])
    }

    /// Flattens all chunks into a [`GameMap`] in global row-major tile order.
    ///
    /// # Errors
    /// Returns [`EngineError`] if [`GameMap::new`] fails (should not happen for a
    /// well-formed grid).
    pub fn into_game_map(self) -> Result<GameMap, EngineError> {
        let cs = CHUNK_SIZE as u32;
        let tw = self.chunks_wide * cs;
        let th = self.chunks_tall * cs;
        let mut tiles = Vec::with_capacity((tw * th) as usize);

        for gy in 0..th {
            let cy = gy / cs;
            let ly = (gy % cs) as usize;
            for gx in 0..tw {
                let cx = gx / cs;
                let lx = (gx % cs) as usize;
                let chunk = &self.chunks[(cy * self.chunks_wide + cx) as usize];
                tiles.push(*chunk.get(lx, ly)?);
            }
        }

        GameMap::new(tw, th, tiles, self.seed)
    }

    fn chunk_index(&self, coord: ChunkCoord) -> Result<usize, EngineError> {
        if coord.x >= self.chunks_wide || coord.y >= self.chunks_tall {
            return Err(EngineError::OutOfBounds(format!(
                "chunk ({}, {}) outside {}×{} grid",
                coord.x, coord.y, self.chunks_wide, self.chunks_tall
            )));
        }
        Ok((coord.y * self.chunks_wide + coord.x) as usize)
    }
}
