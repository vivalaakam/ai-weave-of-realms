//! Host-side file I/O helpers for map discovery, save loading and session creation.
#![cfg(feature = "std")]

use std::fs;
use std::path::{Path, PathBuf};

use rpg_engine::game_state::GameState;
use rpg_engine::hero::Hero;
use rpg_engine::map::game_map::GameMap;
use rpg_engine::spawn;
use rpg_engine::team::Team;
use rpg_mapgen::error::Error as MapgenError;
use rpg_mapgen::map_assembler::{MapAssembler, MapConfig};
use rpg_tiled::read_tmx;
pub use rpg_engine::map::game_map::GameMap;
pub use rpg_engine::hero::Hero;
pub use rpg_engine::team::Team;
pub use rpg_engine::spawn;
pub use rpg_mapgen::map_assembler::{MapAssembler, MapConfig};
pub use rpg_mapgen::error::Error as MapgenError;
pub use rpg_tiled::read_tmx;

/// Shared list entry shown in selector UIs.
/// Shared list entry shown in selector UIs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListEntry {
    /// Stable host-specific identifier used for loading the selected item.
    pub id: String,
    /// Primary display label.
    pub label: String,
    /// Secondary numeric metadata, usually file size.
    pub meta: u32,
}

/// Error type for host-side I/O operations.
#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("I/O error: {0}")]
    FileIO(#[from] std::io::Error),
    #[error("Engine error: {0}")]
    Engine(#[from] rpg_engine::error::Error),
    #[error("Mapgen error: {0}")]
    Mapgen(#[from] MapgenError),
    #[error("{0}")]
    Message(String),
}

/// Result alias for host I/O operations.
pub type Result<T> = std::result::Result<T, IoError>;

/// Generates a map using the provided CLI arguments.
pub fn generate_map(seed: String, width: u32, height: u32, generators: Vec<PathBuf>, validator_dir: Option<PathBuf>, validator: Option<PathBuf>, evaluator: Option<PathBuf>) -> Result<GameMap> {
    let mut generators = generators;
    if generators.is_empty() {
        generators.push(PathBuf::from("scripts/generators/default.lua"));
    }

    let first = generators.remove(0);
    let mut config = MapConfig::default_3x3(seed.clone(), first);
    config.width = width;
    config.height = height;

    for g in generators {
        config = config.with_generator(g);
    }

    if let Some(path) = &validator_dir {
        config = config.with_validator_dir(path.clone());
    } else if let Some(path) = &validator {
        config = config.with_validator(path.clone());
    } else {
        let default = PathBuf::from("scripts/rules");
        if default.is_dir() {
            config = config.with_validator_dir(default);
        }
    }

    if let Some(path) = &evaluator {
        config = config.with_evaluator(path.clone());
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
            tracing::info!(%reason, "map failed validation, falling back to raw generation");
            Ok(assembler.generate()?)
        }
        Err(e) => Err(e.into()),
    }
}

/// Loads a `GameState` from a save file, TMX map, or generated map.
pub fn load_state(seed: &str, width: u32, height: u32, generators: Vec<PathBuf>, validator_dir: Option<PathBuf>, validator: Option<PathBuf>, evaluator: Option<PathBuf>, save_path: Option<&Path>, tmx_path: Option<&Path>) -> Result<GameState> {
    if let Some(path) = save_path {
        let bytes = fs::read(path)?;
        let state = GameState::from_save_bytes(&bytes)?;
        tracing::info!(path = %path.display(), "loaded saved game state");
        return Ok(state);
    }

    if let Some(path) = tmx_path {
        let map = read_tmx(path)?;
        tracing::info!(path = %path.display(), "loaded TMX map");
        return build_default_state(map, seed);
    }

    let map = generate_map(seed.to_string(), width, height, generators, validator_dir, validator, evaluator)?;
    build_default_state(map, seed)
}

/// Builds a default `GameState` with standard heroes and teams.
pub fn build_default_state(map: GameMap, seed: &str) -> Result<GameState> {
    let spawns = spawn::find_spawn_positions(&map)?;
    let mut state = GameState::new(map, seed);
    let player_team_id = state.add_team(Team::red());
    let enemy_team_id = state.add_team(Team::enemy());

    state.add_hero(Hero::new(
        0,
        "Hero",
        100,
        20,
        10,
        15,
        spawns.player,
        player_team_id,
    ));
    state.add_hero(Hero::new(
        1,
        "Enemy",
        85,
        16,
        8,
        12,
        spawns.enemy,
        enemy_team_id,
    ));
    let _ = state.set_city_owner(spawns.player, Some(player_team_id));
    let _ = state.on_turn();
    Ok(state)
}

/// Discovers `.rpgs` files in a directory.
pub fn discover_rpgs_dir(dir: &Path, prefix: &str) -> Result<Vec<ListEntry>> {
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
pub fn file_entry(prefix: &str, path: &Path) -> Result<ListEntry> {
    let metadata = fs::metadata(path)?;
    Ok(ListEntry {
        id: format!("{prefix}{}", path.display()),
        label: file_label(path),
        meta: u32::try_from(metadata.len()).unwrap_or(u32::MAX),
    })
}

/// Extracts the filename stem as a label.
pub fn file_label(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("unnamed")
        .to_string()
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
