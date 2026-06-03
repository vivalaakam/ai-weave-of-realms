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
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use tracing::{info, instrument};

use crate::combat::{self, CombatResult};
use crate::config::{GameConfig, HeroCatalog, TeamCatalog, TileConfig};
use crate::entrance_info::EntranceInfo;
use crate::error::EngineError;
use crate::hero::{Hero, HeroId, TeamId};
use crate::hero_candidate::HeroCandidate;
use crate::map::game_map::{Direction, GameMap, ResourceKind};
use crate::map::tile::Tiles;
use crate::map_coord::MapCoord;
use crate::rng::SeededRng;
use crate::score::{ScoreBoard, ScoreEvent};
use crate::state_flood::flood_city;
use crate::team::Team;

/// Describes how a game can be won.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WinCondition {
    /// Reach this score to trigger a win.
    ScoreThreshold(i32),
    /// Eliminate every enemy hero.
    DefeatAllEnemies,
}

/// Result of checking whether the game has ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameOutcome {
    /// A player team reached the win condition.
    Victory { team_id: TeamId },
    /// All heroes of every player team are dead.
    Defeat,
}

// ─── Economy ────────────────────────────────────────────────────────────────

/// Gold granted to a team at the start of each of its turns.
pub const TURN_GOLD_INCOME: u32 = 50;
/// Gold cost to place a resource-control rod.
pub const ROD_COST: u32 = 50;
/// Gold earned per turn from each owned gold mine.
pub const GOLD_MINE_INCOME: u32 = 25;
/// Resource units earned per turn from each owned resource mine.
pub const RESOURCE_MINE_INCOME: u32 = 10;

// ─── TurnEvent ────────────────────────────────────────────────────────────────

/// An event that occurred during the current turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnEvent {
    /// A hero moved to a new tile.
    HeroMoved { hero_id: HeroId, from: MapCoord, to: MapCoord },
    /// A hero visited a point of interest and triggered a score event.
    PoiVisited { hero_id: HeroId, coord: MapCoord },
    /// City ownership changed: `team_id` is None when the city becomes neutral.
    CityOwnerChanged { coord: MapCoord, team_id: Option<TeamId> },
    /// Resource ownership changed: `team_id` is None when the resource becomes neutral.
    ResourceOwnerChanged { coord: MapCoord, team_id: Option<TeamId> },
    /// Land ownership changed: `team_id` is None when the land becomes neutral.
    LandOwnerChanged { coord: MapCoord, team_id: Option<TeamId> },
    /// A hero placed a resource-control rod and moved away from its tile.
    ResourceRodPlaced { hero_id: HeroId, coord: MapCoord, team_id: TeamId },
    /// A hero engaged and resolved combat with an enemy.
    CombatResolved { attacker_id: HeroId, defender_id: HeroId, result: CombatResult },
    /// A hero was defeated and removed from the map.
    HeroDefeated { hero_id: HeroId },
    /// The turn counter advanced.
    TurnAdvanced { turn: u32 },
    /// A team's per-team turn counter advanced (emitted at the start of that team's turn).
    TeamTurnStarted { team_id: TeamId, turn: u32 },
    /// A team collected its start-of-turn income (base gold plus mine output).
    TeamIncomeCollected { team_id: TeamId, gold: u32 },
}

// ─── GameState ────────────────────────────────────────────────────────────────

/// Complete state of a running game session.
#[derive(Serialize, Deserialize)]
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
    /// Resource point ownership. Absence from the map means the resource is neutral.
    pub resource_owners: BTreeMap<MapCoord, TeamId>,
    /// Land ownership. Absence from the map means the land is neutral.
    pub land_owners: BTreeMap<MapCoord, TeamId>,
    /// Resource-control rods placed by teams.
    pub resource_rods: BTreeMap<MapCoord, TeamId>,
    /// All teams in the game (player-controlled and AI).
    pub(crate) teams: BTreeMap<TeamId, Team>,
    pub(crate) teams_order: VecDeque<TeamId>,
    /// Last active hero for each team. Used to restore selection when switching teams.
    pub(crate) active_hero: BTreeMap<TeamId, Option<HeroId>>,
    /// Session RNG, seeded at construction time.
    pub(crate) rng: SeededRng,
    pub(crate) hero_pointer: HeroId,

    #[serde(default)]
    config: GameConfig,
    hero_candidates: Vec<HeroCandidate>,
}

impl GameState {
    /// Creates a new empty game session with a map and a seed for the session RNG.
    ///
    /// Add teams via [`add_team`](Self::add_team) and heroes via
    /// [`add_hero`](Self::add_hero) before the first turn.
    pub fn new(map: GameMap, seed: impl AsRef<str>) -> Self {
        Self::new_with_config(map, seed, GameConfig::default())
    }

    /// Creates a new empty game session with static config loaded by the host.
    pub fn new_with_config(map: GameMap, seed: impl AsRef<str>, config: GameConfig) -> Self {
        let hero_candidates = config.heroes.heroes().to_vec();
        Self {
            map,
            heroes: BTreeMap::new(),
            score: ScoreBoard::new(),
            city_owners: BTreeMap::new(),
            resource_owners: BTreeMap::new(),
            land_owners: BTreeMap::new(),
            resource_rods: BTreeMap::new(),
            teams: BTreeMap::new(),
            active_hero: BTreeMap::new(),
            teams_order: VecDeque::new(),
            rng: SeededRng::new(seed.as_ref()),
            hero_pointer: 0,
            config,
            hero_candidates,
        }
    }

