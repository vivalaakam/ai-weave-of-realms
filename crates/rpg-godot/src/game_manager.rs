//! `GameManager` GDExtension node — owns [`GameState`] and drives the turn loop.
//!
//! Attach one `GameManager` node to the scene tree (usually as an autoload or
//! a direct child of the main scene).  Call [`GameManager::new_game`] to
//! generate a map and start a session, then use the remaining `#[func]` methods
//! to move heroes, resolve combat, and advance turns.
//!
//! ## Signals
//! | Signal | Args | When |
//! |--------|------|------|
//! | `map_ready` | — | map generated successfully |
//! | `hero_moved` | hero_id, from_x, from_y, to_x, to_y | hero repositioned |
//! | `combat_resolved` | attacker_id, defender_id, att_dmg, def_dmg, att_alive, def_alive | combat finished |
//! | `hero_defeated` | hero_id | hero hp reached 0 |
//! | `turn_advanced` | turn | turn counter incremented |
//! | `score_changed` | score | score board updated |

use godot::classes::ProjectSettings;
use godot::prelude::*;
use tracing::{debug, error, warn};

use rpg_engine::game_state::{GameState, TurnEvent};
use rpg_engine::hero::{Hero, Team};
use rpg_engine::map::game_map::MapCoord;
use rpg_engine::map::tile::Tiles;
use rpg_engine::movement;
use rpg_engine::rng::SeededRng;
use rpg_engine::spawn;
use rpg_engine::Direction;
use rpg_mapgen::map_assembler::{MapAssembler, MapConfig};
use rpg_mapgen::spawner::EnemySpawner;

// ─── GameManager ──────────────────────────────────────────────────────────────

#[derive(GodotClass)]
#[class(base=Node)]
pub struct GameManager {
    base: Base<Node>,
    state: Option<GameState>,
    rng: Option<SeededRng>,
    enemy_spawner: Option<EnemySpawner>,
}

#[godot_api]
impl INode for GameManager {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            state: None,
            rng: None,
            enemy_spawner: None,
        }
    }
}

#[godot_api]
impl GameManager {
    // ── Signals ───────────────────────────────────────────────────────────────

    #[signal]
    fn map_ready();

    #[signal]
    fn hero_moved(hero_id: i64, to_x: i64, to_y: i64);

    #[signal]
    fn combat_resolved(
        attacker_id: i64,
        defender_id: i64,
        att_dmg: i64,
        def_dmg: i64,
        att_alive: bool,
        def_alive: bool,
    );

    #[signal]
    fn hero_defeated(hero_id: i64);

    #[signal]
    fn turn_advanced(turn: i64);

    #[signal]
    fn score_changed(score: i64);

    #[signal]
    fn enemies_spawned(count: i64);

    // ── Session setup ─────────────────────────────────────────────────────────

    /// Generates a map and initialises a new game session.
    ///
    /// `generator` is a path to a Lua generator script (relative to the
    /// working directory or absolute).
    ///
    /// Returns `true` on success; emits `map_ready`.
    #[func]
    pub fn new_game(&mut self, seed: GString, generator: GString, width: i32, height: i32) -> bool {
        let seed_str = seed.to_string();
        // Convert res:// path to an OS-absolute path so the Lua engine can open it.
        let gen_str = generator.to_string();
        let gen_abs = if gen_str.starts_with("res://") {
            ProjectSettings::singleton()
                .globalize_path(&gen_str)
                .to_string()
        } else {
            gen_str
        };
        let gen_path = std::path::PathBuf::from(gen_abs);

        let mut config = MapConfig::default_3x3(seed_str.clone(), gen_path);
        config.width = width as u32;
        config.height = height as u32;

        let assembler = match MapAssembler::new(config) {
            Ok(a) => a,
            Err(e) => {
                error!("map assembler init failed: {e}");
                return false;
            }
        };

        let map = match assembler.generate() {
            Ok(m) => m,
            Err(e) => {
                error!("map generation failed: {e}");
                return false;
            }
        };

        self.rng = Some(SeededRng::new(&seed_str));
        self.state = Some(GameState::new(map, Vec::new()));

        // Load enemy spawn script if not already loaded
        if self.enemy_spawner.is_none() {
            let spawn_script = std::path::PathBuf::from(
                ProjectSettings::singleton()
                    .globalize_path(&GString::from("res://scripts/rules/spawn_enemies.lua"))
                    .to_string(),
            );
            match EnemySpawner::from_script(&spawn_script) {
                Ok(spawner) => {
                    debug!(path = %spawn_script.display(), "enemy spawner loaded");
                    self.enemy_spawner = Some(spawner);
                }
                Err(e) => {
                    warn!("failed to load enemy spawner: {e}");
                }
            }
        }

        // Defer signal emission to avoid borrow conflicts in signal handlers
        self.base_mut().call_deferred(
            "emit_signal",
            &["score_changed".to_variant(), 0i64.to_variant()],
        );
        self.base_mut()
            .call_deferred("emit_signal", &["map_ready".to_variant()]);
        true
    }

