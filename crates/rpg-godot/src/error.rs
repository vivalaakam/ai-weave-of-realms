//! Error types for the rpg-godot crate.

use thiserror::Error;

/// All errors that can occur within the rpg-godot crate.
#[derive(Debug, Error)]
pub enum Error {
    /// An engine-level error propagated from rpg-engine.
    #[error("engine error: {0}")]
    Engine(#[from] rpg_engine::error::Error),

    /// A map generation error propagated from rpg-mapgen.
    #[error("mapgen error: {0}")]
    MapGen(#[from] rpg_mapgen::error::Error),

    /// A Tiled import/export error propagated from rpg-tiled.
    #[error("tiled error: {0}")]
    Tiled(#[from] rpg_tiled::error::Error),

    /// A Godot node was not found by the expected path.
    #[error("Godot node not found at path '{0}'")]
    NodeNotFound(String),

    /// An unexpected Godot signal argument type was received.
    #[error("unexpected signal argument: {0}")]
    UnexpectedSignalArg(String),
}
