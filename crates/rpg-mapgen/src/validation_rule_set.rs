//! Validation rule set — loads and runs multiple Lua rule files from a directory.
//!
//! Each `.lua` file in the directory is treated as an independent validation rule.
//! Rules are loaded in sorted (alphabetical) order so that numbering like
//! `01_passable_terrain.lua`, `02_impassable_limit.lua` gives a deterministic run order.
//!
//! ## Lua contract
//! Each rule file must return a function with the same signature as a single validator:
//! ```lua
//! return function(map) -> boolean, string|nil
//! ```

use std::path::Path;

use tracing::{debug, info, instrument, warn};

use rpg_engine::map::game_map::GameMap;

use crate::error::Error;
use crate::validator::{MapValidator, ValidationResult};

// ─── ValidationRuleSet ────────────────────────────────────────────────────────

/// A collection of [`MapValidator`] rules loaded from a directory of `.lua` files.
///
/// Rules are loaded in sorted file-name order and run sequentially.
/// All results are collected — the set does *not* stop at the first failure.
pub struct ValidationRuleSet {
    /// Validators in sorted (load) order.
    rules: Vec<(String, MapValidator)>,
}

impl ValidationRuleSet {
    /// Loads all `.lua` files from `dir` as independent validation rules.
    ///
    /// Files are processed in alphabetical order by file name.
    /// Non-`.lua` files and sub-directories are silently skipped.
    ///
    /// # Errors
    /// Returns [`Error::ScriptLoad`] if any `.lua` file cannot be read or compiled.
    /// Returns [`Error::Engine`] with [`InvalidState`](rpg_engine::error::Error::InvalidState)
    /// if `dir` cannot be read.
    pub fn from_dir(dir: &Path) -> Result<Self, Error> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| {
                Error::Engine(rpg_engine::error::Error::InvalidState(format!(
                    "cannot read validation rule directory '{}': {e}",
                    dir.display()
                )))
            })?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|x| x == "lua")
                    .unwrap_or(false)
            })
            .collect();

        // Sort by file name for deterministic order (e.g. 01_..., 02_...)
        entries.sort_by_key(|e| e.file_name());

        let mut rules = Vec::with_capacity(entries.len());
        for entry in &entries {
            let path = entry.path();
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();

            debug!(rule = %name, "loading validation rule");
            let validator = MapValidator::from_script(&path)?;
            rules.push((name, validator));
        }

        info!(
            dir = %dir.display(),
            count = rules.len(),
            "validation rule set loaded"
        );

        Ok(Self { rules })
    }

    /// Returns the number of rules in this set.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Returns `true` if no rules are loaded.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Runs all rules against `map` and returns a result per rule.
    ///
    /// Execution continues even after failures so that all rule outcomes are
    /// available to the caller for diagnostics or reporting.
    ///
    /// # Errors
    /// Returns [`Error::LuaExecution`] if a rule raises a runtime error (as opposed
    /// to returning `false` — a soft validation failure).
    #[instrument(skip(self, map))]
    pub fn validate_all(&self, map: &GameMap) -> Result<Vec<RuleResult>, Error> {
        let mut results = Vec::with_capacity(self.rules.len());

        for (name, validator) in &self.rules {
            let vr = validator.validate(map)?;
            if vr.is_valid() {
                debug!(rule = %name, "rule passed");
            } else {
                warn!(rule = %name, reason = ?vr.reason, "rule failed");
            }
            results.push(RuleResult {
                rule: name.clone(),
                valid: vr.valid,
                reason: vr.reason,
            });
        }

        Ok(results)
    }
}

// ─── RuleResult ───────────────────────────────────────────────────────────────

/// The outcome of a single validation rule from a [`ValidationRuleSet`].
#[derive(Debug, Clone)]
pub struct RuleResult {
    /// File name of the rule script (e.g. `"01_passable_terrain.lua"`).
    pub rule: String,
    /// Whether the map passed this rule.
    pub valid: bool,
    /// Human-readable failure reason, or `None` if the rule passed.
    pub reason: Option<String>,
}

impl From<RuleResult> for ValidationResult {
    fn from(r: RuleResult) -> Self {
        ValidationResult {
            valid: r.valid,
            reason: r.reason,
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::fs;

    use rpg_engine::map::chunk::{Chunk, ChunkCoord};
    use rpg_engine::map::tile::{Tile, Tiles};

    use super::*;
    use crate::test_utils::init_tracing;

    fn make_uniform_map(kind: Tiles) -> GameMap {
        let chunks: Vec<Chunk> = (0..9u32)
            .map(|i| Chunk::filled(ChunkCoord::new(i % 3, i / 3), Tile::new(kind)))
            .collect();
        GameMap::new(3, 3, chunks, [0u8; 32]).unwrap()
    }

    fn write_rule(dir: &Path, name: &str, script: &str) {
        fs::write(dir.join(name), script).unwrap();
    }

    fn temp_rule_dir(suffix: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("rpg_rules_{suffix}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn empty_dir_returns_empty_set() {
        init_tracing();
        let dir = temp_rule_dir("empty");
        let rs = ValidationRuleSet::from_dir(&dir).unwrap();
        assert!(rs.is_empty());
        let map = make_uniform_map(Tiles::Meadow);
        assert!(rs.validate_all(&map).unwrap().is_empty());
    }

    #[test]
    fn single_passing_rule() {
        init_tracing();
        let dir = temp_rule_dir("single_pass");
        write_rule(&dir, "01_always_pass.lua", "return function(m) return true, nil end");
        let rs = ValidationRuleSet::from_dir(&dir).unwrap();
        assert_eq!(rs.len(), 1);
        let map = make_uniform_map(Tiles::Meadow);
        let results = rs.validate_all(&map).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].valid);
    }

    #[test]
    fn multiple_rules_all_run() {
        init_tracing();
        let dir = temp_rule_dir("multi_run");
        write_rule(&dir, "01_pass.lua", "return function(m) return true, nil end");
        write_rule(&dir, "02_fail.lua", r#"return function(m) return false, "bad map" end"#);
        let rs = ValidationRuleSet::from_dir(&dir).unwrap();
        let map = make_uniform_map(Tiles::Meadow);
        let results = rs.validate_all(&map).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].valid);
        assert!(!results[1].valid);
        assert_eq!(results[1].reason.as_deref(), Some("bad map"));
    }

    #[test]
    fn rules_run_in_sorted_order() {
        init_tracing();
        let dir = temp_rule_dir("sorted");
        // Write in reverse order to ensure sort overrides fs ordering
        write_rule(&dir, "03_c.lua", "return function(m) return true, nil end");
        write_rule(&dir, "01_a.lua", "return function(m) return true, nil end");
        write_rule(&dir, "02_b.lua", "return function(m) return true, nil end");
        let rs = ValidationRuleSet::from_dir(&dir).unwrap();
        let names: Vec<_> = rs.rules.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["01_a.lua", "02_b.lua", "03_c.lua"]);
    }

    #[test]
    fn non_lua_files_are_skipped() {
        init_tracing();
        let dir = temp_rule_dir("non_lua");
        write_rule(&dir, "README.md", "# not a rule");
        write_rule(&dir, "01_rule.lua", "return function(m) return true, nil end");
        let rs = ValidationRuleSet::from_dir(&dir).unwrap();
        assert_eq!(rs.len(), 1);
    }
}
