//! Hero catalogue — static hero-class metadata loaded from YAML.
//!
//! [`HeroCatalog`] holds the per-class data that designers tune outside of
//! code, currently the gold hire cost. It is the single source of truth for
//! hero prices.
//!
//! # Usage
//!
//! The binary should call [`init_hero_catalog`](crate::config::init_hero_catalog)
//! once at start-up (e.g. in `main`), passing the contents of `assets/heroes.yaml`.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;

use serde::Deserialize;

use crate::error::EngineError;

// ─── YAML schema ──────────────────────────────────────────────────────────────

/// Top-level YAML structure — maps logical hero name → [`HeroEntry`].
///
/// Only the fields the engine consumes are declared; serde ignores the rest
/// (atlas index, stats, description) by default.
#[derive(Debug, Clone, Deserialize)]
struct HeroFile {
    heroes: BTreeMap<String, HeroEntry>,
}

/// A single hero-class entry, narrowed to the fields the engine needs.
#[derive(Debug, Clone, Copy, Deserialize)]
struct HeroEntry {
    class_id: u8,
    cost: u32,
}

// ─── HeroCatalog ──────────────────────────────────────────────────────────────

/// Per-class hero data loaded from `assets/heroes.yaml`.
#[derive(Debug, Clone, Default)]
pub struct HeroCatalog {
    /// Hire cost in gold, keyed by stable `class_id`.
    cost_by_class: BTreeMap<u8, u32>,
}

impl HeroCatalog {
    /// Parses a [`HeroCatalog`] from a YAML string.
    ///
    /// # Errors
    /// Returns [`EngineError::InvalidTiles`] if the YAML is malformed.
    pub fn from_yaml(yaml: &str) -> Result<Self, EngineError> {
        let file: HeroFile = serde_yaml::from_str(yaml)
            .map_err(|e| EngineError::InvalidTiles(format!("failed to parse heroes YAML: {e}")))?;

        let cost_by_class =
            file.heroes.into_values().map(|entry| (entry.class_id, entry.cost)).collect();

        Ok(Self { cost_by_class })
    }

    /// Returns the hire cost for the given `class_id`, if defined.
    pub fn hire_cost(&self, class_id: u8) -> Option<u32> {
        self.cost_by_class.get(&class_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards the real `assets/heroes.yaml` shipped with the game: it must
    /// parse, cover all 24 classes, and price every hero as a multiple of 5
    /// within the 40–60 range.
    #[test]
    fn ships_valid_hero_costs() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/heroes.yaml");
        let yaml = std::fs::read_to_string(path).expect("read assets/heroes.yaml");
        let catalog = HeroCatalog::from_yaml(&yaml).expect("parse assets/heroes.yaml");

        for class_id in 0..24u8 {
            let cost = catalog.hire_cost(class_id).expect("every class has a cost");
            assert!((40..=60).contains(&cost), "class {class_id} cost {cost} out of range");
            assert_eq!(cost % 5, 0, "class {class_id} cost {cost} not a multiple of 5");
        }
    }
}
