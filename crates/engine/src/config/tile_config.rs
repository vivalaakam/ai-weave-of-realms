//! Tile configuration module — static tile metadata loaded from YAML.
//!
//! [`TileConfig`] holds per-tile properties (colours, ASCII, passability,
//! movement cost, sprite variants) and is the single source of truth for all
//! static tile properties.
//!
//! # Usage
//!
//! The binary should call [`init_tile_config`](crate::config::init_tile_config)
//! once at start-up (e.g. in `main`), passing the contents of `assets/tiles.yaml`.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

// ─── TileConfig ───────────────────────────────────────────────────────────────

/// Top-level YAML structure — maps logical tile name → [`TileEntry`].
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TileConfig {
    pub tiles: BTreeMap<String, TileEntry>,
}

/// Per-tile static properties and sprite atlas indexes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TileEntry {
    /// Stable canonical tile ID for save/load and reverse mapping.
    pub tile_id: u32,
    /// One or more atlas index entries (each carries its own variant label).
    pub atlas_indexes: Vec<AtlasIndex>,
    /// All available variant names for this tile (superset of `atlas_indexes` variants).
    #[serde(default)]
    pub variants: Vec<String>,
    /// Hex colour string, e.g. `"#7cb342"`.
    pub color: String,
    /// Single-character ASCII representation.
    pub ascii: String,
    /// Whether units can enter this tile.
    pub passable: bool,
    /// Whether buildings can be constructed on this tile.
    pub buildable: bool,
    /// Extra movement point modifier (can be negative for fast terrain).
    pub movement_cost: i32,
    /// Whether the tile counts as a point of interest.
    #[serde(default)]
    pub poi: bool,
    /// Whether territory can expand onto this tile.
    ///
    /// Impassable terrain (mountains) and water features (water, river) are
    /// not terraformable even though some of them are passable by units.
    #[serde(default = "default_true")]
    pub terraformable: bool,
}

/// A single atlas index entry with its own variant label.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AtlasIndex {
    /// Real index inside the tileset atlas (zero-based).
    pub index: u32,
    /// Variant label for this specific atlas sprite (optional).
    #[serde(default)]
    pub variant: Option<String>,
}

// ─── TileConfig helpers ──────────────────────────────────────────────────────

impl TileConfig {
    /// Look up a tile entry by its logical name.
    pub fn get_entry(&self, name: &str) -> Option<&TileEntry> {
        self.tiles.get(name)
    }

    /// Return the stable `tile_id` of the named tile.
    pub fn base_tile_id(&self, name: &str) -> Option<u32> {
        self.tiles.get(name).map(|e| e.tile_id)
    }

    /// Return the **first** atlas index (canonical render index).
    pub fn atlas_index(&self, name: &str) -> Option<u32> {
        self.tiles.get(name)?.atlas_indexes.first().map(|a| a.index)
    }

    /// Return all atlas indexes for the named tile.
    pub fn atlas_indexes(&self, name: &str) -> Option<Vec<u32>> {
        let entry = self.tiles.get(name)?;
        Some(entry.atlas_indexes.iter().map(|a| a.index).collect())
    }

    /// Return all atlas index entries (with variants) for the named tile.
    pub fn atlas_index_entries(&self, name: &str) -> Option<&[AtlasIndex]> {
        Some(&self.tiles.get(name)?.atlas_indexes)
    }

    /// Return all variant names declared for the named tile.
    pub fn variants(&self, name: &str) -> Option<&[String]> {
        Some(&self.tiles.get(name)?.variants)
    }

    /// RGB triple from the tile's hex colour string.
    pub fn color(&self, name: &str) -> Option<(u8, u8, u8)> {
        parse_hex_color(&self.tiles.get(name)?.color)
    }

    /// First character of the tile's ASCII representation.
    pub fn ascii_char(&self, name: &str) -> Option<char> {
        self.tiles.get(name)?.ascii.chars().next()
    }

    /// Is the tile passable?
    pub fn is_passable(&self, name: &str) -> Option<bool> {
        Some(self.tiles.get(name)?.passable)
    }

    /// Is the tile buildable?
    pub fn is_buildable(&self, name: &str) -> Option<bool> {
        Some(self.tiles.get(name)?.buildable)
    }

    /// Extra movement cost modifier.
    pub fn movement_cost(&self, name: &str) -> Option<i32> {
        Some(self.tiles.get(name)?.movement_cost)
    }

    /// Is the tile a point of interest?
    pub fn is_poi(&self, name: &str) -> Option<bool> {
        Some(self.tiles.get(name)?.poi)
    }

    /// Can territory expand onto this tile?
    pub fn is_terraformable(&self, name: &str) -> Option<bool> {
        Some(self.tiles.get(name)?.terraformable)
    }

