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
use rpg_engine::hero::{Hero, HeroId, TeamId};
use rpg_engine::map::game_map::MapCoord;
use rpg_engine::map::tile::Tiles;
use rpg_engine::movement;
use rpg_engine::rng::SeededRng;
use rpg_engine::spawn;
use rpg_engine::team::Team;
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

    /// Emitted when any tile in a city complex changes ownership.
    ///
    /// `team_name` is empty when the city becomes neutral.
    #[signal]
    fn city_owner_changed(x: i64, y: i64, team_name: GString);

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
        let mut state = GameState::new(map);
        state.add_team(Team::red());
        state.add_team(Team::blue());
        state.add_team(Team::enemy());
        self.state = Some(state);

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

    /// Adds a hero to the current session, auto-assigning `id = heroes.len()`.
    ///
    /// Movement points are derived automatically from `spd` as `20 + spd`.
    ///
    /// `team_id` must match the `id` of a [`Team`] registered in the game state.
    ///
    /// Returns the assigned hero id, or `-1` if no session is active.
    #[func]
    #[allow(clippy::too_many_arguments)]
    pub fn add_hero(
        &mut self,
        name: GString,
        hp: i64,
        atk: i64,
        def: i64,
        spd: i64,
        pos: Vector2i,
        team_id: i64,
    ) -> i64 {
        let Some(state) = &mut self.state else {
            return -1;
        };
        let next_id = state.heroes.len() as HeroId;
        let hero_rng = self
            .rng
            .as_ref()
            .map(|r| r.derive_for_hero(next_id as u32))
            .unwrap_or_else(|| SeededRng::new(&format!("hero_{next_id}")));
        let hero = Hero::new(
            next_id,
            name.to_string(),
            hp as u32,
            atk as u32,
            def as u32,
            spd as u32,
            MapCoord::new(pos.x as u32, pos.y as u32),
            team_id as TeamId,
            hero_rng,
        );
        state.add_hero(hero) as i64
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

                // Add enemies to game state, giving each a derived RNG.
                // Use the first non-player-controlled team id, falling back to Team::enemy().id.
                let enemy_team_id: TeamId = self
                    .state
                    .as_ref()
                    .and_then(|s| {
                        s.teams
                            .values()
                            .find(|t| !t.is_player_controlled())
                            .map(|t| t.get_id())
                    })
                    .unwrap_or(Team::enemy().get_id());

                let base_rng = self
                    .rng
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| SeededRng::new("fallback-spawn"));

                for spawn in spawns {
                    let hero = spawn.into_hero(&base_rng, enemy_team_id);
                    if let Some(state) = &mut self.state {
                        state.add_hero(hero);
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
            .hero(hero_id as HeroId)
            .and_then(|h| state.team_name_by_id(h.team_id))
            .map(GString::from)
            .unwrap_or_default()
    }

    /// Returns whether hero `hero_id` belongs to a player-controlled team.
    #[func]
    pub fn is_hero_player_controlled(&self, hero_id: i64) -> bool {
        let Some(state) = &self.state else {
            return false;
        };
        state
            .hero(hero_id as HeroId)
            .map(|h| state.is_player_controlled(h.team_id))
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
            Some(state) => match state.move_hero(hero_id as HeroId, dir) {
                Ok(ev) => ev,
                Err(e) => {
                    warn!("move_hero {hero_id} dir={direction} failed: {e}");
                    return false;
                }
            },
            None => return false,
        };

        for ev in &events {
            match ev {
                TurnEvent::HeroMoved { hero_id, to, .. } => {
                    self.base_mut().emit_signal(
                        "hero_moved",
                        &[
                            (*hero_id as i64).to_variant(),
                            (to.x as i64).to_variant(),
                            (to.y as i64).to_variant(),
                        ],
                    );
                }
                TurnEvent::CityOwnerChanged { coord, team_id } => {
                    let team_name = match team_id {
                        Some(id) => self
                            .state
                            .as_ref()
                            .and_then(|s| s.team_name_by_id(*id))
                            .unwrap_or("")
                            .to_string(),
                        None => String::new(),
                    };
                    self.base_mut().emit_signal(
                        "city_owner_changed",
                        &[
                            (coord.x as i64).to_variant(),
                            (coord.y as i64).to_variant(),
                            GString::from(team_name.as_str()).to_variant(),
                        ],
                    );
                }
                _ => {}
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
                Some(state) => {
                    match state.attack_hero(attacker_id as HeroId, defender_id as HeroId) {
                        Ok(ev) => ev,
                        Err(e) => {
                            warn!("attack_hero failed: {e}");
                            return false;
                        }
                    }
                }
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
            .hero(hero_id as HeroId)
            .map(|h| h.mov_remaining as i64)
            .unwrap_or(-1)
    }

    /// Returns the maximum movement points for hero `hero_id`, or -1 if not found.
    #[func]
    pub fn get_hero_mov_max(&self, hero_id: i64) -> i64 {
        let Some(state) = &self.state else { return -1 };
        state
            .hero(hero_id as HeroId)
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
        let Some(hero) = state.hero(hero_id as HeroId) else {
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
        state
            .hero_at(coord)
            .map(|h| h.get_id() as i64)
            .unwrap_or(-1)
    }

    /// Returns the current position of hero `hero_id`, or `(-1, -1)` if not found.
    #[func]
    pub fn get_hero_position(&self, hero_id: i64) -> Vector2i {
        let Some(state) = &self.state else {
            return Vector2i::new(-1, -1);
        };
        state
            .hero(hero_id as HeroId)
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
            .hero(hero_id as HeroId)
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
        for (_, hero) in state
            .heroes
            .iter()
            .filter(|(_, h)| state.is_player_controlled(h.team_id) && h.is_alive())
        {
            ids.push(hero.get_id() as i64);
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
            .values()
            .map(|h| h.get_id() as i64)
            .max()
            .unwrap_or(0)
            + 1
    }

    /// Returns `true` if the tile at `(x, y)` is a [`Tiles::City`] or [`Tiles::CityEntrance`].
    ///
    /// Returns `false` for out-of-bounds coordinates or when no game session is active.
    #[func]
    pub fn is_city_tile(&self, x: i64, y: i64) -> bool {
        let Some(state) = &self.state else {
            return false;
        };
        state
            .map
            .get_tile(MapCoord::new(x as u32, y as u32))
            .map(|t| matches!(t.kind, Tiles::City | Tiles::CityEntrance))
            .unwrap_or(false)
    }

    /// Returns the name of the team that owns the city at `(x, y)`, or an empty
    /// string if the city is neutral or the coordinates are out of bounds.
    #[func]
    pub fn get_city_owner(&self, x: i64, y: i64) -> GString {
        let Some(state) = &self.state else {
            return GString::default();
        };
        let coord = MapCoord::new(x as u32, y as u32);
        state
            .city_owner(coord)
            .and_then(|id| state.team_name_by_id(id))
            .map(GString::from)
            .unwrap_or_default()
    }

    /// Assigns ownership of the city at `(x, y)` to `team_id`.
    ///
    /// BFS-floods all connected `City` / `CityEntrance` tiles so the entire
    /// city complex is claimed at once.  Pass -1 to mark neutral.
    /// Emits `city_owner_changed` for every tile whose owner changed.
    #[func]
    pub fn set_city_owner(&mut self, x: i64, y: i64, team_id: i64) {
        let coord = MapCoord::new(x as u32, y as u32);
        let tid = if team_id >= 0 {
            Some(team_id as u8)
        } else {
            None
        };
        let changed = {
            let Some(state) = &mut self.state else { return };
            state.set_city_owner(coord, tid)
        };
        let team_name = match tid {
            Some(id) => self
                .state
                .as_ref()
                .and_then(|s| s.team_name_by_id(id))
                .unwrap_or("")
                .to_string(),
            None => String::new(),
        };
        for c in changed {
            self.base_mut().emit_signal(
                "city_owner_changed",
                &[
                    (c.x as i64).to_variant(),
                    (c.y as i64).to_variant(),
                    GString::from(team_name.as_str()).to_variant(),
                ],
            );
        }
    }

    /// Returns the grid coordinates of every `City` (center body) tile on the map.
    ///
    /// City-entrance tiles are excluded — use this to place city-ownership markers
    /// at the visual center of each city complex.
    #[func]
    pub fn get_city_center_coords(&self) -> Array<Vector2i> {
        let Some(state) = &self.state else {
            return Array::new();
        };
        let w = state.map.tile_width();
        let h = state.map.tile_height();
        let mut arr = Array::new();
        for y in 0..h {
            for x in 0..w {
                if let Ok(tile) = state.map.get_tile(MapCoord::new(x, y)) {
                    if tile.kind == Tiles::City {
                        arr.push(Vector2i::new(x as i32, y as i32));
                    }
                }
            }
        }
        arr
    }

    /// Returns up to `count` city-entrance spawn points spread across the map.
    ///
    /// Uses greedy farthest-point selection so the returned tiles are as far apart
    /// as possible — ideal for placing multiple player team starting positions.
    #[func]
    pub fn find_city_entrance_spawns(&self, count: i64) -> Array<Vector2i> {
        let Some(state) = &self.state else {
            return Array::new();
        };
        let spawns = spawn::find_city_entrance_spawns(&state.map, count as usize);
        let mut arr = Array::new();
        for c in spawns {
            arr.push(Vector2i::new(c.x as i32, c.y as i32));
        }
        arr
    }

    /// Returns ids of all living AI-controlled heroes in stable insertion order.
    #[func]
    pub fn get_living_enemy_hero_ids(&self) -> Array<i64> {
        let Some(state) = &self.state else {
            return Array::new();
        };
        let mut ids = Array::new();
        for (_, hero) in state
            .heroes
            .iter()
            .filter(|(_, h)| !state.is_player_controlled(h.team_id) && h.is_alive())
        {
            ids.push(hero.get_id() as i64);
        }
        ids
    }

    /// Returns the last active hero ID for `team_id`, or -1 if not set.
    #[func]
    pub fn get_active_hero(&self, team_id: i64) -> i64 {
        let Some(state) = &self.state else { return -1 };
        state
            .get_active_hero(team_id as u8)
            .map(|id| id as i64)
            .unwrap_or(-1)
    }

    /// Sets the active hero for `team_id`.
    #[func]
    pub fn set_active_hero(&mut self, team_id: i64, hero_id: i64) {
        let Some(state) = &mut self.state else { return };
        let id = if hero_id >= 0 {
            Some(hero_id as HeroId)
        } else {
            None
        };
        state.set_active_hero(team_id as u8, id);
    }

    /// Returns the next hero for `team_id`, or -1 if none available.
    #[func]
    pub fn get_next_hero(&self, team_id: i64) -> i64 {
        let Some(state) = &self.state else { return -1 };
        state
            .get_next_hero(team_id as u8)
            .map(|id| id as i64)
            .unwrap_or(-1)
    }

    /// Clears all active hero selections.
    #[func]
    pub fn clear_active_heroes(&mut self) {
        let Some(state) = &mut self.state else { return };
        state.clear_active_heroes();
    }

    /// Returns the currently active team id.
    #[func]
    pub fn get_active_team_id(&self) -> i64 {
        let Some(state) = &self.state else { return -1 };
        match state.get_active_team_id() {
            Ok(id) => *id as i64,
            Err(_) => -1,
        }
    }

    /// Returns the currently active team name.
    #[func]
    pub fn get_active_team(&self) -> GString {
        let Some(state) = &self.state else {
            return GString::new();
        };
        GString::from(
            state
                .get_active_team()
                .map(|t| t.name.as_str())
                .unwrap_or(""),
        )
    }

    /// Returns the number of teams.
    #[func]
    pub fn get_team_count(&self) -> i64 {
        let Some(state) = &self.state else { return 0 };
        state.teams.len() as i64
    }

    /// Returns team id by index.
    #[func]
    pub fn get_team_id(&self, index: i64) -> i64 {
        let Some(state) = &self.state else { return -1 };
        state
            .teams
            .get(&(index as TeamId))
            .map(|t| t.get_id() as i64)
            .unwrap_or(-1)
    }

    /// Returns team name by id.
    #[func]
    pub fn get_team_name(&self, team_id: i64) -> GString {
        let Some(state) = &self.state else {
            return GString::new();
        };
        state
            .get_team(team_id as u8)
            .map(|t| GString::from(t.name.as_str()))
            .unwrap_or_default()
    }

    /// Returns team color as RGB tuple packed into i64 (R << 16 | G << 8 | B).
    #[func]
    pub fn get_team_color(&self, team_id: i64) -> i64 {
        let Some(state) = &self.state else { return 0 };
        state
            .get_team(team_id as u8)
            .map(|t| ((t.color.0 as i64) << 16) | ((t.color.1 as i64) << 8) | (t.color.2 as i64))
            .unwrap_or(0)
    }

    /// Returns whether team is player-controlled.
    #[func]
    pub fn is_team_player_controlled(&self, team_id: i64) -> bool {
        let Some(state) = &self.state else {
            return false;
        };
        state
            .get_team(team_id as u8)
            .map(|t| t.is_player_controlled())
            .unwrap_or(false)
    }

    /// Advances to the next player team.
    /// Returns true if the global turn should advance.
    #[func]
    pub fn get_next_active_team(&mut self) -> bool {
        let Some(state) = &mut self.state else {
            return false;
        };
        state.get_next_active_team().is_ok()
    }

    /// Begins the active team's turn: increments their per-team turn counter and
    /// resets movement for all living heroes of that team.
    ///
    /// Must be called once after [`reset_active_team`] at game start and once
    /// after each [`get_next_active_team`] call.
    #[func]
    pub fn on_turn(&mut self) {
        let Some(state) = &mut self.state else { return };
        let _ = state.on_turn();
    }

    /// Returns the current per-team turn counter for `team_id` (0 = not yet started).
    #[func]
    pub fn get_team_turn(&self, team_id: i64) -> i64 {
        let Some(state) = &self.state else { return 0 };
        state
            .get_team(team_id as u8)
            .map(|t| t.get_turn() as i64)
            .unwrap_or(0)
    }
}
