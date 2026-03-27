//! Map validator — checks a [`GameMap`] against rules defined in a Lua script.
//!
//! The Lua script must return a function with the signature:
//! ```lua
//! function validate(map) -> boolean, string|nil
//! ```
//! On success the function returns `(true, nil)`.
//! On failure it returns `(false, "human-readable reason")`.

use std::path::Path;

use mlua::{Function, Lua, MultiValue};
use tracing::{debug, instrument, warn};

use rpg_engine::map::game_map::GameMap;

use crate::error::Error;
use crate::map_table::game_map_to_lua_table;

// ─── ValidationResult ─────────────────────────────────────────────────────────

/// The outcome of a map validation run.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the map passed all validation rules.
    pub valid: bool,
    /// Human-readable failure reason, or `None` if the map is valid.
    pub reason: Option<String>,
}

impl ValidationResult {
    /// Returns `true` if the map passed validation.
    pub fn is_valid(&self) -> bool {
        self.valid
    }
}

// ─── MapValidator ─────────────────────────────────────────────────────────────

/// Validates a [`GameMap`] by calling a Lua `validate(map)` function.
pub struct MapValidator {
    /// The Lua VM that owns `func` and all intermediate values.
    lua: Lua,
    /// The compiled `validate` function loaded from the script file.
    func: Function,
}

impl MapValidator {
    /// Loads a validator script from the given file path.
    ///
    /// The script must evaluate to (return) a Lua function.
    ///
    /// # Errors
    /// Returns [`Error::ScriptLoad`] if the file cannot be read or the script
    /// fails to compile / does not return a function.
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

        debug!(path = %path.display(), "validator script loaded");
        Ok(Self { lua, func })
    }

    /// Validates the given map by calling the Lua `validate` function.
    ///
    /// # Returns
    /// A [`ValidationResult`] indicating whether the map is valid and, if not,
    /// why it failed.
    ///
    /// # Errors
    /// Returns [`Error::LuaExecution`] if the Lua function raises a runtime error
    /// (distinct from a soft validation failure).
    #[instrument(skip(self, map))]
    pub fn validate(&self, map: &GameMap) -> Result<ValidationResult, Error> {
        let map_table = game_map_to_lua_table(&self.lua, map).map_err(|source| {
            Error::LuaExecution {
                function: "game_map_to_lua_table".into(),
                source,
            }
        })?;

        let multi: MultiValue = self
            .func
            .call(map_table)
            .map_err(|source| Error::LuaExecution {
                function: "validate".into(),
                source,
            })?;

        let result = parse_validation_result(multi)?;

        if result.valid {
            debug!("map validation passed");
        } else {
            warn!(reason = ?result.reason, "map validation failed");
        }

        Ok(result)
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Parses the `(boolean, string|nil)` multi-return from the Lua validator.
///
/// # Errors
/// Returns [`Error::LuaExecution`] if the first return value is not a boolean.
fn parse_validation_result(mut multi: MultiValue) -> Result<ValidationResult, Error> {
    // MultiValue is ordered: first value = valid, second value = reason
    let valid = match multi.pop_front() {
        Some(mlua::Value::Boolean(b)) => b,
        other => {
            return Err(Error::LuaExecution {
                function: "validate (return #1)".into(),
                source: mlua::Error::runtime(format!(
                    "expected boolean, got {other:?}"
                )),
            });
        }
    };

    let reason = match multi.pop_front() {
        Some(mlua::Value::String(s)) => Some(s.to_str().map(|b| b.to_string()).unwrap_or_default()),
        Some(mlua::Value::Nil) | None => None,
        other => Some(format!("{other:?}")),
    };

    Ok(ValidationResult { valid, reason })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Function;
    use rpg_engine::map::chunk::{Chunk, ChunkCoord};
    use rpg_engine::map::tile::{Tile, Tiles};
    use crate::test_utils::init_tracing;

    fn make_validator(script: &str) -> MapValidator {
        let lua = Lua::new();
        let func: Function = lua.load(script).eval().unwrap();
        MapValidator { lua, func }
    }

    fn make_uniform_map(kind: Tiles) -> GameMap {
        let chunks: Vec<Chunk> = (0..9u32)
            .map(|i| Chunk::filled(ChunkCoord::new(i % 3, i / 3), Tile::new(kind)))
            .collect();
        GameMap::new(3, 3, chunks, [0u8; 32]).unwrap()
    }

    #[test]
    fn always_valid_script_passes() {
        init_tracing();
        let v = make_validator("return function(map) return true, nil end");
        let map = make_uniform_map(Tiles::Meadow);
        let result = v.validate(&map).unwrap();
        assert!(result.is_valid());
        assert!(result.reason.is_none());
    }

    #[test]
    fn always_invalid_script_fails_with_reason() {
        init_tracing();
        let v = make_validator(r#"return function(map) return false, "no good" end"#);
        let map = make_uniform_map(Tiles::Meadow);
        let result = v.validate(&map).unwrap();
        assert!(!result.is_valid());
        assert_eq!(result.reason.as_deref(), Some("no good"));
    }

    #[test]
    fn all_water_map_fails_passability_check() {
        init_tracing();
        let script = r#"
            return function(map)
                local pass = 0
                for _, k in ipairs(map.tiles) do
                    if k ~= "water" and k ~= "mountain" then pass = pass + 1 end
                end
                if pass / #map.tiles < 0.50 then
                    return false, "too little passable terrain"
                end
                return true, nil
            end
        "#;
        let v = make_validator(script);
        let map = make_uniform_map(Tiles::Water);
        let result = v.validate(&map).unwrap();
        assert!(!result.is_valid());
        assert!(result.reason.unwrap().contains("passable"));
    }

    #[test]
    fn grass_map_passes_passability_check() {
        init_tracing();
        let script = r#"
            return function(map)
                local pass = 0
                for _, k in ipairs(map.tiles) do
                    if k ~= "water" and k ~= "mountain" then pass = pass + 1 end
                end
                if pass / #map.tiles < 0.50 then
                    return false, "too little passable terrain"
                end
                return true, nil
            end
        "#;
        let v = make_validator(script);
        let map = make_uniform_map(Tiles::Meadow);
        let result = v.validate(&map).unwrap();
        assert!(result.is_valid());
    }
}
