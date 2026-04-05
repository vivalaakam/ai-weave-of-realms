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

use alloc::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    format,
    vec,
    vec::Vec,
};
use serde::{Deserialize, Serialize};

use crate::combat::{self, CombatResult};
use crate::error::Error;
use crate::game_error::GameError;
use crate::hero::{Hero, HeroId, TeamId};
use crate::map::game_map::{Direction, GameMap, MapCoord};
use crate::map::tile::Tiles;
use crate::rng::SeededRng;
use crate::score::{ScoreBoard, ScoreEvent};
use crate::team::Team;

// ─── TurnEvent ────────────────────────────────────────────────────────────────

/// An event that occurred during the current turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnEvent {
    /// A hero moved to a new tile.
    HeroMoved {
        hero_id: HeroId,
        from: MapCoord,
        to: MapCoord,
    },
    /// A hero visited a point of interest and triggered a score event.
    PoiVisited { hero_id: HeroId, coord: MapCoord },
    /// City ownership changed: `team_id` is None when the city becomes neutral.
    CityOwnerChanged {
        coord: MapCoord,
        team_id: Option<TeamId>,
    },
    /// A hero engaged and resolved combat with an enemy.
    CombatResolved {
        attacker_id: HeroId,
        defender_id: HeroId,
        result: CombatResult,
    },
    /// A hero was defeated and removed from the map.
    HeroDefeated { hero_id: HeroId },
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
    heroes: BTreeMap<HeroId, Hero>,
    /// Accumulated score.
    pub score: ScoreBoard,
    /// City tile ownership: maps each occupied city [`MapCoord`] to the owning
    /// team id.  Absence from the map means the city is neutral.
    pub city_owners: BTreeMap<MapCoord, TeamId>,
    /// All teams in the game (player-controlled and AI).
    teams: BTreeMap<TeamId, Team>,
    teams_order: VecDeque<TeamId>,
    /// Last active hero for each team. Used to restore selection when switching teams.
    active_hero: BTreeMap<TeamId, Option<HeroId>>,
    /// Session RNG, seeded at construction time.
    rng: SeededRng,
    hero_pointer: HeroId,
}

impl GameState {
    /// Creates a new empty game session with a map and a seed for the session RNG.
    ///
    /// Add teams via [`add_team`](Self::add_team) and heroes via
    /// [`add_hero`](Self::add_hero) before the first turn.
    pub fn new(map: GameMap, seed: impl AsRef<str>) -> Self {
        Self {
            map,
            heroes: BTreeMap::new(),
            score: ScoreBoard::new(),
            city_owners: BTreeMap::new(),
            teams: BTreeMap::new(),
            active_hero: BTreeMap::new(),
            teams_order: VecDeque::new(),
            rng: SeededRng::new(seed.as_ref()),
            hero_pointer: 0,
        }
    }

    /// Adds a hero to the session, auto-assigning `id = heroes.len()`.
    ///
    /// Returns the assigned [`HeroId`].
    pub fn add_hero(&mut self, mut hero: Hero) -> HeroId {
        let next_hero_id = self.hero_pointer;
        hero.reset(next_hero_id, &self.rng);
        self.heroes.insert(next_hero_id, hero);
        self.hero_pointer += 1;
        next_hero_id
    }

    pub fn get_total_heroes(&self) -> usize {
        self.heroes.len()
    }

    /// Adds a team to the session, auto-assigning `id = teams.len()`.
    ///
    /// Returns the assigned [`TeamId`].
    pub fn add_team(&mut self, mut team: Team) -> TeamId {
        team.reset_id(self.teams.len() as TeamId);
        let id = team.get_id();
        if team.is_player_controlled() {
            self.teams_order.push_back(id);
        }
        self.teams.insert(id, team);
        id
    }

    pub fn get_turn(&self) -> u32 {
        self.get_active_team().map(|t| t.get_turn()).unwrap_or(0)
    }

