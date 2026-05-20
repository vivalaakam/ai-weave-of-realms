//! Engine-backed gameplay session shared by embedded frontends.

use alloc::{
    format,
    string::{String, ToString},
};

use rpg_engine::error::Error as EngineError;
use rpg_engine::game_state::GameState;
use rpg_engine::hero::HeroId;
use rpg_engine::map::game_map::MapCoord;
use rpg_engine::Direction;

/// Runtime game session stored by embedded gameplay frontends.
pub struct GameSession {
    map_name: String,
    state: GameState,
    selected_hero_id: HeroId,
}

impl GameSession {
    /// Creates a new engine session from a fully loaded save state.
    ///
    /// # Arguments
    /// * `map_name` - Display name of the loaded map or save.
    /// * `state` - Loaded engine state.
    ///
    /// # Returns
    /// A new [`GameSession`] ready for rendering and input.
    ///
    /// # Errors
    /// Returns [`EngineError::InvalidTiles`] if the state has no heroes to select.
    pub fn from_state(map_name: String, state: GameState) -> Result<Self, EngineError> {
        let selected_hero_id = select_hero(&state)
            .ok_or_else(|| EngineError::InvalidTiles("save has no heroes".to_string()))?;
        Ok(Self { map_name, state, selected_hero_id })
    }

    /// Returns the display name of the loaded map.
    pub fn map_name(&self) -> &str {
        &self.map_name
    }

    /// Returns the immutable engine state.
    pub fn state(&self) -> &GameState {
        &self.state
    }

    /// Returns the mutable engine state.
    pub fn state_mut(&mut self) -> &mut GameState {
        &mut self.state
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
    ///
    /// # Arguments
    /// * `direction` - Single-step movement direction.
    ///
    /// # Returns
    /// The new hero position after a successful move.
    ///
    /// # Errors
    /// Returns any engine move error such as impassable terrain or zero movement points.
    pub fn move_selected_hero(&mut self, direction: Direction) -> Result<MapCoord, EngineError> {
        self.state.move_hero(self.selected_hero_id, direction)?;
        Ok(self.selected_hero_position())
    }

    /// Cycles to the next living player-controlled hero.
    ///
    /// If the current hero is the only living player hero, it stays selected.
    pub fn cycle_selected_hero(&mut self) {
        let player_team = self.state.hero(self.selected_hero_id).map(|h| h.team_id);
        if let Some(team_id) = player_team {
            if let Some(next) = self.state.get_next_hero(team_id) {
                self.selected_hero_id = next;
            }
        }
    }

    /// Ends the current turn, advances to the next team, and selects its first hero.
    ///
    /// # Returns
    /// A status summary string after the turn transition.
    ///
    /// # Errors
    /// Returns any engine error from `on_turn`.
    pub fn end_turn(&mut self) -> Result<String, EngineError> {
        self.state.on_turn().map_err(|e| EngineError::InvalidTiles(e.to_string()))?;
        let new_team = self.state.get_active_team_id().ok().copied();
        if let Some(team_id) = new_team {
            if let Some(next) = self.state.get_next_hero(team_id) {
                self.selected_hero_id = next;
            }
        }
        Ok(self.summary())
    }

    /// Returns a short one-line status summary for HUD rendering.
    pub fn summary(&self) -> String {
        let team = self.state.get_active_team().map(|active| active.name.as_str()).unwrap_or("?");
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
