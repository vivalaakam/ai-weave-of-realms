//! Error types for the rpg-engine crate.

use alloc::string::String;
use core::fmt;

use crate::hero::HeroId;

/// All errors that can occur within the rpg-engine crate.
#[derive(Debug)]
pub enum Error {
    /// A map coordinate or index is out of valid bounds.
    OutOfBounds(String),
    /// An invalid tile identifier (unknown name, GID or ID) was encountered.
    InvalidTileKind(String),
    /// A tile slice or vector had the wrong length.
    InvalidTiles(String),
    /// A game state operation was attempted in an invalid state.
    InvalidChunksSize { expected: usize, got: usize },
    /// A game state operation was attempted in an invalid state.
    InvalidTilesSize { expected: usize, got: usize },
    /// The map-generation pipeline does not contain any generators.
    PipelineEmpty,
    /// A validation-rule directory could not be read.
    #[cfg(feature = "std")]
    ValidationRuleDir {
        /// Path that failed to load.
        path: String,
        /// Underlying I/O error.
        err: std::io::Error,
    },
    /// Movement was requested to a tile that cannot be reached.
    UnreachableTile { x: u32, y: u32 },
    /// Movement was requested to a tile occupied by another hero.
    OccupiedTile { x: u32, y: u32 },
    /// Movement was requested to a tile that is not passable terrain.
    ImpassableTile { x: u32, y: u32 },
    /// Movement was requested but the hero has no movement points remaining.
    NoMovementPoints { hero_id: HeroId },
    /// Save/load serialization failed.
    Save(String),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::OutOfBounds(message) => write!(formatter, "out of bounds: {message}"),
            Error::InvalidTileKind(message) => write!(formatter, "invalid tile: {message}"),
            Error::InvalidTiles(message) => write!(formatter, "invalid tile data: {message}"),
            Error::InvalidChunksSize { expected, got } => {
                write!(
                    formatter,
                    "invalid game state: expected {expected} chunks got {got}"
                )
            }
            Error::InvalidTilesSize { expected, got } => {
                write!(formatter, "invalid game state: expected {expected} tiles got {got}")
            }
            Error::PipelineEmpty => formatter.write_str("pipeline must have at least one generator"),
            #[cfg(feature = "std")]
            Error::ValidationRuleDir { path, err } => {
                write!(formatter, "cannot read validation rule directory '{path}': {err}")
            }
            Error::UnreachableTile { x, y } => write!(formatter, "unreachable tile at ({x}, {y})"),
            Error::OccupiedTile { x, y } => {
                write!(formatter, "tile at ({x}, {y}) is occupied by another hero")
            }
            Error::ImpassableTile { x, y } => write!(formatter, "impassable tile at ({x}, {y})"),
            Error::NoMovementPoints { hero_id } => {
                write!(formatter, "hero {hero_id} has no movement points remaining")
            }
            Error::Save(message) => write!(formatter, "save error: {message}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
