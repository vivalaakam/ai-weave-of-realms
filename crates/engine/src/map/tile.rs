//! Tile primitives: [`Tiles`] enum and [`Tile`] struct.
//!
//! [`Tiles`] is the canonical tile type for the whole project.
//! Static properties (colour, passability, movement cost, sprite variants) are
//! read from the runtime [`TileConfig`](crate::config::TileConfig) rather than
//! hard-coded.

use core::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::config::TileConfig;
use crate::error::EngineError;

// ─── Tiles ────────────────────────────────────────────────────────────────────

/// All terrain and object types available in the world tileset.
///
/// The enum variants themselves are a compile-time identity.  Numeric tile IDs,
/// colours, passability rules and other static data live in the global
/// [`TileConfig`](crate::config::TileConfig) so that games with custom tilesets
/// can override them from YAML without touching Rust code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tiles {
    Meadow,
    Forest,
    Mountain,
    Water,
    City,
    CityEntrance,
    Road,
    River,
    Bridge,
    Village,
    Merchant,
    Ruins,
    Gold,
    Resource,
}

impl Tiles {
    // ── Config-aware property methods ──────────────────────────────────────

    pub fn base_tile_id_with_config(self, cfg: &TileConfig) -> u32 {
        cfg.base_tile_id(self.as_str()).unwrap_or(0)
    }

    pub fn atlas_index_with_config(self, cfg: &TileConfig) -> u32 {
        cfg.atlas_index(self.as_str()).unwrap_or(0)
    }

    pub fn color_with_config(self, cfg: &TileConfig) -> (u8, u8, u8) {
        cfg.color(self.as_str()).unwrap_or((255, 0, 255))
    }

    pub fn is_passable_with_config(self, cfg: &TileConfig) -> bool {
        cfg.is_passable(self.as_str()).unwrap_or(false)
    }

    pub fn is_buildable_with_config(self, cfg: &TileConfig) -> bool {
        cfg.is_buildable(self.as_str()).unwrap_or(false)
    }

    pub fn is_terraformable_with_config(self, cfg: &TileConfig) -> bool {
        cfg.is_terraformable(self.as_str()).unwrap_or(false)
    }

    /// Returns `true` if a city entrance can be placed adjacent to this tile.
    pub fn allows_city_entrance_with_config(self, cfg: &TileConfig) -> bool {
        self.is_buildable_with_config(cfg) || matches!(self, Tiles::Meadow | Tiles::CityEntrance)
    }

    pub fn movement_cost_modifier_with_config(self, cfg: &TileConfig) -> i32 {
        cfg.movement_cost(self.as_str()).unwrap_or(0)
    }

    pub fn is_point_of_interest_with_config(self, cfg: &TileConfig) -> bool {
        cfg.is_poi(self.as_str()).unwrap_or(false)
    }

    pub fn as_char_with_config(self, cfg: &TileConfig) -> char {
        cfg.ascii_char(self.as_str()).unwrap_or('?')
    }

    // ── Identity helpers (no config needed) ────────────────────────────────

    /// Returns the Lua-facing string identifier for this tile.
    pub fn as_str(self) -> &'static str {
        match self {
            Tiles::Meadow => "meadow",
            Tiles::Forest => "forest",
            Tiles::Mountain => "mountain",
            Tiles::Water => "water",
            Tiles::City => "city",
            Tiles::CityEntrance => "city_entrance",
            Tiles::Road => "road",
            Tiles::River => "river",
            Tiles::Bridge => "bridge",
            Tiles::Village => "village",
            Tiles::Merchant => "merchant",
            Tiles::Ruins => "ruins",
            Tiles::Gold => "gold",
            Tiles::Resource => "resource",
        }
    }

    /// Returns the TMX GID for this tile (1-based; GID 0 is reserved by Tiled for "empty").
    ///
    /// Assumes a single tileset whose first GID is 1.
    pub fn to_gid_with_config(self, cfg: &TileConfig) -> u32 {
        self.base_tile_id_with_config(cfg) + 1
    }

    /// Constructs a [`Tiles`] from a zero-based tile ID using the given config.
    ///
    /// # Errors
    /// Returns [`EngineError::InvalidTileKind`] if the ID is out of range.
    pub fn from_id_with_config(id: u32, cfg: &TileConfig) -> Result<Self, EngineError> {
        let name = cfg
            .find_by_base_id(id)
            .ok_or_else(|| EngineError::InvalidTileKind(format!("unknown tile ID {id}")))?;
        Tiles::from_str(name)
    }

    /// Constructs a [`Tiles`] from a TMX GID (1-based) using the given config.
    ///
    /// # Errors
    /// Returns [`EngineError::InvalidTileKind`] if the GID does not map to a known tile.
    pub fn from_gid_with_config(gid: u32, cfg: &TileConfig) -> Result<Self, EngineError> {
        if gid == 0 {
            return Err(EngineError::InvalidTileKind("GID 0 is reserved (empty)".into()));
        }
        Self::from_id_with_config(gid - 1, cfg)
    }

    /// Returns all tile variants in definition order.
    pub fn all() -> &'static [Tiles] {
        &[
            Tiles::Meadow,
            Tiles::Forest,
            Tiles::Mountain,
            Tiles::Water,
            Tiles::City,
            Tiles::CityEntrance,
            Tiles::Road,
            Tiles::River,
            Tiles::Bridge,
            Tiles::Village,
            Tiles::Merchant,
            Tiles::Ruins,
            Tiles::Gold,
            Tiles::Resource,
        ]
    }
}