    /// Adds a hero to the current session.
    ///
    /// Movement points are derived automatically from `spd` as `20 + spd`.
    ///
    /// `team_name` is a human-readable team identifier (e.g. `"player"`, `"enemy"`).
    /// `player_controlled` indicates whether the human player commands this hero.
    #[func]
    #[allow(clippy::too_many_arguments)]
    pub fn add_hero(
        &mut self,
        id: i64,
        name: GString,
        hp: i64,
        atk: i64,
        def: i64,
        spd: i64,
        x: i64,
        y: i64,
        team_name: GString,
        player_controlled: bool,
    ) -> bool {
        let Some(state) = &mut self.state else {
            return false;
        };
        let team = Team::new(team_name.to_string(), player_controlled);
        let hero_rng = self
            .rng
            .as_ref()
            .map(|r| r.derive_for_hero(id as u32))
            .unwrap_or_else(|| rpg_engine::rng::SeededRng::new(&format!("hero_{id}")));
        let hero = Hero::new(
            id as u32,
            name.to_string(),
            hp as u32,
            atk as u32,
            def as u32,
            spd as u32,
            MapCoord::new(x as u32, y as u32),
            team,
            hero_rng,
        );
        state.heroes.push(hero);
        true
    }

    /// Spawns enemies on the map using the Lua spawn script.
    ///
    /// Returns the number of enemies spawned.
    #[func]
    pub fn spawn_enemies(&mut self) -> i64 {
        let Some(state) = &self.state else {
            return 0;
        };
        let Some(spawner) = &self.enemy_spawner else {
            debug!("enemy spawner not loaded");
            return 0;
        };

        match spawner.spawn(&state.map) {
            Ok(spawns) => {
                let count = spawns.len() as i64;
                if count == 0 {
                    return 0;
                }

                // Add enemies to game state, giving each a derived RNG
                let base_rng = self
                    .rng
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| rpg_engine::rng::SeededRng::new("fallback-spawn"));
                for spawn in spawns {
                    let hero = spawn.into_hero(&base_rng);
                    if let Some(state) = &mut self.state {
                        state.heroes.push(hero);
                    }
                }

                debug!(count, "enemies spawned");
                self.base_mut().call_deferred(
                    "emit_signal",
                    &["enemies_spawned".to_variant(), count.to_variant()],
                );
                count
            }
            Err(e) => {
                warn!("enemy spawn failed: {e}");
                0
            }
        }
    }

    /// Returns the number of living AI-controlled heroes on the map.
    #[func]
    pub fn get_enemy_count(&self) -> i64 {
        let Some(state) = &self.state else { return 0 };
        state.living_heroes(false).len() as i64
    }

    /// Returns the team name of hero `hero_id`, or empty string if not found.
    #[func]
    pub fn get_hero_team_name(&self, hero_id: i64) -> GString {
        let Some(state) = &self.state else {
            return GString::default();
        };
        state
            .heroes
            .iter()
            .find(|h| h.id == hero_id as u32)
            .map(|h| GString::from(h.team.name.as_str()))
            .unwrap_or_default()
    }

    /// Returns whether hero `hero_id` belongs to a player-controlled team.
    #[func]
    pub fn is_hero_player_controlled(&self, hero_id: i64) -> bool {
        let Some(state) = &self.state else {
            return false;
        };
        state
            .heroes
            .iter()
            .find(|h| h.id == hero_id as u32)
            .map(|h| h.team.player_controlled)
            .unwrap_or(false)
    }

    // ── Actions ───────────────────────────────────────────────────────────────

    /// Moves hero `hero_id` one step in `direction`.
    ///
    /// `direction` is encoded as an integer:
    /// - `0` = North (−Y)
    /// - `1` = East  (+X)
    /// - `2` = South (+Y)
    /// - `3` = West  (−X)
    ///
    /// Returns `true` and emits `hero_moved` on success.
    #[func]
    pub fn move_hero(&mut self, hero_id: i64, direction: i64) -> bool {
        let dir = match direction {
            0 => Direction::North,
            1 => Direction::East,
            2 => Direction::South,
            3 => Direction::West,
            _ => {
                warn!("move_hero: unknown direction {direction}");
                return false;
            }
        };

        let events = match self.state.as_mut() {
            Some(state) => match state.move_hero(hero_id as u32, dir) {
                Ok(ev) => ev,
                Err(e) => {
                    warn!("move_hero {hero_id} dir={direction} failed: {e}");
                    return false;
                }
            },
            None => return false,
        };

        for ev in &events {
            if let TurnEvent::HeroMoved { hero_id, to, .. } = ev {
                self.base_mut().emit_signal(
                    "hero_moved",
                    &[
                        (*hero_id as i64).to_variant(),
                        (to.x as i64).to_variant(),
                        (to.y as i64).to_variant(),
                    ],
                );
            }
        }
        true
    }

    /// Resolves combat between `attacker_id` and `defender_id`.
    ///
    /// Emits `combat_resolved` and, if applicable, `hero_defeated`.
    #[func]
    pub fn attack_hero(&mut self, attacker_id: i64, defender_id: i64) -> bool {
        let events = {
            match &mut self.state {
                Some(state) => match state.attack_hero(attacker_id as u32, defender_id as u32) {
                    Ok(ev) => ev,
                    Err(e) => {
                        warn!("attack_hero failed: {e}");
                        return false;
                    }
                },
                None => return false,
            }
        };

        for ev in &events {
            match ev {
                TurnEvent::CombatResolved { result, .. } => {
                    self.base_mut().emit_signal(
                        "combat_resolved",
                        &[
                            attacker_id.to_variant(),
                            defender_id.to_variant(),
                            (result.attacker_damage as i64).to_variant(),
                            (result.defender_damage as i64).to_variant(),
                            result.attacker_survived.to_variant(),
                            result.defender_survived.to_variant(),
                        ],
                    );
                }
                TurnEvent::HeroDefeated { hero_id } => {
                    self.base_mut()
                        .emit_signal("hero_defeated", &[(*hero_id as i64).to_variant()]);
                }
                _ => {}
            }
        }
        true
    }

    /// Advances to the next turn; resets movement and emits `turn_advanced`.
    #[func]
    pub fn advance_turn(&mut self) {
        let events = match self.state.as_mut() {
            Some(state) => state.advance_turn(),
            None => return,
        };

        let score = self
            .state
            .as_ref()
            .map(|s| s.score.total() as i64)
            .unwrap_or(0);

        for ev in &events {
            if let TurnEvent::TurnAdvanced { turn } = ev {
                self.base_mut()
                    .emit_signal("turn_advanced", &[(*turn as i64).to_variant()]);
            }
        }
        self.base_mut()
            .emit_signal("score_changed", &[score.to_variant()]);
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Returns the current turn number (starts at 1).
    #[func]
    pub fn get_turn(&self) -> i64 {
        self.state
            .as_ref()
            .map(|s| s.get_turn() as i64)
            .unwrap_or(0)
    }

    /// Returns the movement points remaining for hero `hero_id`, or -1 if not found.
    #[func]
    pub fn get_hero_mov_remaining(&self, hero_id: i64) -> i64 {
        let Some(state) = &self.state else { return -1 };
        state
            .heroes
            .iter()
            .find(|h| h.id == hero_id as u32)
            .map(|h| h.mov_remaining as i64)
            .unwrap_or(-1)
    }

    /// Returns the maximum movement points for hero `hero_id`, or -1 if not found.
    #[func]
    pub fn get_hero_mov_max(&self, hero_id: i64) -> i64 {
        let Some(state) = &self.state else { return -1 };
        state
            .heroes
            .iter()
            .find(|h| h.id == hero_id as u32)
            .map(|h| h.mov as i64)
            .unwrap_or(-1)
    }

    /// Returns the current total score.
    #[func]
    pub fn get_score(&self) -> i64 {
        self.state
            .as_ref()
            .map(|s| s.score.total() as i64)
            .unwrap_or(0)
    }

    /// Returns the tile kind string at `(x, y)`, e.g. `"meadow"`.
    #[func]
    pub fn get_tile_kind(&self, x: i64, y: i64) -> GString {
        let Some(state) = &self.state else {
            return GString::default();
        };
        state
            .map
            .get_tile(MapCoord::new(x as u32, y as u32))
            .map(|t| GString::from(t.kind.as_str()))
            .unwrap_or_default()
    }

    /// Returns the TMX GID for the tile at `(x, y)` (1-based; 0 = out of bounds).
    #[func]
    pub fn get_tile_gid(&self, x: i64, y: i64) -> i64 {
        let Some(state) = &self.state else { return 0 };
        state
            .map
            .get_tile(MapCoord::new(x as u32, y as u32))
            .map(|t| t.kind.to_gid() as i64)
            .unwrap_or(0)
    }

    /// Returns the map width in tiles.
    #[func]
    pub fn get_map_width(&self) -> i64 {
        self.state
            .as_ref()
            .map(|s| s.map.tile_width() as i64)
            .unwrap_or(0)
    }

    /// Returns the map height in tiles.
    #[func]
    pub fn get_map_height(&self) -> i64 {
        self.state
            .as_ref()
            .map(|s| s.map.tile_height() as i64)
            .unwrap_or(0)
    }

    /// Returns all tiles reachable by hero `hero_id` this turn as `Array[Vector2i]`.
    #[func]
    pub fn get_reachable_tiles(&self, hero_id: i64) -> Array<Vector2i> {
        let Some(state) = &self.state else {
            return Array::new();
        };
        let Some(hero) = state.heroes.iter().find(|h| h.id == hero_id as u32) else {
            return Array::new();
        };
        let coords = movement::reachable_tiles(&state.map, hero.position, hero.mov_remaining);
        let mut arr = Array::new();
        for c in coords {
            arr.push(Vector2i::new(c.x as i32, c.y as i32));
        }
        arr
    }

    /// Returns the id of the living hero at tile `(x, y)`, or -1 if none.
    #[func]
    pub fn get_hero_id_at_position(&self, x: i64, y: i64) -> i64 {
        let Some(state) = &self.state else { return -1 };
        let coord = MapCoord::new(x as u32, y as u32);
        state.hero_at(coord).map(|h| h.id as i64).unwrap_or(-1)
    }

    /// Returns the current position of hero `hero_id`, or `(-1, -1)` if not found.
    #[func]
    pub fn get_hero_position(&self, hero_id: i64) -> Vector2i {
        let Some(state) = &self.state else {
            return Vector2i::new(-1, -1);
        };
        state
            .heroes
            .iter()
            .find(|h| h.id == hero_id as u32)
            .map(|h| Vector2i::new(h.position.x as i32, h.position.y as i32))
            .unwrap_or(Vector2i::new(-1, -1))
    }

    /// Returns whether hero `hero_id` is still alive.
    #[func]
    pub fn is_hero_alive(&self, hero_id: i64) -> bool {
        let Some(state) = &self.state else {
            return false;
        };
        state
            .heroes
            .iter()
            .find(|h| h.id == hero_id as u32)
            .map(|h| h.is_alive())
            .unwrap_or(false)
    }

    /// Returns the recommended player spawn tile, or `(-1, -1)` if unavailable.
    #[func]
    pub fn get_player_spawn(&self) -> Vector2i {
        let Some(state) = &self.state else {
            return Vector2i::new(-1, -1);
        };
        spawn::find_player_spawn(&state.map)
            .map(|coord| Vector2i::new(coord.x as i32, coord.y as i32))
            .unwrap_or(Vector2i::new(-1, -1))
    }

    /// Returns the recommended enemy spawn tile, or `(-1, -1)` if unavailable.
    #[func]
    pub fn get_enemy_spawn(&self) -> Vector2i {
        let Some(state) = &self.state else {
            return Vector2i::new(-1, -1);
        };
        spawn::find_spawn_positions(&state.map)
            .map(|positions| Vector2i::new(positions.enemy.x as i32, positions.enemy.y as i32))
            .unwrap_or(Vector2i::new(-1, -1))
    }

    /// Returns ids of all living player-controlled heroes in stable insertion order.
    #[func]
    pub fn get_living_player_hero_ids(&self) -> Array<i64> {
        let Some(state) = &self.state else {
            return Array::new();
        };
        let mut ids = Array::new();
        for hero in state
            .heroes
            .iter()
            .filter(|h| h.team.player_controlled && h.is_alive())
        {
            ids.push(hero.id as i64);
        }
        ids
    }

    /// Returns the next available hero ID (maximum existing hero ID + 1, or 1 if no heroes exist).
    ///
    /// Use this to generate a unique ID before calling [`add_hero`](Self::add_hero).
    #[func]
    pub fn get_next_hero_id(&self) -> i64 {
        let Some(state) = &self.state else { return 1 };
        state
            .heroes
            .iter()
            .map(|h| h.id as i64)
            .max()
            .unwrap_or(0)
            + 1
    }

    /// Returns `true` if the tile at `(x, y)` is a [`Tiles::City`] or [`Tiles::CityEntrance`].
    ///
    /// Returns `false` for out-of-bounds coordinates or when no game session is active.
    #[func]
    pub fn is_city_tile(&self, x: i64, y: i64) -> bool {
        let Some(state) = &self.state else { return false };
        state
            .map
            .get_tile(MapCoord::new(x as u32, y as u32))
            .map(|t| matches!(t.kind, Tiles::City | Tiles::CityEntrance))
            .unwrap_or(false)
    }

    /// Returns ids of all living AI-controlled heroes in stable insertion order.
    #[func]
    pub fn get_living_enemy_hero_ids(&self) -> Array<i64> {
        let Some(state) = &self.state else {
            return Array::new();
        };
        let mut ids = Array::new();
        for hero in state
            .heroes
            .iter()
            .filter(|h| !h.team.player_controlled && h.is_alive())
        {
            ids.push(hero.id as i64);
        }
        ids
    }
}
