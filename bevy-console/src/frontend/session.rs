use engine::Direction;
use engine::MapCoord;
use engine::error::EngineError;
use engine::game_state::GameState;
use engine::hero::HeroId;

/// Runtime game session stored by frontend clients.
pub struct GameSession {
    state: GameState,
    selected_hero_id: Option<HeroId>,
}

impl GameSession {
    /// Creates a new engine session from a fully loaded save state.
    ///
    /// # Arguments
    /// * `map_name` - Display name of the loaded map or save.
    /// * `state` - Loaded engine state.
    ///
    /// # Note
    /// If no heroes exist yet (e.g. player teams need to hire their first hero),
    /// `selected_hero_id` is set to `None`.
    pub fn from_state(state: GameState) -> Self {
        let selected_hero_id = select_hero(&state);
        Self { state, selected_hero_id }
    }

    /// Returns the immutable engine state.
    pub fn state(&self) -> &GameState {
        &self.state
    }

    /// Returns the mutable engine state.
    pub fn state_mut(&mut self) -> &mut GameState {
        &mut self.state
    }

    /// Returns the selected hero id, if any.
    pub fn selected_hero_id(&self) -> Option<HeroId> {
        self.selected_hero_id
    }

    /// Sets the selected hero id (e.g. after hiring the first hero).
    pub fn set_selected_hero_id(&mut self, id: HeroId) {
        self.selected_hero_id = Some(id);
    }

    /// Returns the selected hero position, or a default (0,0) if no hero selected.
    pub fn selected_hero_position(&self) -> MapCoord {
        self.selected_hero_id
            .and_then(|id| self.state.hero(id).map(|hero| *hero.get_position()))
            .unwrap_or(MapCoord::new(0, 0))
    }

    /// Moves the selected hero by one tile using the shared engine logic.
    ///
    /// # Errors
    /// Returns any engine move error such as impassable terrain or zero movement points,
    /// or if no hero is currently selected.
    pub fn move_selected_hero(&mut self, direction: Direction) -> Result<MapCoord, EngineError> {
        let id = self
            .selected_hero_id
            .ok_or_else(|| EngineError::InvalidTiles("no hero selected".to_string()))?;
        self.state.move_hero(id, direction)?;
        Ok(self.selected_hero_position())
    }

    /// Places a resource-control rod under the selected hero.
    ///
    /// # Errors
    /// Returns any engine placement error, or if no hero is currently selected.
    pub fn place_resource_rod(&mut self) -> Result<MapCoord, EngineError> {
        let id = self
            .selected_hero_id
            .ok_or_else(|| EngineError::InvalidTiles("no hero selected".to_string()))?;
        self.state.place_resource_rod(id)?;
        Ok(self.selected_hero_position())
    }

    /// Cycles to the next living player-controlled hero.
    ///
    /// If the current hero is the only living player hero, it stays selected.
    /// Does nothing if no hero is currently selected.
    pub fn cycle_selected_hero(&mut self) {
        let Some(id) = self.selected_hero_id else {
            return;
        };
        let player_team = self.state.hero(id).map(|h| h.get_team_id());
        if let Some(team_id) = player_team
            && let Some(next) = self.state.get_next_hero(team_id)
        {
            self.selected_hero_id = Some(next);
            self.state.set_active_hero(team_id, Some(next));
        }
    }

    /// Ends the current team's turn, advances to the next team, and selects
    /// its first hero.
    ///
    /// If the next team has no heroes yet (e.g. player needs to hire one),
    /// `selected_hero_id` becomes `None`.
    pub fn end_turn(&mut self) -> Result<String, EngineError> {
        // Finish the current team's turn (reset movement, increment turn counter).
        self.state.on_turn().map_err(|e| EngineError::InvalidTiles(e.to_string()))?;

        // Rotate to the next player-controlled team.
        let next_team = self
            .state
            .get_next_active_team()
            .map_err(|e| EngineError::InvalidTiles(e.to_string()))?;

        // Start the new team's turn (reset movement, increment turn counter).
        self.state.on_turn().map_err(|e| EngineError::InvalidTiles(e.to_string()))?;

        // Grant the new team its start-of-turn income (base gold + mine output).
        self.state.grant_turn_income(next_team);

        // Select the first hero of the new team, if one exists.
        self.selected_hero_id = self.state.get_next_hero(next_team);
        if let Some(next) = self.selected_hero_id {
            self.state.set_active_hero(next_team, Some(next));
        }

        Ok(self.summary())
    }

    /// Returns a short one-line status summary for HUD rendering.
    pub fn summary(&self) -> String {
        let Some(id) = self.selected_hero_id else {
            return "No hero – hire one at a city entrance".to_string();
        };
        let Some(hero) = self.state.hero(id) else {
            return "?".to_string();
        };
        let team_heroes = self.state.get_team_alive_heroes_ids(hero.get_team_id());
        let hero_index =
            team_heroes.iter().position(|&hid| hid == id).unwrap_or(0).saturating_add(1);
        let total_team = team_heroes.len();

        format!(
            "{} ({}/{}) MOV:{}/{} HP:{}/{} @{},{}",
            hero.get_name(),
            hero_index,
            total_team,
            hero.get_mov_remaining(),
            hero.get_mov(),
            hero.get_hp(),
            hero.get_max_hp(),
            hero.get_position().x,
            hero.get_position().y
        )
    }
}

fn select_hero(state: &GameState) -> Option<HeroId> {
    let active_team = state.get_active_team_id().ok().copied();
    active_team
        .and_then(|team_id| state.get_active_hero(team_id))
        .or_else(|| active_team.and_then(|team_id| state.get_next_hero(team_id)))
        .or_else(|| state.living_heroes(true).first().map(|hero| hero.get_id()))
    // Do NOT fall back to AI heroes — if the player team has no heroes,
    // selected_hero_id stays None and the UI shows "hire hero" prompt.
}
