use crate::args::Args;
use crate::error::HostError;
use game::app::{AppHost, LoadedGame};
use game::info_overlay::InfoOverlay;
use game::io::{discover_rpgs_dir, file_entry, load_state, sanitize_save_filename, ListEntry};
use game::prelude::render::Size;
use game::GameState;
use std::fs;
use std::path::Path;

pub struct SdlHost {
    pub(crate) args: Args,
    pub(crate) screen_size: Size,
    pub(crate) left_x_right: bool,
    pub(crate) left_x_left: bool,
    pub(crate) left_y_down: bool,
    pub(crate) left_y_up: bool,
    pub(crate) right_x_right: bool,
    pub(crate) right_x_left: bool,
    pub(crate) right_y_down: bool,
    pub(crate) right_y_up: bool,
    pub(crate) trigger_r_active: bool,
}

impl AppHost for SdlHost {
    type Error = HostError;

    fn discover_maps(&mut self) -> Result<Vec<ListEntry>, Self::Error> {
        let mut entries = discover_rpgs_dir(Path::new("maps"), "map:")?;
        if let Some(path) = &self.args.tmx {
            entries.push(file_entry("tmx:", path)?);
        }
        if entries.is_empty() {
            entries.push(ListEntry {
                id: format!("generated:{}", self.args.seed),
                label: format!("Generated map ({})", self.args.seed),
                meta: self.args.width.saturating_mul(self.args.height),
            });
        }
        entries.sort_by(|left, right| left.label.cmp(&right.label));
        Ok(entries)
    }

    fn discover_saves(&mut self) -> Result<Vec<ListEntry>, Self::Error> {
        let mut entries = discover_rpgs_dir(Path::new("savegame"), "save:")?;
        if let Some(path) = &self.args.save {
            entries.push(file_entry("save:", path)?);
        }
        entries.sort_by(|left, right| left.label.cmp(&right.label));
        Ok(entries)
    }

    fn load_map(&mut self, entry: &ListEntry) -> Result<LoadedGame, Self::Error> {
        if let Some(path) = entry.id.strip_prefix("tmx:") {
            let state = load_state(
                &self.args.seed,
                self.args.width,
                self.args.height,
                self.args.generators.clone(),
                self.args.validator_dir.clone(),
                self.args.validator.clone(),
                self.args.evaluator.clone(),
                None,
                Some(Path::new(path)),
            )?;
            return Ok(LoadedGame { map_name: entry.label.clone(), state });
        }
        if entry.id.starts_with("generated:") {
            let state = load_state(
                &self.args.seed,
                self.args.width,
                self.args.height,
                self.args.generators.clone(),
                self.args.validator_dir.clone(),
                self.args.validator.clone(),
                self.args.evaluator.clone(),
                None,
                None,
            )?;
            return Ok(LoadedGame { map_name: entry.label.clone(), state });
        }
        if let Some(path) = entry.id.strip_prefix("map:") {
            let bytes = fs::read(path).map_err(HostError::Io)?;
            let state = GameState::from_save_bytes(&bytes)
                .map_err(|error| HostError::Engine(error.to_string()))?;
            return Ok(LoadedGame { map_name: entry.label.clone(), state });
        }
        Err(HostError::Message("Unknown map entry".to_string()))
    }

    fn load_save(&mut self, entry: &ListEntry) -> Result<LoadedGame, Self::Error> {
        let path = entry
            .id
            .strip_prefix("save:")
            .ok_or_else(|| HostError::Message("Unknown save entry".to_string()))?;
        let bytes = fs::read(path).map_err(HostError::Io)?;
        let state = GameState::from_save_bytes(&bytes)
            .map_err(|error| HostError::Engine(error.to_string()))?;
        Ok(LoadedGame { map_name: entry.label.clone(), state })
    }

    fn save_game(&mut self, name: &str, state: &GameState) -> Result<(), Self::Error> {
        let dir = Path::new("savegame");
        fs::create_dir_all(dir).map_err(HostError::Io)?;
        let file_name = sanitize_save_filename(name);
        let path = dir.join(file_name);
        let bytes = state
            .to_save_bytes_with_name(name)
            .map_err(|error| HostError::Engine(error.to_string()))?;
        fs::write(path, bytes).map_err(HostError::Io)
    }

    fn info_overlay(&mut self) -> Option<InfoOverlay> {
        Some(InfoOverlay::new(
            "System Info".to_string(),
            vec![
                format!("Viewport: {}x{}", self.screen_size.width, self.screen_size.height),
                format!("Seed: {}", self.args.seed),
            ],
            "Enter or q: close".to_string(),
        ))
    }

    fn error_message(&self, error: Self::Error) -> String {
        match error {
            HostError::Message(message) => message,
            HostError::Io(error) => format!("I/O error: {error}"),
            HostError::Engine(message) => format!("Engine error: {message}"),
        }
    }
}