    /// Returns the currently active team info.
    pub fn get_active_team(&self) -> Result<&Team, GameError> {
        let active_team = self.get_active_team_id()?;
        self.teams
            .get(active_team)
            .ok_or(GameError::ActiveTeamNotFound(*active_team))
    }

    /// Returns the currently active team id.
    pub fn get_active_team_id(&self) -> Result<&TeamId, GameError> {
        self.teams_order.front().ok_or(GameError::NoActiveTeam)
    }

    /// Returns all player-controlled teams.
    pub fn player_teams(&self) -> impl Iterator<Item = &Team> {
        self.teams
            .iter()
            .filter(|(_, t)| t.is_player_controlled())
            .map(|(_, t)| t)
    }

    /// Returns team info by id.
    pub fn get_team(&self, id: TeamId) -> Option<&Team> {
        self.teams.get(&id)
    }

    /// Returns the number of teams.
    pub fn teams_count(&self) -> usize {
        self.teams.len()
    }

    /// Returns the first non-player-controlled (AI) team id.
    pub fn enemy_team_id(&self) -> Option<TeamId> {
        self.teams
            .values()
            .find(|t| !t.is_player_controlled())
            .map(|t| t.get_id())
    }

    pub fn get_team_alive_heroes_ids(&self, team_id: TeamId) -> Vec<HeroId> {
        self.heroes
            .iter()
            .filter_map(|(&id, h)| {
                if h.team_id == team_id && h.is_alive() {
                    Some(id)
                } else {
                    None
                }
            })
            .collect::<Vec<HeroId>>()
    }

    /// Advances to the next player team.
    ///
    /// Returns `true` if all player teams have acted (full round completed).
    pub fn get_next_active_team(&mut self) -> Result<TeamId, GameError> {
        let Some(current) = self.teams_order.pop_front() else {
            return Err(GameError::NoActiveTeam);
        };
        self.teams_order.push_back(current);
        let Some(&next) = self.teams_order.front() else {
            return Err(GameError::NextActiveTeam);
        };
        Ok(next)
    }

