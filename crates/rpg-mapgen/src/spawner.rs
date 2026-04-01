//! Enemy spawner — determines enemy spawn positions and stats via a Lua script.
//!
//! The Lua script must return a function with the signature:
//! ```lua
//! function spawn_enemies(map) -> array of enemy descriptors
//! ```
//!
//! Each enemy descriptor is a table with fields:
//! - `id`, `x`, `y` (tile position)
//! - `hp`, `atk`, `def`, `spd` (hero stats; movement is derived as `20 + spd`)

use std::path::Path;

use mlua::{Function, Lua, Table, Value};
use tracing::{debug, instrument};

use rpg_engine::hero::{Hero, Team};
use rpg_engine::map::game_map::{GameMap, MapCoord};
use rpg_engine::rng::SeededRng;

use crate::error::Error;
use crate::map_table::game_map_to_lua_table;

// ─── EnemySpawn ───────────────────────────────────────────────────────────────

/// Result of running the enemy spawn script.
#[derive(Debug, Clone)]
pub struct EnemySpawn {
    /// Unique identifier for this enemy (within the spawn call).
    pub id: u32,
    /// Starting position on the map.
    pub position: MapCoord,
    /// Hero stats for this enemy.
    pub hp: u32,
    pub atk: u32,
    pub def: u32,
    pub spd: u32,
}

impl EnemySpawn {
    /// Converts this spawn descriptor into a [`Hero`].
    ///
    /// Movement points are derived from `spd` via `Hero::movement_for_spd`.
    ///
    /// `base_rng` is the session RNG; the hero's personal RNG is derived from
    /// it via [`SeededRng::derive_for_hero`] so the result is reproducible.
    pub fn into_hero(self, base_rng: &SeededRng) -> Hero {
        Hero::new(
            self.id,
            format!("Enemy {}", self.id),
            self.hp,
            self.atk,
            self.def,
            self.spd,
            self.position,
            Team::enemy(),
            base_rng.derive_for_hero(self.id),
        )
    }
}

// ─── EnemySpawner ─────────────────────────────────────────────────────────────

/// Evaluates enemy spawn positions and stats by calling a Lua script.
pub struct EnemySpawner {
    /// The Lua VM that owns `func` and all intermediate values.
    lua: Lua,
    /// The compiled spawn function loaded from the script file.
    func: Function,
}

impl EnemySpawner {
    /// Loads a spawn script from the given file path.
    ///
    /// # Errors
    /// Returns [`Error::ScriptLoad`] if the file cannot be read or the script
    /// fails to compile or does not return a function.
    pub fn from_script(path: &Path) -> Result<Self, Error> {
        let source = std::fs::read_to_string(path)?;
        let lua = Lua::new();

        let func: Function = lua
            .load(&source)
            .set_name(path.to_string_lossy().as_ref())
            .eval()
            .map_err(|source| Error::ScriptLoad {
                path: path.to_string_lossy().into_owned(),
                source,
            })?;

        debug!(path = %path.display(), "enemy spawn script loaded");
        Ok(Self { lua, func })
    }

    /// Generates enemy spawn data by calling the Lua `spawn_enemies` function.
    ///
    /// # Returns
    /// A vector of [`EnemySpawn`] descriptors, or an empty vector if the script
    /// returns an empty table or an error occurs during placement.
    ///
    /// # Errors
    /// Returns [`Error::LuaExecution`] if the Lua function raises an error.
    #[instrument(skip(self, map))]
    pub fn spawn(&self, map: &GameMap) -> Result<Vec<EnemySpawn>, Error> {
        let map_table =
            game_map_to_lua_table(&self.lua, map).map_err(|source| Error::LuaExecution {
                function: "game_map_to_lua_table".into(),
                source,
            })?;

        let result: Value = self
            .func
            .call(map_table)
            .map_err(|source| Error::LuaExecution {
                function: "spawn_enemies".into(),
                source,
            })?;

        let enemies = parse_spawn_results(result)?;
        debug!(count = enemies.len(), "enemies spawned");
        Ok(enemies)
    }
}

