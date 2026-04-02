//! Game state and turn manager.
//!
//! [`GameState`] is the single source of truth for a running game session.
//! It owns the map, all heroes, the turn counter, and the score board.
//!
//! ## Turn loop
//! ```text
//! // At the start of each team's turn:
//! state.on_turn();           // increments team turn counter, resets that team's movement
//! // player/AI issues move_hero / attack_hero calls
//! // When all player teams have acted:
//! state.advance_turn();      // resets AI-team movement, awards survival points, bumps global turn
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::combat::{self, CombatResult};
use crate::error::Error;
#[allow(unused_imports)] // Team is re-exported for tests via `use super::*`
use crate::hero::{Hero, Team, TeamId, TeamInfo};
use crate::map::game_map::{Direction, GameMap, MapCoord};
use crate::map::tile::Tiles;
use crate::score::{ScoreBoard, ScoreEvent};

// ─── TurnEvent ────────────────────────────────────────────────────────────────

/// An event that occurred during the current turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnEvent {
    /// A hero moved to a new tile.
    HeroMoved {
        hero_id: u32,
        from: MapCoord,
        to: MapCoord,
    },
    /// A hero visited a point of interest and triggered a score event.
    PoiVisited { hero_id: u32, coord: MapCoord },
    /// City ownership changed: `team_id` is None when the city becomes neutral.
    CityOwnerChanged { coord: MapCoord, team_id: Option<TeamId> },
    /// A hero engaged and resolved combat with an enemy.
    CombatResolved {
        attacker_id: u32,
        defender_id: u32,
        result: CombatResult,
    },
    /// A hero was defeated and removed from the map.
    HeroDefeated { hero_id: u32 },
    /// The turn counter advanced.
    TurnAdvanced { turn: u32 },
    /// A team's per-team turn counter advanced (emitted at the start of that team's turn).
    TeamTurnStarted { team_id: TeamId, turn: u32 },
}

// ─── GameState ────────────────────────────────────────────────────────────────

/// Complete state of a running game session.
pub struct GameState {
    /// The assembled game map.
    pub map: GameMap,
    /// All heroes currently on the map (player and enemy, living and dead).
    pub heroes: Vec<Hero>,
    /// Current turn number (starts at 1).
    turn: u32,
    /// Accumulated score.
    pub score: ScoreBoard,
    /// City tile ownership: maps each occupied city [`MapCoord`] to the owning
    /// team id.  Absence from the map means the city is neutral.
    pub city_owners: HashMap<MapCoord, TeamId>,
    /// All teams in the game (player-controlled and AI).
    pub teams: Vec<TeamInfo>,
    /// Last active hero for each team. Used to restore selection when switching teams.
    active_hero: HashMap<TeamId, Option<u32>>,
    /// The team whose turn it is to act (index into `teams`).
    active_team_idx: usize,
}

impl GameState {
    /// Creates a new game session at turn 1 with default teams (Red, Blue, Enemy).
    pub fn new(map: GameMap, heroes: Vec<Hero>) -> Self {
        Self {
            map,
            heroes,
            turn: 1,
            score: ScoreBoard::new(),
            city_owners: HashMap::new(),
            teams: vec![TeamInfo::red(), TeamInfo::blue()],
            active_hero: HashMap::new(),
            active_team_idx: 0,
        }
    }

    /// Creates a new game session with custom teams.
    pub fn with_teams(map: GameMap, heroes: Vec<Hero>, teams: Vec<TeamInfo>) -> Self {
        Self {
            map,
            heroes,
            turn: 1,
            score: ScoreBoard::new(),
            city_owners: HashMap::new(),
            teams,
            active_hero: HashMap::new(),
            active_team_idx: 0,
        }
    }

    pub fn get_turn(&self) -> u32 {
        self.turn
    }

    /// Returns the currently active team info.
    pub fn get_active_team(&self) -> &TeamInfo {
        &self.teams[self.active_team_idx]
    }

    /// Returns the currently active team id.
    pub fn get_active_team_id(&self) -> TeamId {
        self.teams[self.active_team_idx].id
    }

    /// Returns all player-controlled teams.
    pub fn player_teams(&self) -> impl Iterator<Item = &TeamInfo> {
        self.teams.iter().filter(|t| t.player_controlled)
    }

    /// Returns team info by id.
    pub fn get_team(&self, id: TeamId) -> Option<&TeamInfo> {
        self.teams.iter().find(|t| t.id == id)
    }

