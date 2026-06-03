//! Global tile configuration.
//!
//! [`TILE_CONFIG`] is a process-wide singleton that holds the [`TileConfig`]
//! loaded from YAML at start-up. The binary should call [`init_tile_config`]
//! once to load configuration.

use alloc::format;

use crate::error::EngineError;

pub(crate) mod tile_config;
pub use tile_config::{AtlasIndex, TileConfig, TileEntry};

pub mod team_config;
pub use team_config::{TeamCatalog, TeamDef, TeamKind, TeamLogo};

pub mod hero_config;
pub use hero_config::HeroCatalog;

use serde::{Deserialize, Serialize};

/// Static game configuration loaded at application initialization and stored
/// with each [`crate::game_state::GameState`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub tiles: TileConfig,
    pub teams: TeamCatalog,
    pub heroes: HeroCatalog,
}

impl GameConfig {
    pub fn new(tiles: TileConfig, teams: TeamCatalog, heroes: HeroCatalog) -> Self {
        Self { tiles, teams, heroes }
    }
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            tiles: TileConfig::default(),
            teams: TeamCatalog::default(),
            heroes: HeroCatalog::default(),
        }
    }
}

/// Load a [`TileConfig`] from a YAML string and install it as the global
/// singleton.
///
/// # Errors
/// Returns [`EngineError::InvalidTiles`] if the YAML is malformed.
///
/// # Panics
/// Panics if called more than once per process; use only in `main`.
pub fn init_tile_config(yaml: &str) -> Result<TileConfig, EngineError> {
    let config: TileConfig = serde_yaml::from_str(yaml)
        .map_err(|e| EngineError::InvalidTiles(format!("failed to parse tiles YAML: {e}")))?;

    Ok(config)
}

/// Load a [`TeamCatalog`] from a YAML string and install it as the global
/// singleton.
///
/// # Errors
/// Returns [`EngineError::InvalidTiles`] if the YAML is malformed.
///
/// # Panics
/// Panics if called more than once per process; use only in `main`.
pub fn init_team_catalog(yaml: &str) -> Result<TeamCatalog, EngineError> {
    let catalog = TeamCatalog::from_yaml(yaml)?;
    Ok(catalog)
}

/// Load a [`HeroCatalog`] from a YAML string and install it as the global
/// singleton.
///
/// # Errors
/// Returns [`EngineError::InvalidTiles`] if the YAML is malformed.
///
/// # Panics
/// Panics if called more than once per process; use only in `main`.
pub fn init_hero_catalog(yaml: &str) -> Result<HeroCatalog, EngineError> {
    let catalog = HeroCatalog::from_yaml(yaml)?;
    Ok(catalog)
}
