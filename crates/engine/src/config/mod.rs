//! Global tile configuration.
//!
//! [`TILE_CONFIG`] is a process-wide singleton that holds the [`TileConfig`]
//! loaded from YAML at start-up.  On `std` targets the binary should call
//! [`init_tile_config`] once; on `no_std` the built-in defaults are returned
//! automatically.

#[cfg(feature = "std")]
use alloc::format;
use alloc::sync::Arc;

#[cfg(feature = "std")]
use std::sync::OnceLock;

use crate::error::EngineError;

pub(crate) mod tile_config;
pub use tile_config::{default_tile_config, AtlasIndex, TileConfig, TileEntry};

#[cfg(feature = "std")]
/// Global singleton holding the active [`TileConfig`].
///
/// Populated by calling [`init_tile_config`] once at application start.
static TILE_CONFIG: OnceLock<Arc<TileConfig>> = OnceLock::new();

#[cfg(feature = "std")]
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

#[cfg(feature = "std")]
/// Return a reference to the global [`TileConfig`], falling back to the
/// built-in default if [`init_tile_config`] has not yet been called.
pub fn get_tile_config() -> Arc<TileConfig> {
    TILE_CONFIG.get().cloned().unwrap_or_else(|| Arc::new(default_tile_config()))
}

#[cfg(not(feature = "std"))]
/// On `no_std` there is no file system, so we always return the compile-time
/// built-in configuration.
pub fn get_tile_config() -> Arc<TileConfig> {
    // Arc avoids extra copies when the caller clones; on no_std we just
    // allocate once per call — acceptable for a T-Deck read-only config.
    Arc::new(default_tile_config())
}
