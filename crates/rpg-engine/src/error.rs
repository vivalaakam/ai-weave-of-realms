//! Error types for the rpg-engine crate.

use thiserror::Error;

use crate::hero::HeroId;

/// All errors that can occur within the rpg-engine crate.
#[derive(Debug, Error)]
pub enum Error {
    /// A map coordinate or index is out of valid bounds.
    #[error("out of bounds: {0}")]
    OutOfBounds(String),

    /// An invalid tile identifier (unknown name, GID or ID) was encountered.
    #[error("invalid tile: {0}")]
    InvalidTileKind(String),

    /// A tile slice or vector had the wrong length.
    #[error("invalid tile data: {0}")]
    InvalidTiles(String),

    /// A game state operation was attempted in an invalid state.
    #[error("invalid game state: expected {expected} chunks got {got}")]
    InvalidChunksSize { expected: usize, got: usize },

    /// A game state operation was attempted in an invalid state.
    #[error("invalid game state: expected {expected} tiles got {got}")]
    InvalidTilesSize { expected: usize, got: usize },

    #[error("pipeline must have at least one generator")]
    PipelineEmpty,

    #[error("cannot read validation rule directory '{path}': {err}")]
    ValidationRuleDir { path: String, err: std::io::Error },

    /// Movement was requested to a tile that cannot be reached.
    #[error("unreachable tile at ({x}, {y})")]
    UnreachableTile { x: u32, y: u32 },

    /// Movement was requested to a tile occupied by another hero.
    #[error("tile at ({x}, {y}) is occupied by another hero")]
    OccupiedTile { x: u32, y: u32 },

    /// Movement was requested to a tile that is not passable terrain.
    #[error("impassable tile at ({x}, {y})")]
    ImpassableTile { x: u32, y: u32 },

    /// Movement was requested but the hero has no movement points remaining.
    #[error("hero {hero_id} has no movement points remaining")]
    NoMovementPoints { hero_id: HeroId },
}
