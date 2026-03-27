//! Error types for the rpg-mapgen crate.

use thiserror::Error;

/// All errors that can occur within the rpg-mapgen crate.
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to load or compile a Lua script file.
    #[error("failed to load Lua script '{path}': {source}")]
    ScriptLoad {
        path: String,
        #[source]
        source: mlua::Error,
    },

    /// A Lua script returned an unexpected value or panicked during execution.
    #[error("Lua execution error in '{function}': {source}")]
    LuaExecution {
        function: String,
        #[source]
        source: mlua::Error,
    },

    /// The Lua generator script returned an invalid tile table.
    #[error("invalid chunk data returned by Lua: {0}")]
    InvalidChunkData(String),

    /// Map validation failed with a message from the Lua validator.
    #[error("map validation failed: {0}")]
    ValidationFailed(String),

    /// An engine-level error propagated from rpg-engine.
    #[error("engine error: {0}")]
    Engine(#[from] rpg_engine::error::Error),

    /// I/O error while reading a Lua script file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("map generation pipeline failed: {0}")]
    PipelineFailed(String),
}
