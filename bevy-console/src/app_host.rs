use std::path::{Path, PathBuf};
use std::{fs, io};

use bevy::prelude::Resource;
use engine::MapCoord;
use engine::config::{GameConfig, TeamDef};
use engine::error::EngineError;
use engine::game_state::GameState;
use engine::map::game_map::GameMap;
use engine::team::Team;
use helpers::ListEntry;
use helpers::sanitize_save_filename;
use mapgen::error::MapgenError;
use mapgen::map_assembler::{MapAssembler, MapConfig};
use tracing::{info, instrument};

/// Host-side errors for I/O operations.
#[derive(Debug, thiserror::Error)]
pub enum AppHostError {
    #[error("Failed to discover dir {0}: {1}")]
    DiscoverDir(String, io::Error),
    #[error("Unknown save entry: {0}")]
    LoadSaveEntryUnknown(String),
    #[error("Failed to load save {0}: {1}")]
    LoadSaveLoadFailed(String, io::Error),
    #[error("Failed to load game: {0}")]
    LoadSaveEngineError(String, EngineError),
    #[error("Failed to create save dir {0}: {1}")]
    SaveGameCreateDirFailed(String, io::Error),
    #[error("Failed to save game: {0}")]
    SaveGameEngineError(EngineError),
    #[error("Failed to write save file {0}: {1}")]
    SaveGameWriteFailed(String, io::Error),
    #[error("Failed to generate map: {0}")]
    LoadMapGeneratedState(String),
    #[error("Failed to read map: {0}")]
    LoadMapReadFailed(String, io::Error),
    #[error("Failed to load map {0}: {1}")]
    LoadMapEngineError(String, EngineError),
    #[error("Unknown map")]
    LoadMapFailed(),
}

/// Host-side storage and platform hooks.
#[derive(Resource)]
pub struct AppHost {
    pub maps_dir: PathBuf,
    pub saves_dir: PathBuf,
    pub seed: String,
    pub width: u32,
    pub height: u32,
    pub generator: Option<PathBuf>,
    pub validator_dir: Option<PathBuf>,
    pub validator: Option<PathBuf>,
    pub evaluator: Option<PathBuf>,
    pub tiles: engine::config::TileConfig,
    pub team_catalog: engine::config::TeamCatalog,
    pub hero_catalog: engine::config::HeroCatalog,
}

/// Data loaded from a save or map file.
#[derive(Resource)]
pub struct LoadedGame {
    pub state: GameState,
}

/// Data stored between selecting a map and finishing team setup.
#[derive(Resource, Default)]
pub struct PendingMapData {
    pub map_name: String,
    pub map: Option<GameMap>,
}

impl AppHost {
    pub fn game_config(&self) -> GameConfig {
        GameConfig::new(self.tiles.clone(), self.team_catalog.clone(), self.hero_catalog.clone())
    }

    pub fn discover_maps(&mut self) -> Result<Vec<ListEntry>, AppHostError> {
        let mut entries = discover_rpgs_dir(&self.maps_dir, "map:").map_err(|e| {
            AppHostError::DiscoverDir(self.maps_dir.to_string_lossy().into_owned(), e)
        })?;
        entries.push(ListEntry {
            id: format!("generated:{}", self.seed),
            label: format!("Generated map ({})", self.seed),
            meta: self.width.saturating_mul(self.height),
        });
        entries.sort_by(|a, b| a.label.cmp(&b.label));
        Ok(entries)
    }

    pub fn discover_saves(&mut self) -> Result<Vec<ListEntry>, AppHostError> {
        let entries = discover_rpgs_dir(&self.saves_dir, "save:").map_err(|e| {
            AppHostError::DiscoverDir(self.saves_dir.to_string_lossy().into_owned(), e)
        })?;
        Ok(entries)
    }

    pub fn load_map_only(&mut self, entry: &ListEntry) -> Result<GameMap, AppHostError> {
        if entry.id.starts_with("generated:") {
            let map = generate_map(
                self.seed.to_string(),
                self.width,
                self.height,
                self.generator.as_deref(),
                self.validator_dir.as_deref(),
                self.validator.as_deref(),
                self.evaluator.as_deref(),
            )
            .map_err(AppHostError::LoadMapGeneratedState)?;
            return Ok(map);
        }
        if let Some(path) = entry.id.strip_prefix("map:") {
            let bytes =
                fs::read(path).map_err(|e| AppHostError::LoadMapReadFailed(path.to_string(), e))?;
            let mut state = GameState::from_save_bytes(&bytes)
                .map_err(|e| AppHostError::LoadMapEngineError(path.to_string(), e))?;
            state.set_config(self.game_config());
            return Ok(state.map);
        }
        Err(AppHostError::LoadMapFailed())
    }

    pub fn load_save(&mut self, entry: &ListEntry) -> Result<LoadedGame, AppHostError> {
        let path = entry
            .id
            .strip_prefix("save:")
            .ok_or_else(|| AppHostError::LoadSaveEntryUnknown(entry.id.clone()))?;
        let bytes =
            fs::read(path).map_err(|e| AppHostError::LoadSaveLoadFailed(entry.id.clone(), e))?;
        let mut state = GameState::from_save_bytes(&bytes)
            .map_err(|e| AppHostError::LoadSaveEngineError(entry.id.clone(), e))?;
        state.set_config(self.game_config());
        Ok(LoadedGame { state })
    }

