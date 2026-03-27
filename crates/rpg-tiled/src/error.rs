//! Error types for the rpg-tiled crate.

use thiserror::Error;

/// All errors that can occur within the rpg-tiled crate.
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to parse TMX XML data.
    #[error("TMX parse error: {0}")]
    Parse(#[from] quick_xml::Error),

    /// A required TMX attribute or element was missing.
    #[error("missing TMX field '{0}'")]
    MissingField(String),

    /// An attribute value could not be parsed to the expected type.
    #[error("invalid TMX attribute '{field}': {value}")]
    InvalidAttribute { field: String, value: String },

    /// The TMX map dimensions do not match expected chunk layout.
    #[error("dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: String, got: String },

    /// An unknown tile GID was encountered during import.
    #[error("unknown tile GID {0}")]
    UnknownGid(u32),

    /// An engine-level error propagated from rpg-engine.
    #[error("engine error: {0}")]
    Engine(#[from] rpg_engine::error::Error),

    /// I/O error during file read/write.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
