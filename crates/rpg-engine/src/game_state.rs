//! Game state and turn manager.
//!
//! [`GameState`] is the single source of truth for a running game session.
//! It owns the map, all heroes, the turn counter, and the score board.
//!
//! ## Turn loop
//! ```text
//! loop {
//!     // player/AI issues move_hero / attack_hero calls
//!     state.advance_turn();   // resets movement, awards survival points
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::combat::{self, CombatResult};
use crate::error::Error;
#[allow(unused_imports)] // Team is re-exported for tests via `use super::*`
use crate::hero::{Hero, Team};
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
    /// City ownership changed: `team_name` is empty when the city becomes neutral.
    CityOwnerChanged { coord: MapCoord, team_name: String },
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
    /// team name.  Absence from the map means the city is neutral.
    pub city_owners: HashMap<MapCoord, String>,
}

impl GameState {
    /// Creates a new game session at turn 1.
    pub fn new(map: GameMap, heroes: Vec<Hero>) -> Self {
        Self {
            map,
            heroes,
            turn: 1,
            score: ScoreBoard::new(),
            city_owners: HashMap::new(),
        }
    }

    pub fn get_turn(&self) -> u32 {
        self.turn
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

    /// Returns the team name that owns the city at `coord`, or `None` if neutral.
    pub fn city_owner(&self, coord: MapCoord) -> Option<&str> {
        self.city_owners.get(&coord).map(|s| s.as_str())
    }

    /// Sets the owning team for the city at `coord`.
    ///
    /// Pass an empty string (or use `city_owners.remove`) to make a city neutral.
    pub fn set_city_owner(&mut self, coord: MapCoord, team_name: String) {
        self.city_owners.insert(coord, team_name);
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

            // City ownership: any hero entering a City or CityEntrance claims it for
            // their team.  Emit CityOwnerChanged only when the owner actually changes.
            if matches!(tile.kind, Tiles::City | Tiles::CityEntrance) {
                let team_name = self.heroes[idx].team.name.clone();
                let already_owned = self
                    .city_owners
                    .get(&target)
                    .map(|o| o == &team_name)
                    .unwrap_or(false);
                if !already_owned {
                    self.city_owners.insert(target, team_name.clone());
                    events.push(TurnEvent::CityOwnerChanged {
                        coord: target,
                        team_name,
                    });
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

    /// Advances to the next turn: resets movement for all living heroes
    /// and awards one survival point.
    pub fn advance_turn(&mut self) -> Vec<TurnEvent> {
        for hero in self.heroes.iter_mut().filter(|h| h.is_alive()) {
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
    fn advance_turn_resets_movement_and_increments_turn() {
        let map = meadow_map(5, 5);
        let mut h = player(1, MapCoord::new(0, 0));
        h.mov_remaining = 0;
        let mut state = GameState::new(map, vec![h]);
        state.advance_turn();
        assert_eq!(state.turn, 2);
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
}
