//! Host-side file I/O helpers for map discovery, save loading and session creation.
#![cfg(feature = "std")]

use engine::error::EngineError;
use engine::game_state::GameState;
use engine::hero::Hero;
use engine::map::game_map::{GameMap, MapCoord};
use engine::spawn;
use engine::spawn::SpawnError;
use engine::team::Team;
use mapgen::error::MapgenError;
use mapgen::map_assembler::{MapAssembler, MapConfig};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, instrument};

pub use crate::types::ListEntry;

/// Error type for host-side I/O operations.
#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("I/O error: {0}")]
    FileIO(#[from] std::io::Error),
    #[error("Engine error: {0}")]
    Engine(#[from] EngineError),
    #[error("Mapgen error: {0}")]
    Mapgen(#[from] MapgenError),
    #[error("Spawn error: {0}")]
    Spawn(#[from] SpawnError),
    #[error("{0}")]
    Message(String),
}

/// Generates a map using the provided CLI arguments.
#[instrument(level = "info")]
pub fn generate_map(
    seed: String,
    width: u32,
    height: u32,
    generators: Option<&Path>,
    validator_dir: Option<&Path>,
    validator: Option<&Path>,
    evaluator: Option<&Path>,
) -> Result<GameMap, IoError> {
    let generator = generators.unwrap_or_else(|| Path::new("scripts/generators/default.lua"));

    let mut config = MapConfig::default_3x3(seed.clone(), generator);
    config.width = width;
    config.height = height;

    if let Some(path) = &validator_dir {
        config = config.with_validator_dir(path);
    } else if let Some(path) = &validator {
        config = config.with_validator(path);
    } else {
        let default = PathBuf::from("scripts/rules");
        if default.is_dir() {
            config = config.with_validator_dir(default);
        }
    }

    if let Some(path) = &evaluator {
        config = config.with_evaluator(path);
    } else {
        let default = PathBuf::from("scripts/evaluators/evaluate.lua");
        if default.exists() {
            config = config.with_evaluator(default);
        }
    }

    let assembler = MapAssembler::new(config)?;
    match assembler.generate_validated() {
        Ok(map) => Ok(map),
        Err(MapgenError::ValidationFailed(reason)) => {
            info!(%reason, "map failed validation, falling back to raw generation");
            Ok(assembler.generate()?)
        }
        Err(e) => Err(e.into()),
    }
}

/// Loads a `GameState` from a save file, TMX map, or generated map.
#[instrument(level = "info")]
pub fn load_state(
    seed: &str,
    width: u32,
    height: u32,
    generators: Option<&Path>,
    validator_dir: Option<&Path>,
    validator: Option<&Path>,
    evaluator: Option<&Path>,
    save_path: Option<&Path>,
) -> Result<GameState, IoError> {
    if let Some(path) = save_path {
        let bytes = fs::read(path)?;
        let state = GameState::from_save_bytes(&bytes)?;
        info!(path = %path.display(), "loaded saved game state");
        return Ok(state);
    }

    let map = generate_map(
        seed.to_string(),
        width,
        height,
        generators,
        validator_dir,
        validator,
        evaluator,
    )?;

    build_default_state(map, seed)
}

/// Builds a default `GameState` with standard heroes and teams.
pub fn build_default_state(map: GameMap, seed: &str) -> Result<GameState, IoError> {
    let team_cfgs = [
        TeamConfig { name: "Red".to_string(), color: (220, 50, 50), player_controlled: true },
        TeamConfig { name: "Enemy".to_string(), color: (150, 80, 200), player_controlled: false },
    ];
    build_state_with_teams(map, seed, &team_cfgs).map_err(IoError::Engine)
}

/// Configuration for a single team when building a game state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamConfig {
    pub name: String,
    pub color: (u8, u8, u8),
    pub player_controlled: bool,
}

/// Build a `GameState` from a map with configurable teams.
pub fn build_state_with_teams(
    map: GameMap,
    seed: &str,
    teams: &[TeamConfig],
) -> Result<GameState, EngineError> {
    let entrance_spawns = spawn::find_city_entrance_spawns(&map, teams.len());
    let mut state = GameState::new(map, seed);

    for (i, cfg) in teams.iter().enumerate() {
        let team_id = state.add_team(Team::new(i as u8, &cfg.name, cfg.color, cfg.player_controlled));
        let hero_pos = entrance_spawns.get(i).copied().unwrap_or_else(|| MapCoord::new(0, 0));
        let hero_name = cfg.name.to_string();
        state.add_hero(Hero::new(
            i as u8,
            &hero_name,
            100,
            20,
            10,
            15,
            hero_pos,
            team_id,
        ));
        state.set_city_owner(hero_pos, Some(team_id));
    }

    let _ = state.on_turn();
    Ok(state)
}

/// Loads a `GameState` from an entry, with configurable teams.
#[instrument(level = "info")]
pub fn load_state_with_teams(
    seed: &str,
    width: u32,
    height: u32,
    generators: Option<&Path>,
    validator_dir: Option<&Path>,
    validator: Option<&Path>,
    evaluator: Option<&Path>,
    save_path: Option<&Path>,
    team_cfgs: &[TeamConfig],
) -> Result<GameState, IoError> {
    if let Some(path) = save_path {
        let bytes = fs::read(path)?;
        let state = GameState::from_save_bytes(&bytes)?;
        info!(path = %path.display(), "loaded saved game state");
        return Ok(state);
    }

    let map = generate_map(
        seed.to_string(),
        width,
        height,
        generators,
        validator_dir,
        validator,
        evaluator,
    )?;

    build_state_with_teams(map, seed, team_cfgs).map_err(IoError::Engine)
}

/// Load a `GameMap` from a `ListEntry` without creating a `GameState`.
#[instrument(level = "info")]
pub fn load_map_only(
    seed: &str,
    width: u32,
    height: u32,
    generators: Option<&Path>,
    validator_dir: Option<&Path>,
    validator: Option<&Path>,
    evaluator: Option<&Path>,
    save_path: Option<&Path>,
) -> Result<GameMap, IoError> {
    if let Some(path) = save_path {
        let bytes = fs::read(path)?;
        let state = GameState::from_save_bytes(&bytes)?;
        info!(path = %path.display(), "loaded map from save");
        return Ok(state.map);
    }

    generate_map(
        seed.to_string(),
        width,
        height,
        generators,
        validator_dir,
        validator,
        evaluator,
    )
}

/// Configurable spawn finder for team placements.
/// Finds up to `count` city entrance or city tiles spread across the map.
pub fn find_city_entrance_spawns(map: &GameMap, count: usize) -> Vec<MapCoord> {
    spawn::find_city_entrance_spawns(map, count)
}

/// Discovers `.rpgs` files in a directory
#[instrument(level = "info")]
pub fn discover_rpgs_dir(dir: &Path, prefix: &str) -> Result<Vec<ListEntry>, IoError> {
    let mut entries: Vec<ListEntry> = Vec::new();
    if !dir.is_dir() {
        return Ok(entries);
    }

    let read_dir = fs::read_dir(dir)?;
    for entry in read_dir {
        let entry = entry?;
        let path = entry.path();
        if !is_rpgs_path(&path) {
            continue;
        }
        let metadata = entry.metadata()?;
        let label = read_save_name(&path).unwrap_or_else(|| file_label(&path));
        entries.push(ListEntry {
            id: format!("{prefix}{}", path.display()),
            label,
            meta: u32::try_from(metadata.len()).unwrap_or(u32::MAX),
        });
    }
    Ok(entries)
}

/// Creates a `ListEntry` from a file path.
pub fn file_entry(prefix: &str, path: &Path) -> Result<ListEntry, IoError> {
    let metadata = fs::metadata(path)?;
    Ok(ListEntry {
        id: format!("{prefix}{}", path.display()),
        label: file_label(path),
        meta: u32::try_from(metadata.len()).unwrap_or(u32::MAX),
    })
}

/// Extracts the filename stem as a label.
pub fn file_label(path: &Path) -> String {
    path.file_stem().and_then(|value| value.to_str()).unwrap_or("unnamed").to_string()
}

/// Reads the save name from the first bytes of an `.rpgs` file.
pub fn read_save_name(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    GameState::read_save_name(&bytes).ok()
}

/// Checks whether a file has the `.rpgs` extension.
pub fn is_rpgs_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("rpgs"))
        .unwrap_or(false)
}

/// Sanitises a user-provided name into a safe 8.3-style filename.
pub fn sanitize_save_filename(name: &str) -> String {
    let mut cleaned = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            cleaned.push(ch.to_ascii_uppercase());
        }
    }
    if cleaned.is_empty() {
        cleaned.push_str("SAVE");
    }
    cleaned.truncate(7);
    format!("{cleaned}.RPGS")
}
