//! Engine-backed game session wrapper used by the T-Deck frontend.

use alloc::{format, string::String};

use rpg_engine::Direction;
use rpg_engine::error::Error as EngineError;
use rpg_engine::game_state::GameState;
use rpg_engine::hero::{Hero, HeroId};
use rpg_engine::map::game_map::{GameMap, MapCoord};
use rpg_engine::spawn;
use rpg_engine::team::Team;

/// Runtime game session stored by the map-view screen.
pub struct GameSession {
    map_name: String,
    state: GameState,
    selected_hero_id: HeroId,
}

impl GameSession {
    /// Creates a new engine session from a loaded map.
    ///
    /// The session starts with one player-controlled team and one hero placed
    /// on the best city entrance spawn available on the map.
    pub fn new(map_name: String, map: GameMap) -> Result<Self, EngineError> {
        let spawn = spawn::find_city_entrance_spawns(&map, 1)
            .first()
            .copied()
            .map(Ok)
            .unwrap_or_else(|| spawn::find_player_spawn(&map))?;

        let mut state = GameState::new(map, &map_name);
        let team_id = state.add_team(Team::red());
        let hero_id = state.add_hero(Hero::new(
            0,
            "Герой",
            100,
            20,
            10,
            15,
            spawn,
            team_id,
        ));
        state.set_city_owner(spawn, Some(team_id));
        let _ = state.on_turn();

        Ok(Self {
            map_name,
            state,
            selected_hero_id: hero_id,
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
