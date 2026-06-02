//! ChunkGrid — временная структура для генерации и stitching.
//!
//! Используется только внутри rpg-mapgen.  После генерации и stitching
//! преобразуется в плоский [`GameMap`] через [`ChunkGrid::into_game_map`].

use engine::error::EngineError;
use engine::map::chunk::{Chunk, CHUNK_SIZE};
use engine::map::game_map::GameMap;

use crate::error::MapgenError;

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
    /// Returns [`MapgenError::Engine`] if `chunks.len() != chunks_wide * chunks_tall`.
    pub fn new(
        chunks_wide: u32,
        chunks_tall: u32,
        chunks: Vec<Chunk>,
        seed: [u8; 32],
    ) -> Result<Self, MapgenError> {
        let expected = (chunks_wide * chunks_tall) as usize;
        if chunks.len() != expected {
            return Err(MapgenError::Engine(EngineError::InvalidChunksSize {
                expected,
                got: chunks.len(),
            }));
        }
        Ok(Self { chunks_wide, chunks_tall, chunks, seed })
    }

    /// Flattens all chunks into a [`GameMap`] in global row-major tile order.
    ///
    /// # Errors
    /// Returns [`EngineError`] if [`GameMap::new`] fails (should not happen for a
    /// well-formed grid).
    pub fn into_game_map(self) -> Result<GameMap, EngineError> {
        let cs = CHUNK_SIZE;
        let grid_w = self.chunks_wide as usize;
        let grid_h = self.chunks_tall as usize;
        let tw = grid_w * cs;
        let th = grid_h * cs;
        let mut tiles = Vec::with_capacity(tw * th);

        for cy in 0..grid_h {
            for ly in 0..cs {
                for cx in 0..grid_w {
                    let chunk = &self.chunks[cy * grid_w + cx];
                    let row_start = ly * cs;
                    let row_end = row_start + cs;
                    tiles.extend_from_slice(&chunk.tiles()[row_start..row_end]);
                }
            }
        }

        GameMap::new(tw as u32, th as u32, tiles, self.seed)
    }
}
