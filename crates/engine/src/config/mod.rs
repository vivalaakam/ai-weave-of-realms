//! Global tile configuration.
//!
//! [`TILE_CONFIG`] is a process-wide singleton that holds the [`TileConfig`]
//! loaded from YAML at start-up. The binary should call [`init_tile_config`]
//! once; the built-in defaults are used as a fallback.

use alloc::format;
use alloc::sync::Arc;

use std::sync::OnceLock;

use crate::error::EngineError;

pub(crate) mod tile_config;
pub use tile_config::{default_tile_config, AtlasIndex, TileConfig, TileEntry};

/// Global singleton holding the active [`TileConfig`].
///
/// Populated by calling [`init_tile_config`] once at application start.
static TILE_CONFIG: OnceLock<Arc<TileConfig>> = OnceLock::new();

/// Load a [`TileConfig`] from a YAML string and install it as the global
/// singleton.
///
/// # Errors
/// Returns [`EngineError::InvalidTiles`] if the YAML is malformed.
///
/// # Panics
/// Panics if called more than once per process; use only in `main`.
pub fn init_tile_config(yaml: &str) -> Result<(), EngineError> {
    let config: TileConfig = serde_yaml::from_str(yaml)
        .map_err(|e| EngineError::InvalidTiles(format!("failed to parse tiles YAML: {e}")))?;

    let arc = Arc::new(config);
    // Only allow initialisation once — callers typically invoke this from `main`.
    assert!(TILE_CONFIG.set(arc).is_ok(), "init_tile_config called twice");
    Ok(())
}

/// Return a reference to the global [`TileConfig`], falling back to the
/// built-in default if [`init_tile_config`] has not yet been called.
pub fn get_tile_config() -> Arc<TileConfig> {
    TILE_CONFIG.get().cloned().unwrap_or_else(|| Arc::new(default_tile_config()))
}
