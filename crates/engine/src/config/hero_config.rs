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
use crate::hero_candidate::HeroCandidate;
// ─── YAML schema ──────────────────────────────────────────────────────────────

/// Top-level YAML structure — maps logical hero name → [`HeroCandidate`].
///
/// Only the fields the engine consumes are declared; serde ignores the rest
/// (atlas index, stats, description) by default.
#[derive(Debug, Clone, Deserialize)]
struct HeroFile {
    heroes: BTreeMap<String, HeroCandidate>,
}

// ─── HeroCatalog ──────────────────────────────────────────────────────────────

/// Per-class hero data loaded from `assets/heroes.yaml`.
#[derive(Debug, Clone, Default)]
pub struct HeroCatalog {
    heroes: Vec<HeroCandidate>,
}

impl HeroCatalog {
    /// Parses a [`HeroCatalog`] from a YAML string.
    ///
    /// # Errors
    /// Returns [`EngineError::InvalidTiles`] if the YAML is malformed.
    pub fn from_yaml(yaml: &str) -> Result<Self, EngineError> {
        let file: HeroFile = serde_yaml::from_str(yaml)
            .map_err(|e| EngineError::InvalidTiles(format!("failed to parse heroes YAML: {e}")))?;

        let heroes = file.heroes.into_values().collect();

        Ok(Self { heroes })
    }

    /// Returns all configured hero candidates in catalogue order.
    pub fn heroes(&self) -> &[HeroCandidate] {
        &self.heroes
    }
}
