use crate::app::LoadedGame;
use crate::info_overlay::InfoOverlay;
use crate::io::{discover_rpgs_dir, load_state, sanitize_save_filename, IoError, ListEntry};
use engine::error::EngineError;
use engine::game_state::GameState;
use std::path::Path;
use std::{fs, io};

#[derive(Debug, thiserror::Error)]
pub enum AppHostError {
    #[error("Failed to discover dir {0}: {1}")]
    DiscoverDir(String, IoError),
    #[error("Unknow save entry: {0}")]
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
    LoadMapGeneratedState(IoError),
    #[error("Failed to read map: {0}")]
    LoadMapReadFailed(String, io::Error),
    #[error("Failed to load map {0}: {1}")]
    LoadMapEngineError(String, EngineError),
    #[error("Unknow map")]
    LoadMapFailed(),
}

/// Host-side storage and platform hooks required by the shared controller.
pub trait AppHost {
    fn get_maps_dir(&self) -> &Path;
    fn get_saves_dir(&self) -> &Path;
    fn get_seed(&self) -> &str;

    fn get_width(&self) -> u32;
    fn get_height(&self) -> u32;

    fn get_screen_width(&self) -> u32;
    fn get_screen_height(&self) -> u32;

    fn get_generator(&self) -> Option<&Path>;

    fn get_validator_dir(&self) -> Option<&Path>;

    fn get_validator(&self) -> Option<&Path>;

    fn get_evaluator(&self) -> Option<&Path>;

    /// Returns all loadable maps shown under "New Game".
    fn discover_maps(&mut self) -> Result<Vec<ListEntry>, AppHostError> {
        let mut entries = discover_rpgs_dir(self.get_maps_dir(), "map:").map_err(|err| {
            AppHostError::DiscoverDir(self.get_maps_dir().to_string_lossy().into_owned(), err)
        })?;

        entries.push(ListEntry {
            id: format!("generated:{}", self.get_seed()),
            label: format!("Generated map ({})", self.get_seed()),
            meta: self.get_width().saturating_mul(self.get_height()),
        });
        entries.sort_by(|left, right| left.label.cmp(&right.label));
        Ok(entries)
    }
    /// Returns all loadable saves shown under "Load Game".
    fn discover_saves(&mut self) -> Result<Vec<ListEntry>, AppHostError> {
        let mut entries = discover_rpgs_dir(self.get_saves_dir(), "save:").map_err(|err| {
            AppHostError::DiscoverDir(self.get_maps_dir().to_string_lossy().into_owned(), err)
        })?;

        entries.sort_by(|left, right| left.label.cmp(&right.label));
        Ok(entries)
    }

    /// Loads a map entry into a full engine state.
    fn load_map(&mut self, entry: &ListEntry) -> Result<LoadedGame, AppHostError> {
        if entry.id.starts_with("generated:") {
            let state = load_state(
                &self.get_seed(),
                self.get_width(),
                self.get_height(),
                self.get_generator(),
                self.get_validator_dir(),
                self.get_validator(),
                self.get_evaluator(),
                None,
            )
            .map_err(AppHostError::LoadMapGeneratedState)?;

            return Ok(LoadedGame { map_name: entry.label.clone(), state });
        }
        if let Some(path) = entry.id.strip_prefix("map:") {
            let bytes = fs::read(path)
                .map_err(|err| AppHostError::LoadMapReadFailed(entry.id.clone(), err))?;

            let state = GameState::from_save_bytes(&bytes)
                .map_err(|error| AppHostError::LoadMapEngineError(path.to_string(), error))?;
            return Ok(LoadedGame { map_name: entry.label.clone(), state });
        }

        Err(AppHostError::LoadMapFailed())
    }
    /// Loads a save entry into a full engine state.
    fn load_save(&mut self, entry: &ListEntry) -> Result<LoadedGame, AppHostError> {
        let path = entry
            .id
            .strip_prefix("save:")
            .ok_or(AppHostError::LoadSaveEntryUnknown(entry.id.clone()))?;

        let bytes = fs::read(path)
            .map_err(|err| AppHostError::LoadSaveLoadFailed(entry.id.clone(), err))?;

        let state = GameState::from_save_bytes(&bytes)
            .map_err(|error| AppHostError::LoadSaveEngineError(entry.id.clone(), error))?;

        Ok(LoadedGame { map_name: entry.label.clone(), state })
    }
    /// Persists the current engine state as a save file.
    fn save_game(&mut self, name: &str, state: &GameState) -> Result<(), AppHostError> {
        fs::create_dir_all(self.get_saves_dir()).map_err(|err| {
            AppHostError::SaveGameCreateDirFailed(
                self.get_saves_dir().to_string_lossy().to_string(),
                err,
            )
        })?;
        let file_name = sanitize_save_filename(name);

        let path = self.get_saves_dir().join(file_name);

        let bytes =
            state.to_save_bytes_with_name(name).map_err(AppHostError::SaveGameEngineError)?;
        fs::write(&path, bytes).map_err(|err| {
            AppHostError::SaveGameWriteFailed(path.to_string_lossy().to_string(), err)
        })?;

        Ok(())
    }
    /// Builds an optional platform information overlay.
    fn info_overlay(&mut self) -> Option<InfoOverlay> {
        Some(InfoOverlay::new(
            "System Info".to_string(),
            vec![
                format!("Viewport: {}x{}", self.get_screen_width(), self.get_screen_height()),
                format!("Seed: {}", self.get_seed()),
            ],
            "Enter or q: close".to_string(),
        ))
    }
    /// Converts a host-specific error into a user-visible message.
    fn error_message(&self, error: AppHostError) -> String {
        error.to_string()
    }
}