    /// Advances to the next player team.
    ///
    /// Returns `true` if the global turn advanced (all player teams completed their turns).
    pub fn get_next_active_team(&mut self) -> bool {
        // Find next player-controlled team
        let start_idx = self.active_team_idx;
        loop {
            self.active_team_idx = (self.active_team_idx + 1) % self.teams.len();
            if self.teams[self.active_team_idx].player_controlled {
                break;
            }
            if self.active_team_idx == start_idx {
                // Wrapped around - advance global turn
                break;
            }
        }

        // Check if we've cycled through all player teams
        let player_teams: Vec<usize> = self.teams.iter()
            .enumerate()
            .filter(|(_, t)| t.player_controlled)
            .map(|(i, _)| i)
            .collect();

        if self.active_team_idx == player_teams[0] {
            // We're back at the first player team - advance global turn
            true
        } else {
            false
        }
    }

    /// Resets to the first player-controlled team.
    pub fn reset_active_team(&mut self) {
        for (i, team) in self.teams.iter().enumerate() {
            if team.player_controlled {
                self.active_team_idx = i;
                return;
            }
        }
        self.active_team_idx = 0;
    }

    /// Begins the active team's turn:
    /// 1. Increments their per-team turn counter.
    /// 2. Resets movement points for all living heroes that belong to the active team.
    ///
    /// Must be called at the start of each team's turn, including the very first
    /// turn after game initialisation (so that turn 0 → 1 fires the same event as
    /// any subsequent team-turn start).
    pub fn on_turn(&mut self) -> Vec<TurnEvent> {
        let team = &mut self.teams[self.active_team_idx];
        team.turn += 1;
        let team_id = team.id;
        let turn = team.turn;
        let team_name = team.name.to_lowercase();

        for hero in self.heroes.iter_mut().filter(|h| h.is_alive() && h.team.name == team_name) {
            hero.reset_movement();
        }

        vec![TurnEvent::TeamTurnStarted { team_id, turn }]
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Returns living heroes whose team has the given `player_controlled` value.
    ///
    /// Pass `true` to get all player-controlled heroes, `false` for AI-controlled ones.
    pub fn living_heroes(&self, player_controlled: bool) -> Vec<&Hero> {
        self.heroes
            .iter()
            .filter(|h| h.team.player_controlled == player_controlled && h.is_alive())
            .collect()
    }

    /// Returns a reference to a hero by id, or `None`.
    pub fn hero(&self, id: u32) -> Option<&Hero> {
        self.heroes.iter().find(|h| h.id == id)
    }

    /// Returns a reference to the living hero at `pos`, or `None`.
    pub fn hero_at(&self, pos: MapCoord) -> Option<&Hero> {
        self.heroes
            .iter()
            .find(|h| h.is_alive() && h.position == pos)
    }

    /// Returns the team id that owns the city at `coord`, or `None` if neutral.
    pub fn city_owner(&self, coord: MapCoord) -> Option<TeamId> {
        self.city_owners.get(&coord).copied()
    }

    /// Returns the last active hero ID for `team_id`, or `None` if not set.
    pub fn get_active_hero(&self, team_id: TeamId) -> Option<u32> {
        self.active_hero.get(&team_id).copied().flatten()
    }

    /// Sets the active hero for `team_id`.
    pub fn set_active_hero(&mut self, team_id: TeamId, hero_id: Option<u32>) {
        self.active_hero.insert(team_id, hero_id);
    }

    /// Returns the next hero for `team_id` after the current active one.
    ///
    /// If no active hero is set or the active hero is dead/wrong team,
    /// returns the first living hero of the team.
    /// Returns `None` if the team has no living heroes.
    pub fn get_next_hero(&self, team_id: TeamId) -> Option<u32> {
        let team_heroes: Vec<u32> = self
            .heroes
            .iter()
            .filter(|h| {
                // For now, match by team name until Hero stores TeamId
                self.teams.iter().any(|t| t.id == team_id && t.name.to_lowercase() == h.team.name)
                    && h.is_alive()
            })
            .map(|h| h.id)
            .collect();

        if team_heroes.is_empty() {
            return None;
        }

        let current = self.get_active_hero(team_id);
        let current_idx = current.and_then(|id| {
            team_heroes.iter().position(|&hid| hid == id)
        });

        let next_idx = current_idx
            .map(|idx| (idx + 1) % team_heroes.len())
            .unwrap_or(0);

        team_heroes.get(next_idx).copied()
    }

    /// Clears all active hero selections.
    pub fn clear_active_heroes(&mut self) {
        self.active_hero.clear();
    }

    /// Sets the owning team for the city at `coord` and all connected city tiles.
    ///
    /// Uses BFS to flood all adjacent `City` / `CityEntrance` tiles so that the
    /// entire city complex is claimed at once.  Pass `None` to make the city neutral.
    ///
    /// # Returns
    /// The full list of tile coordinates whose ownership was updated.
    pub fn set_city_owner(&mut self, coord: MapCoord, team_id: Option<TeamId>) -> Vec<MapCoord> {
        let connected = flood_city(&self.map, coord);
        for &c in &connected {
            match team_id {
                Some(id) => self.city_owners.insert(c, id),
                None => self.city_owners.remove(&c),
            };
        }
        connected
    }

    /// Finds TeamId by team name (case-insensitive). Returns None if not found.
    pub fn team_id_by_name(&self, name: &str) -> Option<TeamId> {
        let name_lower = name.to_lowercase();
        self.teams.iter()
            .find(|t| t.name.to_lowercase() == name_lower)
            .map(|t| t.id)
    }

    /// Finds team name by TeamId. Returns None if not found.
    pub fn team_name_by_id(&self, id: TeamId) -> Option<&str> {
        self.teams.iter()
            .find(|t| t.id == id)
            .map(|t| t.name.as_str())
    }

    // ── Actions ───────────────────────────────────────────────────────────────

    /// Moves hero `hero_id` one step in `direction`, spending movement points.
    ///
    /// The target tile is computed from the hero's current position. Movement is
    /// rejected if the tile is out of bounds, impassable, occupied by another
    /// hero, or the hero has insufficient movement points.
    ///
    /// After moving, checks whether the destination is a point of interest
    /// and records score events accordingly.
    ///
    /// # Errors
    /// - [`Error::OutOfBounds`]      — hero id not found or step leaves the map
    /// - [`Error::NoMovementPoints`] — hero has no movement points remaining
    /// - [`Error::ImpassableTile`]   — target tile is not passable terrain
    /// - [`Error::OccupiedTile`]     — target tile is occupied by another hero
    pub fn move_hero(
        &mut self,
        hero_id: u32,
        direction: Direction,
    ) -> Result<Vec<TurnEvent>, Error> {
        let idx = self.hero_index(hero_id)?;
        let start = self.heroes[idx].position;

        // Ensure the hero has at least one movement point before computing cost.
        if self.heroes[idx].mov_remaining == 0 {
            return Err(Error::NoMovementPoints { hero_id });
        }

        // Compute the target coordinate (bounds-checked).
        let w = self.map.tile_width();
        let h = self.map.tile_height();
        let target = direction.apply(start, w, h).ok_or_else(|| {
            Error::OutOfBounds(format!(
                "direction {direction:?} from ({}, {}) leaves the map",
                start.x, start.y
            ))
        })?;

        // Check passability.
        let tile = self.map.get_tile(target)?;
        if !tile.kind.is_passable() {
            return Err(Error::ImpassableTile {
                x: target.x,
                y: target.y,
            });
        }

        // Check occupancy.
        if let Some(other) = self.hero_at(target) {
            if other.id != hero_id {
                return Err(Error::OccupiedTile {
                    x: target.x,
                    y: target.y,
                });
            }
        }

        // Deduct the movement cost for entering the target tile.
        let cost = (1i32 + tile.kind.movement_cost_modifier()).max(1) as u32;
        if self.heroes[idx].mov_remaining < cost {
            return Err(Error::NoMovementPoints { hero_id });
        }

        self.heroes[idx].mov_remaining -= cost;
        self.heroes[idx].position = target;

        let mut events = vec![TurnEvent::HeroMoved {
            hero_id,
            from: start,
            to: target,
        }];

        // Trigger POI score events.
        if let Ok(tile) = self.map.get_tile(target) {
            if tile.kind.is_point_of_interest() {
                events.push(TurnEvent::PoiVisited {
                    hero_id,
                    coord: target,
                });
                match tile.kind {
                    Tiles::City | Tiles::CityEntrance => {
                        self.score.record(ScoreEvent::CityCapture { city: target });
                    }
                    Tiles::Gold => {
                        self.score
                            .record(ScoreEvent::GoldCollected { coord: target });
                    }
                    Tiles::Resource => {
                        self.score
                            .record(ScoreEvent::ResourceCollected { coord: target });
                    }
                    _ => {}
                }
            }

            // City ownership: entering any City/CityEntrance tile claims the
            // entire connected city complex for the hero's team.
            // Emit CityOwnerChanged for every tile whose owner actually changes.
            if matches!(tile.kind, Tiles::City | Tiles::CityEntrance) {
                // Find TeamId by hero's team name
                let team_id = self.team_id_by_name(&self.heroes[idx].team.name);
                if let Some(tid) = team_id {
                    for coord in flood_city(&self.map, target) {
                        let already_owned = self
                            .city_owners
                            .get(&coord)
                            .map(|&o| o == tid)
                            .unwrap_or(false);
                        if !already_owned {
                            self.city_owners.insert(coord, tid);
                            events.push(TurnEvent::CityOwnerChanged {
                                coord,
                                team_id: Some(tid),
                            });
                        }
                    }
                }
            }
        }

        Ok(events)
    }

    /// Initiates combat between hero `attacker_id` and hero `defender_id`.
    ///
    /// Each hero's personal RNG (stored on the hero itself) is consumed to
    /// compute their attack roll — no external RNG is required.
    ///
    /// Applies damage to both heroes.  Defeated heroes remain in the list
    /// but `is_alive()` returns `false`.
    ///
    /// # Errors
    /// Returns [`Error::OutOfBounds`] if either hero id is not found.
    pub fn attack_hero(
        &mut self,
        attacker_id: u32,
        defender_id: u32,
    ) -> Result<Vec<TurnEvent>, Error> {
        let att_idx = self.hero_index(attacker_id)?;
        let def_idx = self.hero_index(defender_id)?;

        let (attacker, defender) = two_mut(&mut self.heroes, att_idx, def_idx);
        let result = combat::resolve_combat(attacker, defender);

        self.heroes[att_idx].take_damage(result.attacker_damage);
        self.heroes[def_idx].take_damage(result.defender_damage);

        let mut events = vec![TurnEvent::CombatResolved {
            attacker_id,
            defender_id,
            result: result.clone(),
        }];

        if !result.defender_survived {
            self.score.record(ScoreEvent::EnemyDefeated {
                enemy_id: defender_id,
            });
            events.push(TurnEvent::HeroDefeated {
                hero_id: defender_id,
            });
        }
        if !result.attacker_survived {
            events.push(TurnEvent::HeroDefeated {
                hero_id: attacker_id,
            });
        }

        Ok(events)
    }

    /// Advances the global turn: resets movement for non-player-controlled (AI) heroes
    /// and awards one survival point.
    ///
    /// Movement for player-controlled heroes is reset per-team in [`GameState::on_turn`].
    pub fn advance_turn(&mut self) -> Vec<TurnEvent> {
        for hero in self.heroes.iter_mut().filter(|h| h.is_alive() && !h.team.player_controlled) {
            hero.reset_movement();
        }
        self.score.record(ScoreEvent::TurnSurvived);
        self.turn += 1;
        vec![TurnEvent::TurnAdvanced { turn: self.turn }]
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn hero_index(&self, id: u32) -> Result<usize, Error> {
        self.heroes
            .iter()
            .position(|h| h.id == id)
            .ok_or_else(|| Error::OutOfBounds(format!("hero {id} not found")))
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Returns all tile coordinates that form a single connected city complex
/// starting from `start`, using BFS over adjacent `City` / `CityEntrance` tiles.
///
/// If `start` is not a city tile, only `start` itself is returned so that callers
/// can still record the single-tile ownership without triggering a full flood.
fn flood_city(map: &GameMap, start: MapCoord) -> Vec<MapCoord> {
    let is_city = map
        .get_tile(start)
        .map(|t| matches!(t.kind, Tiles::City | Tiles::CityEntrance))
        .unwrap_or(false);

    if !is_city {
        return vec![start];
    }

    let w = map.tile_width();
    let h = map.tile_height();
    let mut visited: HashSet<MapCoord> = HashSet::new();
    let mut queue: VecDeque<MapCoord> = VecDeque::new();
    let mut result: Vec<MapCoord> = Vec::new();

    visited.insert(start);
    queue.push_back(start);

    while let Some(coord) = queue.pop_front() {
        result.push(coord);

        for dir in [
            Direction::North,
            Direction::East,
            Direction::South,
            Direction::West,
        ] {
            if let Some(neighbor) = dir.apply(coord, w, h) {
                if !visited.contains(&neighbor) {
                    if map
                        .get_tile(neighbor)
                        .map(|t| matches!(t.kind, Tiles::City | Tiles::CityEntrance))
                        .unwrap_or(false)
                    {
                        visited.insert(neighbor);
                        queue.push_back(neighbor);
                    }
                }
            }
        }
    }

    result
}

/// Returns two mutable references into a slice at distinct indices `i` and `j`.
///
/// Panics if `i == j`.
fn two_mut<T>(slice: &mut [T], i: usize, j: usize) -> (&mut T, &mut T) {
    assert!(i != j, "two_mut: indices must be distinct");
    if i < j {
        let (left, right) = slice.split_at_mut(j);
        (&mut left[i], &mut right[0])
    } else {
        let (left, right) = slice.split_at_mut(i);
        (&mut right[0], &mut left[j])
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::tile::Tile;
    use crate::rng::SeededRng;

    fn meadow_map(w: u32, h: u32) -> GameMap {
        let tiles = vec![
            Tile {
                kind: Tiles::Meadow
            };
            (w * h) as usize
        ];
        GameMap::new(w, h, tiles, [0u8; 32]).unwrap()
    }

    fn base_rng() -> SeededRng {
        SeededRng::new("test-session")
    }

    fn player(id: u32, pos: MapCoord) -> Hero {
        // spd=10 → mov = 30
        Hero::new(
            id,
            "Player",
            100,
            20,
            10,
            10,
            pos,
            Team::player(),
            base_rng().derive_for_hero(id),
        )
    }

    fn enemy(id: u32, pos: MapCoord) -> Hero {
        // spd=5 → mov = 25
        Hero::new(
            id,
            "Enemy",
            30,
            10,
            5,
            5,
            pos,
            Team::enemy(),
            base_rng().derive_for_hero(id),
        )
    }

    #[test]
    fn move_hero_updates_position_and_spends_movement() {
        let map = meadow_map(10, 10);
        let h = player(1, MapCoord::new(0, 0));
        let mut state = GameState::new(map, vec![h]);

        // Move East three times — each step costs 1 movement point on Meadow.
        state.move_hero(1, Direction::East).unwrap();
        state.move_hero(1, Direction::East).unwrap();
        let events = state.move_hero(1, Direction::East).unwrap();
        assert_eq!(state.hero(1).unwrap().position, MapCoord::new(3, 0));
        assert_eq!(state.hero(1).unwrap().mov_remaining, 27); // 30 - 3 = 27
        assert!(events
            .iter()
            .any(|e| matches!(e, TurnEvent::HeroMoved { .. })));
    }

    #[test]
    fn move_hero_with_zero_budget_returns_error() {
        let map = meadow_map(10, 10);
        let mut h = player(1, MapCoord::new(0, 0));
        h.mov_remaining = 0;
        let mut state = GameState::new(map, vec![h]);
        let result = state.move_hero(1, Direction::East);
        assert!(matches!(result, Err(Error::NoMovementPoints { .. })));
    }

    #[test]
    fn move_hero_into_impassable_returns_error() {
        use crate::map::tile::Tile;
        // Build a map where (1, 0) is a Mountain.
        let mut tiles = vec![
            Tile {
                kind: Tiles::Meadow
            };
            9
        ];
        tiles[1] = Tile {
            kind: Tiles::Mountain,
        };
        let map = GameMap::new(3, 3, tiles, [0u8; 32]).unwrap();
        let h = player(1, MapCoord::new(0, 0));
        let mut state = GameState::new(map, vec![h]);
        let result = state.move_hero(1, Direction::East);
        assert!(matches!(result, Err(Error::ImpassableTile { .. })));
    }

    #[test]
    fn move_hero_out_of_bounds_returns_error() {
        let map = meadow_map(5, 5);
        let h = player(1, MapCoord::new(0, 0));
        let mut state = GameState::new(map, vec![h]);
        // Moving North from (0,0) should go out of bounds.
        let result = state.move_hero(1, Direction::North);
        assert!(matches!(result, Err(Error::OutOfBounds(_))));
    }

    #[test]
    fn advance_turn_increments_global_turn_and_resets_ai_movement() {
        let map = meadow_map(5, 5);
        let mut p = player(1, MapCoord::new(0, 0));
        let mut e = enemy(2, MapCoord::new(1, 0));
        p.mov_remaining = 0;
        e.mov_remaining = 0;
        let mut state = GameState::new(map, vec![p, e]);
        state.advance_turn();
        assert_eq!(state.turn, 2);
        // Player hero is NOT reset by advance_turn — that's on_turn's job.
        assert_eq!(state.hero(1).unwrap().mov_remaining, 0);
        // Enemy hero IS reset by advance_turn.
        assert_eq!(state.hero(2).unwrap().mov_remaining, 25); // spd=5 → mov=25
    }

    #[test]
    fn on_turn_resets_active_team_movement() {
        let map = meadow_map(5, 5);
        // Hero must belong to the active team ("red" in GameState::new default).
        let mut h = Hero::new(
            1, "Hero", 100, 20, 10, 10,
            MapCoord::new(0, 0),
            Team::red(),
            base_rng().derive_for_hero(1),
        );
        h.mov_remaining = 0;
        let mut state = GameState::new(map, vec![h]);
        state.on_turn();
        assert_eq!(state.hero(1).unwrap().mov_remaining, 30); // spd=10 → mov=30
    }

    #[test]
    fn attack_hero_applies_damage() {
        let map = meadow_map(5, 5);
        let heroes = vec![
            player(1, MapCoord::new(0, 0)),
            enemy(2, MapCoord::new(1, 0)),
        ];
        let mut state = GameState::new(map, heroes);
        state.attack_hero(1, 2).unwrap();
        // Enemy should have taken damage (strong player vs weak enemy)
        assert!(state.hero(2).unwrap().hp < 30);
    }

    #[test]
    fn defeated_enemy_awards_score() {
        let map = meadow_map(5, 5);
        // Player with overwhelming stats, enemy with minimal hp
        let base = base_rng();
        let p = Hero::new(
            1,
            "P",
            100,
            200,
            0,
            10,
            MapCoord::new(0, 0),
            Team::player(),
            base.derive_for_hero(1),
        );
        let e = Hero::new(
            2,
            "E",
            1,
            1,
            0,
            1,
            MapCoord::new(1, 0),
            Team::enemy(),
            base.derive_for_hero(2),
        );
        let mut state = GameState::new(map, vec![p, e]);
        state.attack_hero(1, 2).unwrap();
        assert!(state.score.total() > 0);
    }

    #[test]
    fn living_heroes_excludes_dead() {
        let map = meadow_map(5, 5);
        let mut e = enemy(2, MapCoord::new(1, 0));
        e.take_damage(30); // kill
        let state = GameState::new(map, vec![player(1, MapCoord::new(0, 0)), e]);
        assert_eq!(state.living_heroes(false).len(), 0);
        assert_eq!(state.living_heroes(true).len(), 1);
    }

    #[test]
    fn on_turn_increments_active_team_counter() {
        let map = meadow_map(5, 5);
        let mut state = GameState::new(map, vec![]);
        // All teams start at 0.
        for team in &state.teams {
            assert_eq!(team.turn, 0);
        }
        // First team's turn begins.
        let events = state.on_turn();
        let active_id = state.get_active_team_id();
        assert_eq!(state.get_active_team().turn, 1);
        assert!(matches!(
            events[0],
            TurnEvent::TeamTurnStarted { team_id, turn: 1 } if team_id == active_id
        ));
    }

    #[test]
    fn on_turn_each_team_has_own_counter() {
        let map = meadow_map(5, 5);
        let teams = vec![TeamInfo::red(), TeamInfo::blue()];
        let mut state = GameState::with_teams(map, vec![], teams);

        // Simulate: first team begins turn 1.
        state.on_turn();
        assert_eq!(state.teams[0].turn, 1);
        assert_eq!(state.teams[1].turn, 0);

        // Switch to second team and begin its turn 1.
        state.get_next_active_team();
        state.on_turn();
        assert_eq!(state.teams[0].turn, 1);
        assert_eq!(state.teams[1].turn, 1);

        // Cycle back to first team, begin its turn 2.
        state.get_next_active_team();
        state.on_turn();
        assert_eq!(state.teams[0].turn, 2);
        assert_eq!(state.teams[1].turn, 1);
    }
}