    /// Find the tile whose `tile_id` matches.
    /// Used for reverse mapping (e.g. during deserialization).
    pub fn find_by_base_id(&self, tile_id: u32) -> Option<&str> {
        for (name, entry) in &self.tiles {
            if entry.tile_id == tile_id {
                return Some(name);
            }
        }
        None
    }

    /// Return an iterator over all defined tile names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.tiles.keys().map(|s| s.as_str())
    }
}

impl TileEntry {
    /// The stable canonical tile_id.
    pub fn base_tile_id(&self) -> u32 {
        self.tile_id
    }
}

/// Default value for `terraformable: bool` — used by `#[serde(default)]`.
const fn default_true() -> bool {
    true
}

/// Parse a `"#RRGGBB"` or `"RRGGBB"` string into a `(r, g, b)` triple.
pub(crate) fn parse_hex_color(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

// ─── Test helper (shared with other crates) ─────────────────────────────

/// Construct a minimal TileConfig for unit tests that need tile data.
///
#[doc(hidden)]
pub fn test_tile_config() -> TileConfig {
    let mut tiles = BTreeMap::new();

    tiles.insert(
        String::from("meadow"),
        TileEntry {
            tile_id: 0,
            atlas_indexes: vec![AtlasIndex { index: 5, variant: Some(String::from("summer")) }],
            variants: vec![String::from("summer"), String::from("summer_bright"), String::from("summer_dark")],
            color: String::from("#7cb342"),
            ascii: String::from("."),
            passable: true,
            buildable: true,
            movement_cost: 0,
            poi: false,
            terraformable: true,
        },
    );

    tiles.insert(
        String::from("forest"),
        TileEntry {
            tile_id: 1,
            atlas_indexes: vec![AtlasIndex { index: 1, variant: Some(String::from("evergreen")) }],
            variants: vec![String::from("evergreen"), String::from("evergreen_sparse")],
            color: String::from("#2e7d32"),
            ascii: String::from("♣"),
            passable: true,
            buildable: false,
            movement_cost: 1,
            poi: false,
            terraformable: true,
        },
    );

    tiles.insert(
        String::from("mountain"),
        TileEntry {
            tile_id: 2,
            atlas_indexes: vec![AtlasIndex { index: 2, variant: Some(String::from("rocky")) }],
            variants: vec![String::from("rocky")],
            color: String::from("#8d8d8d"),
            ascii: String::from("▲"),
            passable: false,
            buildable: false,
            movement_cost: 3,
            poi: false,
            terraformable: false,
        },
    );

    tiles.insert(
        String::from("water"),
        TileEntry {
            tile_id: 3,
            atlas_indexes: vec![AtlasIndex { index: 3, variant: Some(String::from("calm")) }],
            variants: vec![String::from("calm")],
            color: String::from("#0d47a1"),
            ascii: String::from("~"),
            passable: true,
            buildable: false,
            movement_cost: 3,
            poi: false,
            terraformable: false,
        },
    );

    tiles.insert(
        String::from("city"),
        TileEntry {
            tile_id: 4,
            atlas_indexes: vec![AtlasIndex { index: 416, variant: Some(String::from("castle")) }],
            variants: vec![String::from("castle"), String::from("fortress"), String::from("citadel")],
            color: String::from("#ff7043"),
            ascii: String::from("⌂"),
            passable: false,
            buildable: false,
            movement_cost: 0,
            poi: false,
            terraformable: false,
        },
    );

    tiles.insert(
        String::from("city_entrance"),
        TileEntry {
            tile_id: 5,
            atlas_indexes: vec![AtlasIndex { index: 420, variant: Some(String::from("gate")) }],
            variants: vec![String::from("gate"), String::from("portcullis")],
            color: String::from("#FBC02D"),
            ascii: String::from("⌂"),
            passable: true,
            buildable: false,
            movement_cost: 0,
            poi: false,
            terraformable: true,
        },
    );

    tiles.insert(
        String::from("road"),
        TileEntry {
            tile_id: 6,
            atlas_indexes: vec![AtlasIndex { index: 6, variant: Some(String::from("dirt")) }],
            variants: vec![String::from("dirt"), String::from("cobble"), String::from("paved")],
            color: String::from("#d7b899"),
            ascii: String::from("#"),
            passable: true,
            buildable: false,
            movement_cost: -1,
            poi: false,
            terraformable: true,
        },
    );

    tiles.insert(
        String::from("river"),
        TileEntry {
            tile_id: 7,
            atlas_indexes: vec![AtlasIndex { index: 7, variant: Some(String::from("shallow")) }],
            variants: vec![String::from("shallow"), String::from("shallow_rocky"), String::from("deep")],
            color: String::from("#1e88e5"),
            ascii: String::from("≈"),
            passable: true,
            buildable: false,
            movement_cost: 3,
            poi: false,
            terraformable: false,
        },
    );

    tiles.insert(
        String::from("bridge"),
        TileEntry {
            tile_id: 8,
            atlas_indexes: vec![AtlasIndex { index: 8, variant: Some(String::from("wood")) }],
            variants: vec![String::from("wood"), String::from("stone"), String::from("rope")],
            color: String::from("#9fb7c6"),
            ascii: String::from("="),
            passable: true,
            buildable: false,
            movement_cost: 0,
            poi: false,
            terraformable: true,
        },
    );

    tiles.insert(
        String::from("village"),
        TileEntry {
            tile_id: 9,
            atlas_indexes: vec![AtlasIndex { index: 9, variant: Some(String::from("hamlet")) }],
            variants: vec![String::from("hamlet"), String::from("town"), String::from("trading_post")],
            color: String::from("#FF2D55"),
            ascii: String::from("⌘"),
            passable: true,
            buildable: false,
            movement_cost: 0,
            poi: true,
            terraformable: true,
        },
    );

    tiles.insert(
        String::from("merchant"),
        TileEntry {
            tile_id: 10,
            atlas_indexes: vec![AtlasIndex { index: 10, variant: Some(String::from("tent")) }],
            variants: vec![String::from("tent"), String::from("stall"), String::from("caravan")],
            color: String::from("#cb30e0"),
            ascii: String::from("$"),
            passable: true,
            buildable: false,
            movement_cost: 0,
            poi: true,
            terraformable: true,
        },
    );

    tiles.insert(
        String::from("ruins"),
        TileEntry {
            tile_id: 11,
            atlas_indexes: vec![AtlasIndex { index: 11, variant: Some(String::from("ancient")) }],
            variants: vec![String::from("ancient"), String::from("crumbling"), String::from("buried")],
            color: String::from("#BF6A02"),
            ascii: String::from("⍟"),
            passable: true,
            buildable: false,
            movement_cost: 0,
            poi: true,
            terraformable: true,
        },
    );

    tiles.insert(
        String::from("gold"),
        TileEntry {
            tile_id: 12,
            atlas_indexes: vec![AtlasIndex { index: 1091, variant: Some(String::from("mine")) }],
            variants: vec![String::from("mine")],
            color: String::from("#f2c94c"),
            ascii: String::from("*"),
            passable: true,
            buildable: false,
            movement_cost: 0,
            poi: true,
            terraformable: true,
        },
    );

    tiles.insert(
        String::from("resource"),
        TileEntry {
            tile_id: 13,
            atlas_indexes: vec![AtlasIndex { index: 1089, variant: Some(String::from("resource_1")) }],
            variants: vec![String::from("resource_1"), String::from("resource_2"), String::from("resource_3"), String::from("resource_4")],
            color: String::from("#ffffff"),
            ascii: String::from("◆"),
            passable: true,
            buildable: false,
            movement_cost: 0,
            poi: true,
            terraformable: true,
        },
    );

    TileConfig { tiles }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_has_all_tiles() {
        let cfg = test_tile_config();
        let expected = [
            "meadow",
            "forest",
            "mountain",
            "water",
            "city",
            "city_entrance",
            "road",
            "river",
            "bridge",
            "village",
            "merchant",
            "ruins",
            "gold",
            "resource",
        ];
        for name in expected {
            assert!(cfg.tiles.contains_key(name), "missing tile: {name}");
        }
        assert_eq!(cfg.tiles.len(), expected.len());
    }

    #[test]
    fn color_parsing_round_trip() {
        let cfg = test_tile_config();
        for entry in cfg.tiles.values() {
            let parsed = parse_hex_color(&entry.color).unwrap();
            let rebuilt = format!("{:02X}{:02X}{:02X}", parsed.0, parsed.1, parsed.2);
            assert_eq!(rebuilt, entry.color.trim_start_matches('#').to_uppercase());
        }
    }

    #[test]
    fn find_by_base_id_works() {
        let cfg = test_tile_config();
        assert_eq!(cfg.find_by_base_id(0), Some("meadow"));
        assert_eq!(cfg.find_by_base_id(5), Some("city_entrance"));
        assert_eq!(cfg.find_by_base_id(999), None);
    }

    #[test]
    fn atlas_index_entry_has_variant() {
        let cfg = test_tile_config();
        let meadow = cfg.tiles.get("meadow").unwrap();
        assert_eq!(meadow.atlas_indexes.len(), 1);
        assert_eq!(meadow.atlas_indexes[0].variant.as_deref(), Some("summer"));
    }

    #[test]
    fn top_level_variants_present() {
        let cfg = test_tile_config();
        let city = cfg.tiles.get("city").unwrap();
        assert_eq!(city.variants, vec!["castle", "fortress", "citadel"]);

        let road = cfg.tiles.get("road").unwrap();
        assert_eq!(road.variants, vec!["dirt", "cobble", "paved"]);
    }
}