    pub fn generate_and_save_map(&mut self, seed: &str) -> Result<ListEntry, AppHostError> {
        let map = generate_map(
            seed.to_string(),
            self.width,
            self.height,
            self.generator.as_deref(),
            self.validator_dir.as_deref(),
            self.validator.as_deref(),
            self.evaluator.as_deref(),
        )
        .map_err(AppHostError::LoadMapGeneratedState)?;
        let state = build_default_state(map, seed, self.game_config())
            .map_err(AppHostError::LoadMapGeneratedState)?;
        fs::create_dir_all(&self.maps_dir).map_err(|e| {
            AppHostError::SaveGameCreateDirFailed(self.maps_dir.to_string_lossy().to_string(), e)
        })?;
        let file_name = sanitize_save_filename(seed);
        let path = self.maps_dir.join(&file_name);
        let bytes = state.to_save_bytes().map_err(AppHostError::SaveGameEngineError)?;
        fs::write(&path, bytes).map_err(|e| {
            AppHostError::SaveGameWriteFailed(path.to_string_lossy().to_string(), e)
        })?;
        Ok(ListEntry {
            id: format!("map:{}", path.display()),
            label: seed.to_string(),
            meta: self.width.saturating_mul(self.height),
        })
    }
}

fn discover_rpgs_dir(dir: &Path, prefix: &str) -> io::Result<Vec<ListEntry>> {
    let mut entries = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("rpgs")) {
                let id = format!("{}{}", prefix, path.display());
                let label = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                entries.push(ListEntry { id, label, meta: entry.metadata()?.len() as u32 });
            }
        }
    }
    entries.sort_by(|a, b| a.label.cmp(&b.label));
    Ok(entries)
}

#[instrument(level = "info")]
fn generate_map(
    seed: String,
    width: u32,
    height: u32,
    generators: Option<&Path>,
    validator_dir: Option<&Path>,
    validator: Option<&Path>,
    evaluator: Option<&Path>,
) -> Result<GameMap, String> {
    let generator = generators.unwrap_or_else(|| Path::new("scripts/generators/default.lua"));
    let mut config = MapConfig::default_3x3(seed.clone(), generator);
    config.width = width;
    config.height = height;
    if let Some(path) = validator_dir {
        config = config.with_validator_dir(path);
    } else if let Some(path) = validator {
        config = config.with_validator(path);
    } else {
        let default = PathBuf::from("scripts/rules");
        if default.is_dir() {
            config = config.with_validator_dir(default);
        }
    }
    if let Some(path) = evaluator {
        config = config.with_evaluator(path);
    } else {
        let default = PathBuf::from("scripts/evaluators/evaluate.lua");
        if default.exists() {
            config = config.with_evaluator(default);
        }
    }
    let assembler = MapAssembler::new(config).map_err(|e| e.to_string())?;
    match assembler.generate_validated() {
        Ok(map) => Ok(map),
        Err(MapgenError::ValidationFailed(reason)) => {
            info!(%reason, "map failed validation, falling back to raw generation");
            assembler.generate().map_err(|e| e.to_string())
        }
        Err(e) => Err(e.to_string()),
    }
}

fn build_default_state(map: GameMap, seed: &str, config: GameConfig) -> Result<GameState, String> {
    build_state_with_teams(map, seed, &vec![], config).map_err(|e| e.to_string())
}

/// Configuration for a single team when building a game state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamConfig {
    pub team_def: TeamDef,
    pub player_controlled: bool,
}

pub fn build_state_with_teams(
    map: GameMap,
    seed: &str,
    teams: &[TeamConfig],
    config: GameConfig,
) -> Result<GameState, EngineError> {
    let entrance_spawns =
        engine::spawn::find_city_entrance_spawns(&map, teams.len(), &config.tiles);
    let mut state = GameState::new_with_config(map, seed, config);
    for (i, cfg) in teams.iter().enumerate() {
        let team_id = state.add_team(Team::new(i as u8, &cfg.team_def, cfg.player_controlled));
        let hero_pos = entrance_spawns.get(i).copied().unwrap_or_else(|| MapCoord::new(0, 0));

        // Player-controlled teams start without a hero — they must hire one at a city entrance.
        // AI-controlled teams get a hero immediately.
        if !cfg.player_controlled
            && let Some(hero) = state.get_hero_candidate_at(0).cloned()
        {
            state.add_hero(team_id, &hero, &hero_pos);
        }

        // The city always belongs to the team regardless of whether a hero is placed.
        state.set_city_owner(hero_pos, Some(team_id));
        // Claim initial territory around the city (radius CITY_INITIAL_RADIUS).
        state.claim_initial_city_territory(team_id);
    }
    let _ = state.on_turn();
    // Grant the first active team its start-of-turn income.
    if let Ok(team_id) = state.get_active_team_id().copied() {
        let _ = state.grant_turn_income(team_id);
    }
    Ok(state)
}
