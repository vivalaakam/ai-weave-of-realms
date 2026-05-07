//! Engine-backed game session wrapper used by the T-Deck frontend.

use alloc::{format, string::{String, ToString}};

use rpg_engine::Direction;
use rpg_engine::error::Error as EngineError;
use rpg_engine::game_state::GameState;
use rpg_engine::hero::HeroId;
use rpg_engine::map::game_map::MapCoord;

/// Runtime game session stored by the map-view screen.
pub struct GameSession {
    map_name: String,
    state: GameState,
    selected_hero_id: HeroId,
}

impl GameSession {
    /// Creates a new engine session from a fully loaded save state.
    ///
    /// # Arguments
    /// * `map_name` - Display name of the loaded save file.
    /// * `state` - Loaded engine state.
    ///
    /// # Returns
    /// A new [`GameSession`] ready for rendering and input.
    ///
    /// # Errors
    /// Returns [`EngineError::InvalidTiles`] if the save has no heroes to select.
    pub fn from_state(map_name: String, state: GameState) -> Result<Self, EngineError> {
        let selected_hero_id = select_hero(&state)
            .ok_or_else(|| EngineError::InvalidTiles("save has no heroes".to_string()))?;
        Ok(Self {
            map_name,
            state,
            selected_hero_id,
        })
    }

    /// Returns the display name of the loaded map.
    pub fn map_name(&self) -> &str {
        &self.map_name
    }

    /// Returns the immutable engine state.
    pub fn state(&self) -> &GameState {
        &self.state
    }

    /// Returns the selected hero id.
    pub fn selected_hero_id(&self) -> HeroId {
        self.selected_hero_id
    }

    /// Returns the selected hero position.
    pub fn selected_hero_position(&self) -> MapCoord {
        self.state
            .hero(self.selected_hero_id)
            .map(|hero| hero.position)
            .unwrap_or(MapCoord::new(0, 0))
    }

    /// Moves the selected hero by one tile using the shared engine logic.
    pub fn move_selected_hero(&mut self, direction: Direction) -> Result<MapCoord, EngineError> {
        self.state.move_hero(self.selected_hero_id, direction)?;
        Ok(self.selected_hero_position())
    }

    /// Returns a short one-line status summary for HUD rendering.
    pub fn summary(&self) -> String {
        let team = self
            .state
            .get_active_team()
            .map(|active| active.name.as_str())
            .unwrap_or("?");
        let hero = self
            .state
            .hero(self.selected_hero_id)
            .map(|selected| selected.name.as_str())
            .unwrap_or("?");
        let position = self.selected_hero_position();
        format!("{team} {hero} @{},{}", position.x, position.y)
    }
}

fn select_hero(state: &GameState) -> Option<HeroId> {
    let active_team = state.get_active_team_id().ok().copied();
    active_team
        .and_then(|team_id| state.get_active_hero(team_id))
        .or_else(|| active_team.and_then(|team_id| state.get_next_hero(team_id)))
        .or_else(|| state.living_heroes(true).first().map(|hero| hero.get_id()))
        .or_else(|| state.living_heroes(false).first().map(|hero| hero.get_id()))
}