    /// Replaces static configuration after loading a save produced by an older
    /// schema or a host that stores config outside the save file.
    pub fn set_config(&mut self, config: GameConfig) {
        for hero in self.heroes.values_mut() {
            if hero.atlas_index == 0 {
                if let Some(candidate) = config
                    .heroes
                    .heroes()
                    .iter()
                    .find(|candidate| candidate.get_class_id() == hero.class_id)
                {
                    hero.atlas_index = candidate.get_atlas_index();
                }
            }
        }
        self.hero_candidates = config.heroes.heroes().to_vec();
        self.config = config;
    }

    pub fn config(&self) -> &GameConfig {
        &self.config
    }

    pub fn tile_config(&self) -> &TileConfig {
        &self.config.tiles
    }

    pub fn team_catalog(&self) -> &TeamCatalog {
        &self.config.teams
    }

    pub fn hero_catalog(&self) -> &HeroCatalog {
        &self.config.heroes
    }

    /// Adds a hero to the session, auto-assigning `id = heroes.len()`.
    ///
    /// Returns the assigned [`HeroId`].
    #[instrument(level = "info", skip(self))]
    pub fn add_hero(&mut self, team_id: TeamId, hero: &HeroCandidate, coord: &MapCoord) -> HeroId {
        let next_hero_id = self.hero_pointer;

        let hero = Hero::new(next_hero_id, hero, coord, team_id, &self.rng);

        self.heroes.insert(next_hero_id, hero);
        self.hero_pointer += 1;
        info!(hero_id = next_hero_id, "Added hero");
        next_hero_id
    }

    /// Hires a hero of `class` for `team_id` at `coord`, charging the class's
    /// [`hire_cost`](HeroClass::hire_cost) from the team's gold.
    ///
    /// The tile must be a city owned by the team and free of other heroes.
    ///
    /// # Errors
    /// - [`EngineError::InvalidTiles`] if the tile is occupied or not an owned city.
    /// - [`EngineError::InsufficientGold`] if the team cannot afford the hire.
    pub fn hire_hero(
        &mut self,
        hero: &HeroCandidate,
        coord: &MapCoord,
    ) -> Result<HeroId, EngineError> {
        let active_team_id = *self.get_active_team_id()?;

        if self.hero_at(coord).is_some() || self.city_owner(coord) != Some(active_team_id) {
            return Err(EngineError::InvalidTiles("cannot hire on this tile".into()));
        }

        let Some(team) = self.teams.get_mut(&active_team_id) else {
            return Err(EngineError::ActiveTeamNotFound(active_team_id));
        };

        if !team.spend_gold(hero.get_cost()) {
            return Err(EngineError::InsufficientGold {
                needed: hero.get_cost(),
                have: team.gold(),
            });
        }

        Ok(self.add_hero(active_team_id, hero, coord))
    }

    pub fn get_hero(&self, id: HeroId) -> Option<&Hero> {
        self.heroes.get(&id)
    }

    pub fn get_team_heroes(&self, team_id: TeamId) -> Vec<HeroId> {
        self.heroes.values().filter(|h| h.get_team_id() == team_id).map(|h| h.get_id()).collect()
    }

    pub fn get_total_heroes(&self) -> usize {
        self.heroes.len()
    }

    pub fn add_hero_candidates(&mut self, heroes: Vec<HeroCandidate>) {
        self.hero_candidates.extend(heroes);
    }

    pub fn get_hero_candidates(&self) -> &[HeroCandidate] {
        &self.hero_candidates
    }

    pub fn get_hero_candidate_at(&self, index: usize) -> Option<&HeroCandidate> {
        self.hero_candidates.get(index)
    }