    /// Begins the active team's turn:
    /// 1. Increments their per-team turn counter.
    /// 2. Resets movement points for all living heroes that belong to the active team.
    ///
    /// Must be called at the start of each team's turn, including the very first
    /// turn after game initialisation (so that turn 0 → 1 fires the same event as
    /// any subsequent team-turn start).
    pub fn on_turn(&mut self) -> Result<TurnEvent, GameError> {
        let active_team_id = *self.get_active_team_id()?;
        let Some(team) = self.teams.get_mut(&active_team_id) else {
            return Err(GameError::ActiveTeamNotFound(active_team_id));
        };

        team.increment_turn();
        let team_id = team.get_id();
        let turn = team.get_turn();

        for (_, hero) in self
            .heroes
            .iter_mut()
            .filter(|(_, h)| h.is_alive() && h.team_id == team_id)
        {
            hero.reset_movement();
        }

        Ok(TurnEvent::TeamTurnStarted { team_id, turn })
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Returns `true` if the team with `team_id` is player-controlled.
    pub fn is_player_controlled(&self, team_id: TeamId) -> bool {
        let Some(t) = self.teams.get(&team_id) else {
            return false;
        };

        t.is_player_controlled()
    }

    /// Returns living heroes whose team has the given `player_controlled` value.
    ///
    /// Pass `true` to get all player-controlled heroes, `false` for AI-controlled ones.
    pub fn living_heroes(&self, player_controlled: bool) -> Vec<&Hero> {
        self.heroes
            .values()
            .filter(|h| self.is_player_controlled(h.team_id) == player_controlled && h.is_alive())
            .collect()
    }

    /// Returns a reference to a hero by id, or `None`.
    pub fn hero(&self, id: HeroId) -> Option<&Hero> {
        self.heroes.get(&id)
    }

    /// Returns a reference to the living hero at `pos`, or `None`.
    pub fn hero_at(&self, pos: MapCoord) -> Option<&Hero> {
        self.heroes
            .values()
            .find(|h| h.is_alive() && h.position == pos)
    }

    /// Returns the team id that owns the city at `coord`, or `None` if neutral.
    pub fn city_owner(&self, coord: MapCoord) -> Option<TeamId> {
        self.city_owners.get(&coord).copied()
    }

    /// Returns the last active hero ID for `team_id`, or `None` if not set.
    pub fn get_active_hero(&self, team_id: TeamId) -> Option<HeroId> {
        self.active_hero.get(&team_id).copied().flatten()
    }

    /// Sets the active hero for `team_id`.
    pub fn set_active_hero(&mut self, team_id: TeamId, hero_id: Option<HeroId>) {
        self.active_hero.insert(team_id, hero_id);
    }

    /// Returns the next hero for `team_id` after the current active one.
    ///
    /// If no active hero is set or the active hero is dead/wrong team,
    /// returns the first living hero of the team.
    /// Returns `None` if the team has no living heroes.
    pub fn get_next_hero(&self, team_id: TeamId) -> Option<HeroId> {
        let team_heroes: Vec<HeroId> = self
            .heroes
            .iter()
            .filter(|(_, h)| h.team_id == team_id && h.is_alive())
            .map(|(_, h)| h.get_id())
            .collect();

        if team_heroes.is_empty() {
            return None;
        }

        let current = self.get_active_hero(team_id);
        let current_idx = current.and_then(|id| team_heroes.iter().position(|&hid| hid == id));

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
        self.teams
            .values()
            .find(|t| t.name.to_lowercase() == name_lower)
            .map(|t| t.get_id())
    }

    /// Finds team name by TeamId. Returns None if not found.
    pub fn team_name_by_id(&self, id: TeamId) -> Option<&str> {
        self.teams.get(&id).map(|t| t.name.as_str())
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
        hero_id: HeroId,
        direction: Direction,
    ) -> Result<Vec<TurnEvent>, Error> {
        self.hero_index(hero_id)?;
        let start = self.heroes[&hero_id].position;

        // Ensure the hero has at least one movement point before computing cost.
        if self.heroes[&hero_id].mov_remaining == 0 {
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
            if other.get_id() != hero_id {
                return Err(Error::OccupiedTile {
                    x: target.x,
                    y: target.y,
                });
            }
        }

        // Deduct the movement cost for entering the target tile.
        let cost = (1i32 + tile.kind.movement_cost_modifier()).max(1) as u32;
        if self.heroes[&hero_id].mov_remaining < cost {
            return Err(Error::NoMovementPoints { hero_id });
        }

        self.heroes.get_mut(&hero_id).unwrap().mov_remaining -= cost;
        self.heroes.get_mut(&hero_id).unwrap().position = target;

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
                let tid = self.heroes[&hero_id].team_id;
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
        attacker_id: HeroId,
        defender_id: HeroId,
    ) -> Result<Vec<TurnEvent>, Error> {
        self.hero_index(attacker_id)?;
        self.hero_index(defender_id)?;

        let mut attacker = self.heroes.remove(&attacker_id).unwrap();
        let mut defender = self.heroes.remove(&defender_id).unwrap();
        let result = combat::resolve_combat(&mut attacker, &mut defender);
        attacker.take_damage(result.attacker_damage);
        defender.take_damage(result.defender_damage);
        self.heroes.insert(attacker_id, attacker);
        self.heroes.insert(defender_id, defender);

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
        let player_teams: BTreeSet<TeamId> = self
            .teams
            .values()
            .filter(|t| t.is_player_controlled())
            .map(|t| t.get_id())
            .collect();

        for (_, hero) in self
            .heroes
            .iter_mut()
            .filter(|(_, h)| h.is_alive() && !player_teams.contains(&h.team_id))
        {
            hero.reset_movement();
        }
        self.score.record(ScoreEvent::TurnSurvived);
        vec![TurnEvent::TurnAdvanced { turn: 0 }]
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn hero_index(&self, id: HeroId) -> Result<HeroId, Error> {
        if self.heroes.contains_key(&id) {
            Ok(id)
        } else {
            Err(Error::OutOfBounds(format!("hero {id} not found")))
        }
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
    let mut visited: BTreeSet<MapCoord> = BTreeSet::new();
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
                if !visited.contains(&neighbor)
                    && map
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

    result
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::tile::Tile;

    fn meadow_map(w: u32, h: u32) -> GameMap {
        let tiles = vec![
            Tile {
                kind: Tiles::Meadow
            };
            (w * h) as usize
        ];
        GameMap::new(w, h, tiles, [0u8; 32]).unwrap()
    }

    /// Creates a state with Red (0), Blue (1), Enemy (2) teams pre-registered.
    fn make_state(map: GameMap) -> GameState {
        let mut state = GameState::new(map, "test-session");
        state.add_team(Team::red());
        state.add_team(Team::blue());
        state.add_team(Team::enemy());
        state
    }

    // team_id=0 = Team::red() at index 0 (default active team); spd=10 → mov=30
    fn player(pos: MapCoord) -> Hero {
        Hero::new(0, "Player", 100, 20, 10, 10, pos, 0)
    }

    // team_id=2 = Team::enemy() (non-player-controlled); spd=5 → mov=25
    fn enemy(pos: MapCoord) -> Hero {
        Hero::new(0, "Enemy", 30, 10, 5, 5, pos, 2)
    }

    #[test]
    fn move_hero_updates_position_and_spends_movement() {
        let map = meadow_map(10, 10);
        let mut state = make_state(map);
        state.add_hero(player(MapCoord::new(0, 0)));

        // Move East three times — each step costs 1 movement point on Meadow.
        state.move_hero(0, Direction::East).unwrap();
        state.move_hero(0, Direction::East).unwrap();
        let events = state.move_hero(0, Direction::East).unwrap();
        assert_eq!(state.hero(0).unwrap().position, MapCoord::new(3, 0));
        assert_eq!(state.hero(0).unwrap().mov_remaining, 27); // 30 - 3 = 27
        assert!(events
            .iter()
            .any(|e| matches!(e, TurnEvent::HeroMoved { .. })));
    }

    #[test]
    fn move_hero_with_zero_budget_returns_error() {
        let map = meadow_map(10, 10);
        let mut state = make_state(map);
        let mut h = player(MapCoord::new(0, 0));
        h.mov_remaining = 0;
        state.add_hero(h);
        let result = state.move_hero(0, Direction::East);
        assert!(matches!(result, Err(Error::NoMovementPoints { .. })));
    }

    #[test]
    fn move_hero_into_impassable_returns_error() {
        use crate::map::tile::Tile;
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
        let mut state = make_state(map);
        state.add_hero(player(MapCoord::new(0, 0)));
        let result = state.move_hero(0, Direction::East);
        assert!(matches!(result, Err(Error::ImpassableTile { .. })));
    }

    #[test]
    fn move_hero_out_of_bounds_returns_error() {
        let map = meadow_map(5, 5);
        let mut state = make_state(map);
        state.add_hero(player(MapCoord::new(0, 0)));
        let result = state.move_hero(0, Direction::North);
        assert!(matches!(result, Err(Error::OutOfBounds(_))));
    }

    #[test]
    fn advance_turn_increments_global_turn_and_resets_ai_movement() {
        let map = meadow_map(5, 5);
        let mut state = make_state(map);
        let mut p = player(MapCoord::new(0, 0));
        p.mov_remaining = 0;
        let mut e = enemy(MapCoord::new(1, 0));
        e.mov_remaining = 0;
        let pid = state.add_hero(p); // id=0
        let eid = state.add_hero(e); // id=1
        state.advance_turn();
        // Player hero is NOT reset by advance_turn — that's on_turn's job.
        assert_eq!(state.hero(pid).unwrap().mov_remaining, 0);
        // Enemy hero IS reset by advance_turn.
        assert_eq!(state.hero(eid).unwrap().mov_remaining, 25); // spd=5 → mov=25
    }

    #[test]
    fn on_turn_resets_active_team_movement() {
        let map = meadow_map(5, 5);
        // team_id=0 = Team::red(), the first active team.
        let mut state = make_state(map);
        let mut h = player(MapCoord::new(0, 0));
        h.mov_remaining = 0;
        let hid = state.add_hero(h);
        state.on_turn().unwrap();
        assert_eq!(state.hero(hid).unwrap().mov_remaining, 30); // spd=10 → mov=30
    }

    #[test]
    fn attack_hero_applies_damage() {
        let map = meadow_map(5, 5);
        let mut state = make_state(map);
        let pid = state.add_hero(player(MapCoord::new(0, 0)));
        let eid = state.add_hero(enemy(MapCoord::new(1, 0)));
        state.attack_hero(pid, eid).unwrap();
        assert!(state.hero(eid).unwrap().hp < 30);
    }

    #[test]
    fn defeated_enemy_awards_score() {
        let map = meadow_map(5, 5);
        let mut state = make_state(map);
        let pid = state.add_hero(Hero::new(0, "P", 100, 200, 0, 10, MapCoord::new(0, 0), 0));
        let eid = state.add_hero(Hero::new(0, "E", 1, 1, 0, 1, MapCoord::new(1, 0), 2));
        state.attack_hero(pid, eid).unwrap();
        assert!(state.score.total() > 0);
    }

    #[test]
    fn living_heroes_excludes_dead() {
        let map = meadow_map(5, 5);
        let mut state = make_state(map);
        state.add_hero(player(MapCoord::new(0, 0)));
        let mut e = enemy(MapCoord::new(1, 0));
        e.take_damage(30); // kill before adding
        state.add_hero(e);
        assert_eq!(state.living_heroes(false).len(), 0);
        assert_eq!(state.living_heroes(true).len(), 1);
    }

    #[test]
    fn on_turn_increments_active_team_counter() {
        let map = meadow_map(5, 5);
        let mut state = make_state(map);
        // All teams start at 0.
        for (_, team) in &state.teams {
            assert_eq!(team.get_turn(), 0);
        }
        // First team's turn begins.
        let event = state.on_turn().unwrap();
        let active_id = *state.get_active_team_id().unwrap();
        assert_eq!(state.get_active_team().unwrap().get_turn(), 1);
        assert!(matches!(
            event,
            TurnEvent::TeamTurnStarted { team_id, turn: 1 } if team_id == active_id
        ));
    }

    #[test]
    fn on_turn_each_team_has_own_counter() {
        let map = meadow_map(5, 5);
        let mut state = GameState::new(map, "test-session");
        state.add_team(Team::red());
        state.add_team(Team::blue());

        // Simulate: first team begins turn 1.
        state.on_turn().unwrap();
        assert_eq!(state.teams[&0].get_turn(), 1);
        assert_eq!(state.teams[&1].get_turn(), 0);

        // Switch to second team and begin its turn 1.
        let _ = state.get_next_active_team();
        state.on_turn().unwrap();
        assert_eq!(state.teams[&0].get_turn(), 1);
        assert_eq!(state.teams[&1].get_turn(), 1);

        // Cycle back to first team, begin its turn 2.
        let _ = state.get_next_active_team();
        state.on_turn().unwrap();
        assert_eq!(state.teams[&0].get_turn(), 2);
        assert_eq!(state.teams[&1].get_turn(), 1);
    }
}