/// Converts a Lua return value into a vector of [`EnemySpawn`].
///
/// Accepts a Lua table array (0-indexed or 1-indexed) where each element
/// is a table with `id`, `x`, `y`, `hp`, `atk`, `def`, `spd`, `mov` fields.
fn parse_spawn_results(value: Value) -> Result<Vec<EnemySpawn>, Error> {
    let table = match value {
        Value::Table(t) => t,
        _ => return Ok(Vec::new()),
    };

    let mut enemies = Vec::new();

    // Try 1-indexed array first (Lua convention)
    let len = table.len().unwrap_or(0) as usize;
    if len > 0 {
        for i in 1..=len {
            match table.get::<mlua::Table>(i) {
                Ok(entry) => {
                    if let Some(spawn) = parse_enemy_entry(entry, i as u32)? {
                        enemies.push(spawn);
                    }
                }
                Err(_) => {}
            }
        }
    } else {
        // Fallback: iterate as dict (0-indexed)
        for result in table.pairs::<mlua::Value, mlua::Value>() {
            match result {
                Ok((key, val)) => {
                    if let (mlua::Value::Integer(idx), mlua::Value::Table(entry)) = (key, val) {
                        if let Some(spawn) = parse_enemy_entry(entry, idx as u32)? {
                            enemies.push(spawn);
                        }
                    }
                }
                Err(_) => {}
            }
        }
    }

    Ok(enemies)
}

/// Parses a single enemy entry table from Lua.
fn parse_enemy_entry(table: Table, index: u32) -> Result<Option<EnemySpawn>, Error> {
    // Extract required fields with defaults
    let id: u32 = table.get("id").unwrap_or(index);
    let x: i64 = table.get("x").unwrap_or(0);
    let y: i64 = table.get("y").unwrap_or(0);
    let hp: u32 = table.get("hp").unwrap_or(30);
    let atk: u32 = table.get("atk").unwrap_or(10);
    let def: u32 = table.get("def").unwrap_or(5);
    let spd: u32 = table.get("spd").unwrap_or(5);

    // Skip invalid positions
    if x < 0 || y < 0 {
        return Ok(None);
    }

    Ok(Some(EnemySpawn {
        id,
        position: MapCoord::new(x as u32, y as u32),
        hp,
        atk,
        def,
        spd,
    }))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rpg_engine::map::tile::{Tile, Tiles};

    fn make_map() -> GameMap {
        let tiles = vec![Tile::new(Tiles::Meadow); 96 * 96];
        GameMap::new(96, 96, tiles, [0u8; 32]).unwrap()
    }

    fn make_spawner(script: &str) -> EnemySpawner {
        let lua = Lua::new();
        let func: Function = lua.load(script).eval().unwrap();
        EnemySpawner { lua, func }
    }

    #[test]
    fn empty_result_returns_empty_vector() {
        let spawner = make_spawner("return function() return {} end");
        let enemies = spawner.spawn(&make_map()).unwrap();
        assert!(enemies.is_empty());
    }

    #[test]
    fn valid_enemies_are_parsed() {
        let spawner = make_spawner(
            r#"
            return function(map)
                return {
                    { id = 1, x = 10, y = 20, hp = 50, atk = 15, def = 8, spd = 6, mov = 3 },
                    { id = 2, x = 30, y = 40, hp = 30, atk = 10, def = 5, spd = 5, mov = 3 },
                }
            end
            "#,
        );
        let enemies = spawner.spawn(&make_map()).unwrap();
        assert_eq!(enemies.len(), 2);
        assert_eq!(enemies[0].position, MapCoord::new(10, 20));
        assert_eq!(enemies[0].hp, 50);
        assert_eq!(enemies[1].position, MapCoord::new(30, 40));
    }

    #[test]
    fn missing_fields_get_defaults() {
        let spawner = make_spawner(
            r#"
            return function(map)
                return { { x = 5, y = 5 } }
            end
            "#,
        );
        let enemies = spawner.spawn(&make_map()).unwrap();
        assert_eq!(enemies.len(), 1);
        assert_eq!(enemies[0].hp, 30);
        assert_eq!(enemies[0].atk, 10);
    }

    #[test]
    fn negative_positions_are_skipped() {
        let spawner = make_spawner(
            r#"
            return function(map)
                return {
                    { x = -1, y = 5 },
                    { x = 10, y = 10 },
                }
            end
            "#,
        );
        let enemies = spawner.spawn(&make_map()).unwrap();
        assert_eq!(enemies.len(), 1);
    }

    #[test]
    fn convert_spawns_to_heroes() {
        let spawner = make_spawner(
            r#"
            return function(map)
                return {
                    { id = 1, x = 10, y = 20, hp = 50, atk = 15, def = 8, spd = 6, mov = 3 },
                }
            end
            "#,
        );
        let spawns = spawner.spawn(&make_map()).unwrap();
        let base_rng = SeededRng::new("test-session");
        let hero = spawns.into_iter().next().unwrap().into_hero(&base_rng);
        assert_eq!(hero.hp, 50);
        assert_eq!(hero.atk, 15);
    }
}
