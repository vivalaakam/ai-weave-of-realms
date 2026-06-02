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

pub mod team_config;
pub use team_config::{TeamCatalog, TeamDef, TeamKind, TeamLogo};

pub mod hero_config;
pub use hero_config::HeroCatalog;

/// Global singleton holding the active [`TileConfig`].
///
/// Populated by calling [`init_tile_config`] once at application start.
static TILE_CONFIG: OnceLock<Arc<TileConfig>> = OnceLock::new();

/// Global singleton holding the active [`TeamCatalog`].
///
/// Populated by calling [`init_team_catalog`] once at application start.
static TEAM_CATALOG: OnceLock<Arc<TeamCatalog>> = OnceLock::new();

/// Global singleton holding the active [`HeroCatalog`].
///
/// Populated by calling [`init_hero_catalog`] once at application start.
static HERO_CATALOG: OnceLock<Arc<HeroCatalog>> = OnceLock::new();

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

/// Load a [`TeamCatalog`] from a YAML string and install it as the global
/// singleton.
///
/// # Errors
/// Returns [`EngineError::InvalidTiles`] if the YAML is malformed.
///
/// # Panics
/// Panics if called more than once per process; use only in `main`.
pub fn init_team_catalog(yaml: &str) -> Result<(), EngineError> {
    let catalog = TeamCatalog::from_yaml(yaml)?;
    let arc = Arc::new(catalog);
    assert!(TEAM_CATALOG.set(arc).is_ok(), "init_team_catalog called twice");
    Ok(())
}

/// Return a reference to the global [`TeamCatalog`], falling back to an empty
/// catalogue if [`init_team_catalog`] has not yet been called.
pub fn get_team_catalog() -> Option<Arc<TeamCatalog>> {
    TEAM_CATALOG.get().cloned()
}

/// Load a [`HeroCatalog`] from a YAML string and install it as the global
/// singleton.
///
/// # Errors
/// Returns [`EngineError::InvalidTiles`] if the YAML is malformed.
///
/// # Panics
/// Panics if called more than once per process; use only in `main`.
pub fn init_hero_catalog(yaml: &str) -> Result<(), EngineError> {
    let catalog = HeroCatalog::from_yaml(yaml)?;
    let arc = Arc::new(catalog);
    assert!(HERO_CATALOG.set(arc).is_ok(), "init_hero_catalog called twice");
    Ok(())
}

/// Return a reference to the global [`HeroCatalog`], or `None` if
/// [`init_hero_catalog`] has not yet been called.
pub fn get_hero_catalog() -> Option<Arc<HeroCatalog>> {
    HERO_CATALOG.get().cloned()
}
