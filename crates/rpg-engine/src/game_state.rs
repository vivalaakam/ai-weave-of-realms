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

use crate::combat::{self, CombatResult};
use crate::error::Error;
use crate::hero::{Hero, Team};
use crate::map::game_map::{GameMap, MapCoord};
use crate::map::tile::Tiles;
use crate::movement;
use crate::rng::SeededRng;
use crate::score::{ScoreBoard, ScoreEvent};

// ─── TurnEvent ────────────────────────────────────────────────────────────────

/// An event that occurred during the current turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnEvent {
    /// A hero moved to a new tile.
    HeroMoved { hero_id: u32, from: MapCoord, to: MapCoord },
    /// A hero visited a point of interest and triggered a score event.
    PoiVisited { hero_id: u32, coord: MapCoord },
    /// A hero engaged and resolved combat with an enemy.
    CombatResolved { attacker_id: u32, defender_id: u32, result: CombatResult },
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
    pub turn: u32,
    /// Accumulated score.
    pub score: ScoreBoard,
}

impl GameState {
    /// Creates a new game session at turn 1.
    pub fn new(map: GameMap, heroes: Vec<Hero>) -> Self {
        Self { map, heroes, turn: 1, score: ScoreBoard::new() }
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

    // ── Actions ───────────────────────────────────────────────────────────────

    /// Moves hero `hero_id` to `target`, spending movement points.
    ///
    /// After moving, checks whether the destination is a point of interest
    /// and records score events accordingly.
    ///
    /// # Errors
    /// - [`Error::OutOfBounds`] — hero id not found
    /// - [`Error::UnreachableTile`] — `target` unreachable within movement budget
    pub fn move_hero(
        &mut self,
        hero_id: u32,
        target: MapCoord,
    ) -> Result<Vec<TurnEvent>, Error> {
        let idx = self.hero_index(hero_id)?;
        
        // Check if target is occupied by another living hero
        if let Some(other) = self.hero_at(target) {
            if other.id != hero_id {
                return Err(Error::OccupiedTile { x: target.x, y: target.y });
            }
        }
        
        let start  = self.heroes[idx].position;
        let budget = self.heroes[idx].mov_remaining;

        let path = movement::find_path(&self.map, start, target, budget)
            .ok_or(Error::UnreachableTile { x: target.x, y: target.y })?;

        // Deduct the actual cost along the chosen path
        let cost: u32 = path.windows(2)
            .map(|w| {
                let tile = self.map.get_tile(w[1]).unwrap();
                (1i32 + tile.kind.movement_cost_modifier()).max(0) as u32
            })
            .sum();

        self.heroes[idx].mov_remaining = self.heroes[idx].mov_remaining.saturating_sub(cost);
        self.heroes[idx].position = target;

        let mut events = vec![TurnEvent::HeroMoved { hero_id, from: start, to: target }];

        // Trigger POI score events
        if let Ok(tile) = self.map.get_tile(target) {
            if tile.kind.is_point_of_interest() {
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
        }

        Ok(events)
    }

    /// Initiates combat between hero `attacker_id` and hero `defender_id`.
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
        rng: &mut SeededRng,
    ) -> Result<Vec<TurnEvent>, Error> {
        let att_idx = self.hero_index(attacker_id)?;
        let def_idx = self.hero_index(defender_id)?;

        let result = combat::resolve_combat(&self.heroes[att_idx], &self.heroes[def_idx], rng);

        self.heroes[att_idx].take_damage(result.attacker_damage);
        self.heroes[def_idx].take_damage(result.defender_damage);

        let mut events = vec![TurnEvent::CombatResolved {
            attacker_id,
            defender_id,
            result: result.clone(),
        }];

        if !result.defender_survived {
            self.score.record(ScoreEvent::EnemyDefeated { enemy_id: defender_id });
            events.push(TurnEvent::HeroDefeated { hero_id: defender_id });
        }
        if !result.attacker_survived {
            events.push(TurnEvent::HeroDefeated { hero_id: attacker_id });
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

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::tile::Tile;

    fn meadow_map(w: u32, h: u32) -> GameMap {
        let tiles = vec![Tile { kind: Tiles::Meadow }; (w * h) as usize];
        GameMap::new(w, h, tiles, [0u8; 32]).unwrap()
    }

    fn player(id: u32, pos: MapCoord) -> Hero {
        Hero::new(id, "Player", 100, 20, 10, 10, 4, pos, Team::player())
    }

    fn enemy(id: u32, pos: MapCoord) -> Hero {
        Hero::new(id, "Enemy", 30, 10, 5, 5, 3, pos, Team::enemy())
    }

    #[test]
    fn move_hero_updates_position_and_spends_movement() {
        let map = meadow_map(10, 10);
        let h = player(1, MapCoord::new(0, 0));
        let mut state = GameState::new(map, vec![h]);

        let events = state.move_hero(1, MapCoord::new(3, 0)).unwrap();
        assert_eq!(state.hero(1).unwrap().position, MapCoord::new(3, 0));
        assert_eq!(state.hero(1).unwrap().mov_remaining, 1); // 4 - 3 = 1
        assert!(events.iter().any(|e| matches!(e, TurnEvent::HeroMoved { .. })));
    }

    #[test]
    fn move_hero_beyond_budget_returns_error() {
        let map = meadow_map(10, 10);
        let h = player(1, MapCoord::new(0, 0));
        let mut state = GameState::new(map, vec![h]);
        let result = state.move_hero(1, MapCoord::new(9, 0));
        assert!(matches!(result, Err(Error::UnreachableTile { .. })));
    }

    #[test]
    fn advance_turn_resets_movement_and_increments_turn() {
        let map = meadow_map(5, 5);
        let mut h = player(1, MapCoord::new(0, 0));
        h.mov_remaining = 0;
        let mut state = GameState::new(map, vec![h]);
        state.advance_turn();
        assert_eq!(state.turn, 2);
        assert_eq!(state.hero(1).unwrap().mov_remaining, 4);
    }

    #[test]
    fn attack_hero_applies_damage() {
        let map = meadow_map(5, 5);
        let heroes = vec![
            player(1, MapCoord::new(0, 0)),
            enemy(2, MapCoord::new(1, 0)),
        ];
        let mut state = GameState::new(map, heroes);
        let mut rng = SeededRng::new("fight");
        state.attack_hero(1, 2, &mut rng).unwrap();
        // Enemy should have taken damage (strong player vs weak enemy)
        assert!(state.hero(2).unwrap().hp < 30);
    }

    #[test]
    fn defeated_enemy_awards_score() {
        let map = meadow_map(5, 5);
        // Player with overwhelming stats, enemy with minimal hp
        let p = Hero::new(1, "P", 100, 200, 0, 10, 4, MapCoord::new(0, 0), Team::player());
        let e = Hero::new(2, "E",   1,   1, 0,  1, 3, MapCoord::new(1, 0), Team::enemy());
        let mut state = GameState::new(map, vec![p, e]);
        let mut rng = SeededRng::new("kill");
        state.attack_hero(1, 2, &mut rng).unwrap();
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