    pub fn get_hero_candidate_count(&self) -> usize {
        self.hero_candidates.len()
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
    pub fn get_active_team(&self) -> Result<&Team, EngineError> {
        let active_team = self.get_active_team_id()?;
        self.teams.get(active_team).ok_or(EngineError::ActiveTeamNotFound(*active_team))
    }

    /// Returns the currently active team id.
    pub fn get_active_team_id(&self) -> Result<&TeamId, EngineError> {
        self.teams_order.front().ok_or(EngineError::NoActiveTeam)
    }

    /// Returns all player-controlled teams.
    pub fn player_teams(&self) -> impl Iterator<Item = &Team> {
        self.teams.iter().filter(|(_, t)| t.is_player_controlled()).map(|(_, t)| t)
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
        self.teams.values().find(|t| !t.is_player_controlled()).map(|t| t.get_id())
    }

    pub fn get_team_alive_heroes_ids(&self, team_id: TeamId) -> Vec<HeroId> {
        self.heroes
            .iter()
            .filter_map(
                |(&id, h)| {
                    if h.team_id == team_id && h.is_alive() {
                        Some(id)
                    } else {
                        None
                    }
                },
            )
            .collect::<Vec<HeroId>>()
    }

    /// Advances to the next player team.
    ///
    /// Returns `true` if all player teams have acted (full round completed).
    pub fn get_next_active_team(&mut self) -> Result<TeamId, EngineError> {
        let Some(current) = self.teams_order.pop_front() else {
            return Err(EngineError::NoActiveTeam);
        };
        self.teams_order.push_back(current);
        let Some(&next) = self.teams_order.front() else {
            return Err(EngineError::NextActiveTeam);
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
    pub fn on_turn(&mut self) -> Result<TurnEvent, EngineError> {
        let active_team_id = *self.get_active_team_id()?;
        let Some(team) = self.teams.get_mut(&active_team_id) else {
            return Err(EngineError::ActiveTeamNotFound(active_team_id));
        };

        team.increment_turn();
        let team_id = team.get_id();
        let turn = team.get_turn();

        for (_, hero) in
            self.heroes.iter_mut().filter(|(_, h)| h.is_alive() && h.team_id == team_id)
        {
            hero.reset_movement();
        }

        Ok(TurnEvent::TeamTurnStarted { team_id, turn })
    }

    /// Grants `team_id` its start-of-turn income: a flat gold stipend plus the
    /// output of every mine the team owns. Gold mines pay gold; resource mines
    /// add to the matching resource stockpile.
    ///
    /// Call this once when a team's turn begins (after [`on_turn`](Self::on_turn)).
    pub fn grant_turn_income(&mut self, team_id: TeamId) -> TurnEvent {
        // Tally mine output first to avoid borrowing the team while reading the map.
        let mut gold = TURN_GOLD_INCOME;
        let mut resources = [0u32; 4];
        for (&coord, &owner) in self.resource_owners.iter() {
            if owner != team_id {
                continue;
            }
            match self.resource_income_kind(coord) {
                Some(kind) if kind.is_gold() => gold += GOLD_MINE_INCOME,
                Some(kind) => {
                    if let Some(idx) = kind.resource_index() {
                        resources[idx] += RESOURCE_MINE_INCOME;
                    }
                }
                None => {}
            }
        }

        if let Some(team) = self.teams.get_mut(&team_id) {
            team.add_gold(gold);
            for (idx, amount) in resources.iter().enumerate() {
                if *amount > 0 {
                    team.add_resource(idx, *amount);
                }
            }
        }

        TurnEvent::TeamIncomeCollected { team_id, gold }
    }

    /// Returns the resource kind produced by the mine at `coord`, falling back
    /// to the tile kind for maps (e.g. Tiled imports) that carry no resource
    /// nodes. Returns `None` when the tile is not a mine at all.
    fn resource_income_kind(&self, coord: MapCoord) -> Option<ResourceKind> {
        if let Some(node) = self.map.resource_node_at(coord) {
            return Some(node.kind);
        }
        match self.map.get_tile(coord).map(|t| t.kind) {
            Ok(Tiles::Gold) => Some(ResourceKind::GoldMine),
            Ok(Tiles::Resource) => Some(ResourceKind::Resource1),
            _ => None,
        }
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
    pub fn hero_at(&self, pos: &MapCoord) -> Option<&Hero> {
        self.heroes.values().find(|h| h.is_alive() && h.position.eq(pos))
    }

    /// Returns the team id that owns the city at `coord`, or `None` if neutral.
    pub fn city_owner(&self, coord: &MapCoord) -> Option<TeamId> {
        self.city_owners.get(coord).copied()
    }

    /// Returns the team id that owns the resource at `coord`, or `None` if neutral.
    pub fn resource_owner(&self, coord: MapCoord) -> Option<TeamId> {
        self.resource_owners.get(&coord).copied()
    }

    /// Returns the team id that owns land at `coord`, or `None` if neutral.
    pub fn land_owner(&self, coord: MapCoord) -> Option<TeamId> {
        self.land_owners.get(&coord).copied()
    }

    /// Returns the team id that owns the resource-control rod at `coord`, if any.
    pub fn resource_rod_owner(&self, coord: MapCoord) -> Option<TeamId> {
        self.resource_rods.get(&coord).copied()
    }

    /// Returns the first city entrance tile owned by `team_id`.
    ///
    /// Useful for centering the camera when the team has no hero yet.
    pub fn city_entrance_for_team(&self, team_id: TeamId) -> Option<MapCoord> {
        self.city_owners.iter().filter(|(_, &owner)| owner == team_id).find_map(|(coord, _)| {
            let tile = self.map.get_tile(*coord).ok()?;
            if tile.kind == Tiles::CityEntrance {
                Some(*coord)
            } else {
                None
            }
        })
    }

    /// Returns any city tile owned by the given team.
    pub fn city_owner_for_team(&self, team_id: TeamId) -> Option<MapCoord> {
        self.city_owners.iter().find(|(_, &owner)| owner == team_id).map(|(coord, _)| *coord)
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

        let next_idx = current_idx.map(|idx| (idx + 1) % team_heroes.len()).unwrap_or(0);

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

    /// Sets the owning team for a resource point.
    ///
    /// # Returns
    /// `true` if the coordinate is a known resource point and ownership was updated.
    pub fn set_resource_owner(&mut self, coord: MapCoord, team_id: Option<TeamId>) -> bool {
        if !self.is_resource_point(coord) {
            return false;
        }
        match team_id {
            Some(id) => self.resource_owners.insert(coord, id),
            None => self.resource_owners.remove(&coord),
        };
        true
    }

    /// Sets the owning team for one land tile.
    ///
    /// # Returns
    /// `true` if the coordinate is inside the map and ownership was updated.
    pub fn set_land_owner(&mut self, coord: MapCoord, team_id: Option<TeamId>) -> bool {
        if self.map.get_tile(coord).is_err() {
            return false;
        }
        match team_id {
            Some(id) => self.land_owners.insert(coord, id),
            None => self.land_owners.remove(&coord),
        };
        true
    }

    /// Finds TeamId by team name (case-insensitive). Returns None if not found.
    pub fn team_id_by_name(&self, name: &str) -> Option<TeamId> {
        let name_lower = name.to_lowercase();
        self.teams.values().find(|t| t.name.to_lowercase() == name_lower).map(|t| t.get_id())
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
    /// - [`EngineError::OutOfBounds`]      — hero id not found or step leaves the map
    /// - [`EngineError::NoMovementPoints`] — hero has no movement points remaining
    /// - [`EngineError::ImpassableTile`]   — target tile is not passable terrain
    /// - [`EngineError::OccupiedTile`]     — target tile is occupied by another hero
    pub fn move_hero(
        &mut self,
        hero_id: HeroId,
        direction: Direction,
    ) -> Result<Vec<TurnEvent>, EngineError> {
        self.hero_index(hero_id)?;
        let start = self.heroes[&hero_id].position;

        // Ensure the hero has at least one movement point before computing cost.
        if self.heroes[&hero_id].mov_remaining == 0 {
            return Err(EngineError::NoMovementPoints { hero_id });
        }

        // Compute the target coordinate (bounds-checked).
        let w = self.map.tile_width();
        let h = self.map.tile_height();
        let target = direction.apply(start, w, h).ok_or_else(|| {
            EngineError::OutOfBounds(format!(
                "direction {direction:?} from ({}, {}) leaves the map",
                start.x, start.y
            ))
        })?;

        // Check passability.
        let tile = self.map.get_tile(target)?;
        if !tile.kind.is_passable_with_config(self.tile_config()) {
            return Err(EngineError::ImpassableTile { x: target.x, y: target.y });
        }

        // Check occupancy.
        if let Some(other) = self.hero_at(&target) {
            if other.get_id() != hero_id {
                return Err(EngineError::OccupiedTile { x: target.x, y: target.y });
            }
        }

        // Deduct the movement cost for entering the target tile.
        let cost =
            (1i32 + tile.kind.movement_cost_modifier_with_config(self.tile_config())).max(1) as u32;
        if self.heroes[&hero_id].mov_remaining < cost {
            return Err(EngineError::NoMovementPoints { hero_id });
        }

        self.heroes.get_mut(&hero_id).unwrap().mov_remaining -= cost;
        self.heroes.get_mut(&hero_id).unwrap().position = target;

        let mut events = vec![TurnEvent::HeroMoved { hero_id, from: start, to: target }];

        // Trigger POI score events.
        if let Ok(tile) = self.map.get_tile(target) {
            if tile.kind.is_point_of_interest_with_config(self.tile_config()) {
                events.push(TurnEvent::PoiVisited { hero_id, coord: target });
                match tile.kind {
                    Tiles::City | Tiles::CityEntrance => {
                        self.score.record(ScoreEvent::CityCapture { city: target });
                    }
                    Tiles::Gold => {
                        self.score.record(ScoreEvent::GoldCollected { coord: target });
                    }
                    Tiles::Resource => {
                        self.score.record(ScoreEvent::ResourceCollected { coord: target });
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
                    let already_owned =
                        self.city_owners.get(&coord).map(|&o| o == tid).unwrap_or(false);
                    if !already_owned {
                        self.city_owners.insert(coord, tid);
                        events.push(TurnEvent::CityOwnerChanged { coord, team_id: Some(tid) });
                    }
                }
            }

            if self.map.resource_node_at(target).is_some() {
                let tid = self.heroes[&hero_id].team_id;
                self.resource_owners.insert(target, tid);
            }
        }

        Ok(events)
    }

    /// Places a resource-control rod on the selected hero's tile.
    ///
    /// The rod claims land in radius 1. Any resource or gold mine inside that
    /// claimed area becomes owned by the hero's team and expands ownership to
    /// radius 1 around the resource. The hero is moved to the first adjacent
    /// passable, unoccupied tile in North/East/South/West order.
    ///
    /// # Errors
    /// Returns [`EngineError::OutOfBounds`] if the hero id is unknown.
    /// Returns [`EngineError::InvalidTiles`] if the rod cannot be placed.
    pub fn place_resource_rod(&mut self, hero_id: HeroId) -> Result<Vec<TurnEvent>, EngineError> {
        self.hero_index(hero_id)?;
        let rod_coord = self.heroes[&hero_id].position;
        if self.resource_rods.contains_key(&rod_coord) {
            return Err(EngineError::InvalidTiles("resource rod already exists here".into()));
        }

        let team_id = self.heroes[&hero_id].team_id;
        let Some(new_position) = self.first_available_adjacent_tile(rod_coord, hero_id) else {
            return Err(EngineError::InvalidTiles("no adjacent passable tile for hero".into()));
        };

        let team = self.teams.get_mut(&team_id).ok_or(EngineError::ActiveTeamNotFound(team_id))?;
        if !team.spend_gold(ROD_COST) {
            return Err(EngineError::InsufficientGold { needed: ROD_COST, have: team.gold() });
        }

        self.resource_rods.insert(rod_coord, team_id);
        self.heroes.get_mut(&hero_id).unwrap().position = new_position;

        let mut events = vec![
            TurnEvent::ResourceRodPlaced { hero_id, coord: rod_coord, team_id },
            TurnEvent::HeroMoved { hero_id, from: rod_coord, to: new_position },
        ];
        self.claim_land_radius(rod_coord, team_id, &mut events);

        let resource_coords: Vec<MapCoord> = self
            .owned_radius(rod_coord)
            .into_iter()
            .filter(|coord| self.is_resource_point(*coord))
            .collect();
        for resource_coord in resource_coords {
            let already_owned =
                self.resource_owners.get(&resource_coord).map(|&o| o == team_id).unwrap_or(false);
            if !already_owned {
                self.resource_owners.insert(resource_coord, team_id);
                events.push(TurnEvent::ResourceOwnerChanged {
                    coord: resource_coord,
                    team_id: Some(team_id),
                });
            }
            self.claim_land_radius(resource_coord, team_id, &mut events);
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
    /// Returns [`EngineError::OutOfBounds`] if either hero id is not found.
    pub fn attack_hero(
        &mut self,
        attacker_id: HeroId,
        defender_id: HeroId,
    ) -> Result<Vec<TurnEvent>, EngineError> {
        self.hero_index(attacker_id)?;
        self.hero_index(defender_id)?;

        let mut attacker = self.heroes.remove(&attacker_id).unwrap();
        let mut defender = self.heroes.remove(&defender_id).unwrap();
        let result = combat::resolve_combat(&mut attacker, &mut defender);
        attacker.take_damage(result.attacker_damage);
        defender.take_damage(result.defender_damage);
        self.heroes.insert(attacker_id, attacker);
        self.heroes.insert(defender_id, defender);

        let mut events =
            vec![TurnEvent::CombatResolved { attacker_id, defender_id, result: result.clone() }];

        if !result.defender_survived {
            self.score.record(ScoreEvent::EnemyDefeated { enemy_id: defender_id });
            events.push(TurnEvent::HeroDefeated { hero_id: defender_id });
        }
        if !result.attacker_survived {
            events.push(TurnEvent::HeroDefeated { hero_id: attacker_id });
        }

        Ok(events)
    }

    /// Advances the global turn: resets movement for non-player-controlled (AI) heroes
    /// and awards one survival point.
    ///
    /// Movement for player-controlled heroes is reset per-team in [`GameState::on_turn`].
    pub fn advance_turn(&mut self) -> Vec<TurnEvent> {
        let player_teams: BTreeSet<TeamId> =
            self.teams.values().filter(|t| t.is_player_controlled()).map(|t| t.get_id()).collect();

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

    fn hero_index(&self, id: HeroId) -> Result<HeroId, EngineError> {
        if self.heroes.contains_key(&id) {
            Ok(id)
        } else {
            Err(EngineError::OutOfBounds(format!("hero {id} not found")))
        }
    }

    fn first_available_adjacent_tile(&self, coord: MapCoord, hero_id: HeroId) -> Option<MapCoord> {
        let w = self.map.tile_width();
        let h = self.map.tile_height();
        [Direction::North, Direction::East, Direction::South, Direction::West]
            .into_iter()
            .filter_map(|direction| direction.apply(coord, w, h))
            .find(|candidate| {
                self.map
                    .get_tile(*candidate)
                    .map(|tile| tile.kind.is_passable_with_config(self.tile_config()))
                    .unwrap_or(false)
                    && self
                        .hero_at(candidate)
                        .map(|other| other.get_id() == hero_id)
                        .unwrap_or(true)
                    && !self.resource_rods.contains_key(candidate)
            })
    }

    fn owned_radius(&self, center: MapCoord) -> Vec<MapCoord> {
        let mut coords = Vec::new();
        let min_x = center.x.saturating_sub(1);
        let min_y = center.y.saturating_sub(1);
        let max_x = center.x.saturating_add(1).min(self.map.tile_width().saturating_sub(1));
        let max_y = center.y.saturating_add(1).min(self.map.tile_height().saturating_sub(1));
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                coords.push(MapCoord::new(x, y));
            }
        }
        coords
    }

    fn claim_land_radius(
        &mut self,
        center: MapCoord,
        team_id: TeamId,
        events: &mut Vec<TurnEvent>,
    ) {
        for coord in self.owned_radius(center) {
            let already_owned =
                self.land_owners.get(&coord).map(|&o| o == team_id).unwrap_or(false);
            if !already_owned {
                self.land_owners.insert(coord, team_id);
                events.push(TurnEvent::LandOwnerChanged { coord, team_id: Some(team_id) });
            }
        }
    }

    fn is_resource_point(&self, coord: MapCoord) -> bool {
        self.map.resource_node_at(coord).is_some()
            || self
                .map
                .get_tile(coord)
                .map(|tile| matches!(tile.kind, Tiles::Gold | Tiles::Resource))
                .unwrap_or(false)
    }

    /// Checks whether the game has reached a win or loss condition.
    ///
    /// `conditions` is a map from team id to that team's win condition.
    /// A player team must satisfy ANY of the assigned conditions; a loss
    /// triggers when every player team has no living heroes remaining.
    ///
    /// # Returns
    /// - `Some(GameOutcome::Victory { team_id })` if `team_id` has met a win condition.
    /// - `Some(GameOutcome::Defeat)` if all player-team heroes are dead.
    /// - `None` while the game is still in progress.
    pub fn check_outcome(
        &self,
        conditions: &BTreeMap<TeamId, WinCondition>,
    ) -> Option<GameOutcome> {
        // Check victory conditions first.
        for (team_id, condition) in conditions {
            let Some(team) = self.teams.get(team_id) else {
                continue;
            };
            if !team.is_player_controlled() {
                continue;
            }
            match condition {
                WinCondition::ScoreThreshold(threshold) => {
                    if self.score.total() >= *threshold {
                        return Some(GameOutcome::Victory { team_id: *team_id });
                    }
                }
                WinCondition::DefeatAllEnemies => {
                    let enemies_alive = self
                        .heroes
                        .values()
                        .any(|h| h.is_alive() && !self.is_player_controlled(h.team_id));
                    if !enemies_alive {
                        return Some(GameOutcome::Victory { team_id: *team_id });
                    }
                }
            }
        }

        // Check defeat: all heroes in every player team are dead.
        let player_teams: Vec<TeamId> =
            self.teams.values().filter(|t| t.is_player_controlled()).map(|t| t.get_id()).collect();
        if !player_teams.is_empty()
            && player_teams.iter().all(|tid| {
                self.heroes.values().filter(|h| h.team_id == *tid).all(|h| !h.is_alive())
            })
        {
            return Some(GameOutcome::Defeat);
        }

        None
    }

    pub fn get_entrance_info_at_coord(&self, coord: &MapCoord) -> EntranceInfo {
        let Ok(active_team_id) = self.get_active_team_id() else {
            return EntranceInfo::NoOwnership;
        };

        let Some(owner) = self.city_owner(coord) else {
            return EntranceInfo::NoOwnership;
        };

        if owner.ne(active_team_id) {
            return EntranceInfo::NoOwnership;
        }

        if let Some(hero) = self.hero_at(coord) {
            return EntranceInfo::Occupied { name: hero.get_team_id() };
        };

        EntranceInfo::CanHire
    }

    /// Serializes the entire game state into a compact binary save format.
    pub fn to_save_bytes(&self) -> Result<Vec<u8>, EngineError> {
        minicbor_serde::to_vec(&self).map_err(EngineError::from)
    }

    pub fn from_save_bytes(bytes: &[u8]) -> Result<GameState, EngineError> {
        minicbor_serde::from_slice(bytes).map_err(EngineError::from)
    }
}

// ═══ Tests ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::game_map::{ResourceKind, ResourceNode};
    use crate::map::tile::Tile;
    use crate::team::STARTING_GOLD;

    fn meadow_map(w: u32, h: u32) -> GameMap {
        let tiles = vec![Tile { kind: Tiles::Meadow }; (w * h) as usize];
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

    fn candidate(name: &str, hp: u32, atk: u32, def: u32, spd: u32) -> HeroCandidate {
        HeroCandidate {
            class_id: 0,
            name: name.to_owned(),
            description: "test hero".to_owned(),
            atlas_index: 0,
            cost: 50,
            hp,
            atk,
            def,
            spd,
        }
    }

    // team_id=0 = Team::red() at index 0 (default active team); spd=10 → mov=30
    fn player_candidate() -> HeroCandidate {
        candidate("Player", 100, 20, 10, 10)
    }

    // team_id=2 = Team::enemy() (non-player-controlled); spd=5 → mov=25
    fn enemy_candidate() -> HeroCandidate {
        candidate("Enemy", 30, 10, 5, 5)
    }

    fn add_player(state: &mut GameState, pos: MapCoord) -> HeroId {
        state.add_hero(0, &player_candidate(), &pos)
    }

    fn add_enemy(state: &mut GameState, pos: MapCoord) -> HeroId {
        state.add_hero(2, &enemy_candidate(), &pos)
    }

    #[test]
    fn move_hero_updates_position_and_spends_movement() {
        let map = meadow_map(10, 10);
        let mut state = make_state(map);
        add_player(&mut state, MapCoord::new(0, 0));

        // Move East three times — each step costs 1 movement point on Meadow.
        state.move_hero(0, Direction::East).unwrap();
        state.move_hero(0, Direction::East).unwrap();
        let events = state.move_hero(0, Direction::East).unwrap();
        assert_eq!(state.hero(0).unwrap().position, MapCoord::new(3, 0));
        assert_eq!(state.hero(0).unwrap().mov_remaining, 27); // 30 - 3 = 27
        assert!(events.iter().any(|e| matches!(e, TurnEvent::HeroMoved { .. })));
    }

    #[test]
    fn move_hero_claims_resource_node() {
        let mut map = meadow_map(3, 1);
        map.set_resource_nodes(vec![ResourceNode {
            coord: MapCoord::new(1, 0),
            kind: ResourceKind::Resource1,
        }])
        .unwrap();
        let mut state = make_state(map);
        add_player(&mut state, MapCoord::new(0, 0));

        state.move_hero(0, Direction::East).unwrap();

        assert_eq!(state.resource_owner(MapCoord::new(1, 0)), Some(0));
    }

    #[test]
    fn save_round_trip_keeps_resource_nodes_and_owners() {
        let mut map = meadow_map(3, 1);
        let coord = MapCoord::new(1, 0);
        map.set_resource_nodes(vec![ResourceNode { coord, kind: ResourceKind::GoldMine }]).unwrap();
        let mut state = make_state(map);
        state.set_resource_owner(coord, Some(1));

        let bytes = state.to_save_bytes().unwrap();
        let loaded = GameState::from_save_bytes(&bytes).unwrap();

        assert_eq!(loaded.map.resource_node_at(coord).unwrap().kind, ResourceKind::GoldMine);
        assert_eq!(loaded.resource_owner(coord), Some(1));
    }

    #[test]
    fn place_resource_rod_claims_land_resource_and_moves_hero() {
        let mut map = meadow_map(5, 5);
        let resource = MapCoord::new(3, 2);
        map.set_resource_nodes(vec![ResourceNode {
            coord: resource,
            kind: ResourceKind::GoldMine,
        }])
        .unwrap();
        let mut state = make_state(map);
        let hero_id = add_player(&mut state, MapCoord::new(2, 2));

        let events = state.place_resource_rod(hero_id).unwrap();

        assert_eq!(state.resource_rod_owner(MapCoord::new(2, 2)), Some(0));
        assert_eq!(state.hero(hero_id).unwrap().position, MapCoord::new(2, 1));
        assert_eq!(state.resource_owner(resource), Some(0));
        assert_eq!(state.land_owner(MapCoord::new(1, 1)), Some(0));
        assert_eq!(state.land_owner(MapCoord::new(4, 3)), Some(0));
        assert!(events.iter().any(|event| matches!(event, TurnEvent::ResourceRodPlaced { .. })));
        assert!(events.iter().any(|event| matches!(event, TurnEvent::ResourceOwnerChanged { .. })));
    }

    #[test]
    fn place_resource_rod_claims_gold_tile_without_resource_node() {
        let mut map = meadow_map(5, 5);
        let gold = MapCoord::new(3, 2);
        map.get_tile_mut(gold).unwrap().kind = Tiles::Gold;
        let mut state = make_state(map);
        let hero_id = add_player(&mut state, MapCoord::new(2, 2));

        state.place_resource_rod(hero_id).unwrap();

        assert_eq!(state.resource_owner(gold), Some(0));
        assert_eq!(state.land_owner(gold), Some(0));
        assert_eq!(state.land_owner(MapCoord::new(4, 3)), Some(0));
    }

    #[test]
    fn place_resource_rod_charges_gold() {
        let map = meadow_map(5, 5);
        let mut state = make_state(map);
        let hero_id = add_player(&mut state, MapCoord::new(2, 2));
        let before = state.get_team(0).unwrap().gold();

        state.place_resource_rod(hero_id).unwrap();

        assert_eq!(state.get_team(0).unwrap().gold(), before - ROD_COST);
    }

    #[test]
    fn place_resource_rod_fails_without_gold() {
        let map = meadow_map(5, 5);
        let mut state = make_state(map);
        let hero_id = add_player(&mut state, MapCoord::new(2, 2));
        state.teams.get_mut(&0).unwrap().spend_gold(STARTING_GOLD);

        let err = state.place_resource_rod(hero_id).unwrap_err();

        assert!(matches!(err, EngineError::InsufficientGold { needed: ROD_COST, .. }));
        // No rod placed, no gold spent below zero.
        assert_eq!(state.resource_rod_owner(MapCoord::new(2, 2)), None);
        assert_eq!(state.get_team(0).unwrap().gold(), 0);
    }

    #[test]
    fn grant_turn_income_pays_base_plus_mine_output() {
        let mut map = meadow_map(5, 5);
        let gold_mine = MapCoord::new(1, 1);
        let resource_mine = MapCoord::new(3, 3);
        map.set_resource_nodes(vec![
            ResourceNode { coord: gold_mine, kind: ResourceKind::GoldMine },
            ResourceNode { coord: resource_mine, kind: ResourceKind::Resource2 },
        ])
        .unwrap();
        let mut state = make_state(map);
        state.set_resource_owner(gold_mine, Some(0));
        state.set_resource_owner(resource_mine, Some(0));
        let before = state.get_team(0).unwrap().gold();

        let event = state.grant_turn_income(0);

        let team = state.get_team(0).unwrap();
        assert_eq!(team.gold(), before + TURN_GOLD_INCOME + GOLD_MINE_INCOME);
        // Resource2 maps to treasury slot index 1.
        assert_eq!(team.resource(1), RESOURCE_MINE_INCOME);
        assert!(matches!(event, TurnEvent::TeamIncomeCollected { team_id: 0, .. }));
    }

    #[test]
    fn hire_hero_charges_cost_and_blocks_when_poor() {
        let map = meadow_map(5, 5);
        let coord = MapCoord::new(2, 2);
        let second = MapCoord::new(2, 3);
        let mut state = make_state(map);
        state.set_city_owner(coord, Some(0));
        state.set_city_owner(second, Some(0));

        let mage_candidate = candidate("Mage", 100, 10, 5, 5);

        let before = state.get_team(0).unwrap().gold();
        let hero_id = state.hire_hero(&mage_candidate, &coord).unwrap();
        assert_eq!(state.get_hero(hero_id).unwrap().team_id, 0);
        assert_eq!(state.get_team(0).unwrap().gold(), before - 50);

        // Drain the treasury and confirm a second hire is rejected.
        let remaining = state.get_team(0).unwrap().gold();
        state.teams.get_mut(&0).unwrap().spend_gold(remaining);

        let mage_2_candidate = candidate("Mage 2", 100, 10, 5, 5);

        let err = state.hire_hero(&mage_2_candidate, &second).unwrap_err();
        assert!(matches!(err, EngineError::InsufficientGold { .. }));
    }

    #[test]
    fn save_round_trip_keeps_land_and_resource_rods() {
        let map = meadow_map(3, 3);
        let mut state = make_state(map);
        state.set_land_owner(MapCoord::new(1, 1), Some(1));
        state.resource_rods.insert(MapCoord::new(2, 2), 1);

        let bytes = state.to_save_bytes().unwrap();
        let loaded = GameState::from_save_bytes(&bytes).unwrap();

        assert_eq!(loaded.land_owner(MapCoord::new(1, 1)), Some(1));
        assert_eq!(loaded.resource_rod_owner(MapCoord::new(2, 2)), Some(1));
    }

    #[test]
    fn move_hero_with_zero_budget_returns_error() {
        let map = meadow_map(10, 10);
        let mut state = make_state(map);
        let hid = add_player(&mut state, MapCoord::new(0, 0));
        state.heroes.get_mut(&hid).unwrap().mov_remaining = 0;
        let result = state.move_hero(0, Direction::East);
        assert!(matches!(result, Err(EngineError::NoMovementPoints { .. })));
    }

    #[test]
    fn move_hero_into_impassable_returns_error() {
        use crate::map::tile::Tile;
        let mut tiles = vec![Tile { kind: Tiles::Meadow }; 9];
        tiles[1] = Tile { kind: Tiles::Mountain };
        let map = GameMap::new(3, 3, tiles, [0u8; 32]).unwrap();
        let mut state = make_state(map);
        add_player(&mut state, MapCoord::new(0, 0));
        let result = state.move_hero(0, Direction::East);
        assert!(matches!(result, Err(EngineError::ImpassableTile { .. })));
    }

    #[test]
    fn move_hero_out_of_bounds_returns_error() {
        let map = meadow_map(5, 5);
        let mut state = make_state(map);
        add_player(&mut state, MapCoord::new(0, 0));
        let result = state.move_hero(0, Direction::North);
        assert!(matches!(result, Err(EngineError::OutOfBounds(_))));
    }

    #[test]
    fn advance_turn_increments_global_turn_and_resets_ai_movement() {
        let map = meadow_map(5, 5);
        let mut state = make_state(map);
        let pid = add_player(&mut state, MapCoord::new(0, 0)); // id=0
        state.heroes.get_mut(&pid).unwrap().mov_remaining = 0;
        let eid = add_enemy(&mut state, MapCoord::new(1, 0)); // id=1
        state.heroes.get_mut(&eid).unwrap().mov_remaining = 0;
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
        let hid = add_player(&mut state, MapCoord::new(0, 0));
        state.heroes.get_mut(&hid).unwrap().mov_remaining = 0;
        state.on_turn().unwrap();
        assert_eq!(state.hero(hid).unwrap().mov_remaining, 30); // spd=10 → mov=30
    }

    #[test]
    fn attack_hero_applies_damage() {
        let map = meadow_map(5, 5);
        let mut state = make_state(map);
        let pid = add_player(&mut state, MapCoord::new(0, 0));
        let eid = add_enemy(&mut state, MapCoord::new(1, 0));
        state.attack_hero(pid, eid).unwrap();
        assert!(state.hero(eid).unwrap().hp < 30);
    }

    #[test]
    fn defeated_enemy_awards_score() {
        let map = meadow_map(5, 5);
        let mut state = make_state(map);
        let pid = state.add_hero(0, &candidate("P", 100, 200, 0, 10), &MapCoord::new(0, 0));
        let eid = state.add_hero(2, &candidate("E", 1, 1, 0, 1), &MapCoord::new(1, 0));
        state.attack_hero(pid, eid).unwrap();
        assert!(state.score.total() > 0);
    }

    #[test]
    fn living_heroes_excludes_dead() {
        let map = meadow_map(5, 5);
        let mut state = make_state(map);
        add_player(&mut state, MapCoord::new(0, 0));
        let eid = add_enemy(&mut state, MapCoord::new(1, 0));
        state.heroes.get_mut(&eid).unwrap().take_damage(30);
        assert_eq!(state.living_heroes(false).len(), 0);
        assert_eq!(state.living_heroes(true).len(), 1);
    }

    #[test]
    fn on_turn_increments_active_team_counter() {
        let map = meadow_map(5, 5);
        let mut state = make_state(map);
        // All teams start at 0.
        for team in state.teams.values() {
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
        state.add_team(Team::new(1, "Blue", (50, 50, 220), true));

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

    fn make_state_with_heroes(map: GameMap) -> (GameState, HeroId, HeroId, HeroId) {
        let mut state = make_state(map);
        let pid = state.add_hero(0, &candidate("P", 100, 200, 0, 10), &MapCoord::new(0, 0));
        let bid = state.add_hero(1, &candidate("P2", 100, 200, 0, 10), &MapCoord::new(2, 0));
        let eid = state.add_hero(2, &candidate("E", 1, 1, 0, 1), &MapCoord::new(1, 0));
        (state, pid, bid, eid)
    }

    #[test]
    fn check_outcome_score_threshold_victory() {
        let map = meadow_map(5, 5);
        let (mut state, _pid, _bid, eid) = make_state_with_heroes(map);
        let conditions = {
            let mut c = BTreeMap::<u8, WinCondition>::new();
            c.insert(0, WinCondition::ScoreThreshold(10));
            c
        };

        // No events yet → score is 0.
        assert_eq!(state.check_outcome(&conditions), None);

        // Trigger a score event worth 25 points.
        state.attack_hero(0, eid).unwrap();
        assert_eq!(state.check_outcome(&conditions), Some(GameOutcome::Victory { team_id: 0 }));
    }

    #[test]
    fn check_outcome_defeat_all_enemies_victory() {
        let map = meadow_map(5, 5);
        let (mut state, _pid, bid, eid) = make_state_with_heroes(map);
        let conditions = {
            let mut c = BTreeMap::new();
            c.insert(0, WinCondition::DefeatAllEnemies);
            c
        };

        assert_eq!(state.check_outcome(&conditions), None);

        // Kill every non-player hero.
        state.attack_hero(0, bid).unwrap();
        state.attack_hero(0, eid).unwrap();
        assert_eq!(state.check_outcome(&conditions), Some(GameOutcome::Victory { team_id: 0 }));
    }

    #[test]
    fn check_outcome_team_loss_when_all_dead() {
        let map = meadow_map(5, 5);
        let (mut state, pid, bid, _eid) = make_state_with_heroes(map);
        let conditions = {
            let mut c = BTreeMap::new();
            c.insert(0, WinCondition::DefeatAllEnemies);
            c.insert(1, WinCondition::DefeatAllEnemies);
            c
        };

        // Kill both player heroes deterministically.
        state.heroes.get_mut(&pid).unwrap().take_damage(100);
        state.heroes.get_mut(&bid).unwrap().take_damage(100);

        assert_eq!(state.check_outcome(&conditions), Some(GameOutcome::Defeat));
    }

    #[test]
    fn check_outcome_ignores_ai_teams() {
        let map = meadow_map(5, 5);
        let (mut state, pid, _bid, eid) = make_state_with_heroes(map);
        // Assign win condition to enemy team (non-player).
        let conditions = {
            let mut c = BTreeMap::new();
            c.insert(2, WinCondition::ScoreThreshold(1));
            c
        };

        // Even after scoring, enemy team should not trigger victory.
        state.attack_hero(pid, eid).unwrap();
        assert_eq!(state.check_outcome(&conditions), None);
    }
}
