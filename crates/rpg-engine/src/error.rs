//! Error types for the rpg-engine crate.

use thiserror::Error;

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
    #[error("invalid game state: {0}")]
    InvalidState(String),

    /// Movement was requested to a tile that cannot be reached.
    #[error("unreachable tile at ({x}, {y})")]
    UnreachableTile { x: u32, y: u32 },
}