// ─── Tile ─────────────────────────────────────────────────────────────────────

/// A single isometric map tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tile {
    /// The terrain / object type of this tile.
    pub kind: Tiles,
}

impl Tile {
    /// Creates a new tile with the given terrain kind.
    pub fn new(kind: Tiles) -> Self {
        Self { kind }
    }

    pub fn is_city(&self) -> bool {
        matches!(self.kind, Tiles::City | Tiles::CityEntrance)
    }

    pub fn is_resource(&self) -> bool {
        matches!(self.kind, Tiles::Gold | Tiles::Resource)
    }
}

impl Default for Tile {
    /// Returns a default meadow tile.
    fn default() -> Self {
        Self { kind: Tiles::Meadow }
    }
}

impl FromStr for Tiles {
    type Err = EngineError;

    /// Constructs a [`Tiles`] from the Lua-facing string identifier.
    ///
    /// # Errors
    /// Returns [`EngineError::InvalidTileKind`] if the string is not recognised.
    fn from_str(s: &str) -> Result<Self, EngineError> {
        match s {
            "meadow" => Ok(Tiles::Meadow),
            "forest" => Ok(Tiles::Forest),
            "mountain" => Ok(Tiles::Mountain),
            "water" => Ok(Tiles::Water),
            "city" => Ok(Tiles::City),
            "city_entrance" => Ok(Tiles::CityEntrance),
            "road" => Ok(Tiles::Road),
            "river" => Ok(Tiles::River),
            "bridge" => Ok(Tiles::Bridge),
            "village" => Ok(Tiles::Village),
            "merchant" => Ok(Tiles::Merchant),
            "ruins" => Ok(Tiles::Ruins),
            "gold" => Ok(Tiles::Gold),
            "resource" => Ok(Tiles::Resource),
            other => Err(EngineError::InvalidTileKind(other.to_string())),
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use super::*;
    use crate::config::tile_config::test_tile_config;

    #[test]
    fn str_round_trip() {
        for &tile in Tiles::all() {
            let s = tile.as_str();
            let restored = Tiles::from_str(s).unwrap();
            assert_eq!(tile, restored, "round-trip failed for {s}");
        }
    }

    #[test]
    fn gid_round_trip() {
        let cfg = test_tile_config();
        for &tile in Tiles::all() {
            let gid = tile.to_gid_with_config(&cfg);
            let restored = Tiles::from_gid_with_config(gid, &cfg).unwrap();
            assert_eq!(tile, restored, "GID round-trip failed for {tile:?}");
        }
    }

    #[test]
    fn gid_zero_is_error() {
        let cfg = test_tile_config();
        assert!(Tiles::from_gid_with_config(0, &cfg).is_err());
    }

    #[test]
    fn invalid_str_returns_error() {
        assert!(Tiles::from_str("lava").is_err());
    }

    #[test]
    fn passability_rules() {
        let cfg = test_tile_config();
        assert!(Tiles::Meadow.is_passable_with_config(&cfg));
        assert!(Tiles::Road.is_passable_with_config(&cfg));
        assert!(Tiles::Bridge.is_passable_with_config(&cfg));
        assert!(Tiles::Water.is_passable_with_config(&cfg)); // passable with penalty
        assert!(Tiles::River.is_passable_with_config(&cfg)); // passable with penalty
        assert!(!Tiles::City.is_passable_with_config(&cfg)); // impassable
        assert!(!Tiles::Mountain.is_passable_with_config(&cfg)); // impassable
    }

    #[test]
    fn tile_count_matches_config() {
        let cfg = test_tile_config();
        assert_eq!(Tiles::all().len(), cfg.tiles.len());
    }

    #[test]
    fn all_tiles_have_unique_base_ids() {
        let cfg = test_tile_config();
        let mut ids: Vec<u32> =
            Tiles::all().iter().map(|t| t.base_tile_id_with_config(&cfg)).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), Tiles::all().len());
    }

    #[test]
    fn all_tiles_defined_in_config() {
        let cfg = test_tile_config();
        for &tile in Tiles::all() {
            assert!(cfg.tiles.contains_key(tile.as_str()), "tile {:?} missing from config", tile);
        }
    }
}
