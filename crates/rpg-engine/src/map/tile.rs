//! Tile primitives: [`Tiles`] enum and [`Tile`] struct.
//!
//! [`Tiles`] is the canonical tile type for the whole project.
//! It maps 1-to-1 to the `world_tileset` (see `tileset/tileset.tsx`).

use serde::{Deserialize, Serialize};

use crate::error::Error;

// ─── Tiles ────────────────────────────────────────────────────────────────────

/// All terrain and object types available in the world tileset.
///
/// The numeric IDs (`tile_id()`) correspond directly to the tile indices in
/// `tileset/tileset.tsx` and must never be reordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tiles {
    /// Open grassland. Passable, normal movement cost, buildable.
    Meadow,
    /// Dense forest. Passable, +1 movement cost.
    Forest,
    /// Mountain terrain. Impassable.
    Mountain,
    /// Deep water. Impassable (unless crossing a bridge).
    Water,
    /// City tile. Passable; special game object.
    City,
    /// City entrance tile. Passable; marks city entry point.
    CityEntrance,
    /// Paved road. Passable, −1 movement cost (minimum 1).
    Road,
    /// River. Impassable without a bridge.
    River,
    /// Bridge over a river. Passable.
    Bridge,
    /// Village. Passable; minor settlement.
    Village,
    /// Merchant camp. Passable; trade point of interest.
    Merchant,
    /// Ancient ruins. Passable; adventure point of interest.
    Ruins,
    /// Gold deposit. Passable; resource node.
    Gold,
    /// Generic resource node. Passable.
    Resource,
}

impl Tiles {
    /// Returns the zero-based tile index in the `world_tileset`.
    ///
    /// Matches the `id` attribute in `tileset/tileset.tsx`.
    pub fn tile_id(self) -> u32 {
        match self {
            Tiles::Meadow => 0,
            Tiles::Forest => 1,
            Tiles::Mountain => 2,
            Tiles::Water => 3,
            Tiles::City => 4,
            Tiles::CityEntrance => 5,
            Tiles::Road => 6,
            Tiles::River => 7,
            Tiles::Bridge => 8,
            Tiles::Village => 9,
            Tiles::Merchant => 10,
            Tiles::Ruins => 11,
            Tiles::Gold => 12,
            Tiles::Resource => 13,
        }
    }

    /// Returns the representative RGB colour for minimap and debug rendering.
    pub fn as_color(self) -> (u8, u8, u8) {
        match self {
            Tiles::Meadow => (124, 179, 66),
            Tiles::Forest => (46, 125, 50),
            Tiles::Mountain => (141, 141, 141),
            Tiles::Water => (13, 71, 161),
            Tiles::City => (255, 112, 67),
            Tiles::CityEntrance => (251, 192, 45),
            Tiles::Road => (215, 184, 153),
            Tiles::River => (30, 136, 229),
            Tiles::Bridge => (159, 183, 198),
            Tiles::Village => (255, 45, 85),
            Tiles::Merchant => (203, 48, 224),
            Tiles::Ruins => (191, 106, 2),
            Tiles::Gold => (0, 200, 179),
            Tiles::Resource => (0, 195, 208),
        }
    }

    /// Returns `true` if a unit can enter this tile without special equipment.
    pub fn is_passable(self) -> bool {
        !matches!(self, Tiles::Mountain | Tiles::Water | Tiles::River)
    }

    /// Returns `true` if a building can be constructed on this tile.
    pub fn is_buildable(self) -> bool {
        matches!(self, Tiles::Meadow)
    }

    /// Returns `true` if a city entrance can be placed adjacent to this tile.
    pub fn allows_city_entrance(self) -> bool {
        matches!(self, Tiles::Meadow)
    }

    /// Returns the extra movement point cost to enter this tile.
    ///
    /// Positive = slower; negative = faster.
    /// Effective cost is always `max(1, 1 + modifier)`.
    /// Impassable tiles should be checked via [`is_passable`](Self::is_passable) first.
    pub fn movement_cost_modifier(self) -> i32 {
        match self {
            Tiles::Road => -1,
            Tiles::Forest => 1,
            _ => 0,
        }
    }

    /// Returns `true` if this tile is a point of interest that may trigger events.
    pub fn is_point_of_interest(self) -> bool {
        matches!(
            self,
            Tiles::Ruins | Tiles::Gold | Tiles::Resource | Tiles::Merchant | Tiles::Village
        )
    }

    /// Returns the single-character symbol used in ASCII / terminal display.
    pub fn as_char(self) -> char {
        match self {
            Tiles::Meadow => '.',
            Tiles::Forest => '♣',
            Tiles::Mountain => '▲',
            Tiles::Water => '~',
            Tiles::City => '⌂',
            Tiles::CityEntrance => '⌂',
            Tiles::Road => '#',
            Tiles::River => '≈',
            Tiles::Bridge => '=',
            Tiles::Village => '⌘',
            Tiles::Merchant => '$',
            Tiles::Ruins => '⍟',
            Tiles::Gold => '*',
            Tiles::Resource => '◆',
        }
    }

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

    /// Constructs a [`Tiles`] from the Lua-facing string identifier.
    ///
    /// # Errors
    /// Returns [`Error::InvalidTileKind`] if the string is not recognised.
    pub fn from_str(s: &str) -> Result<Self, Error> {
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
            other => Err(Error::InvalidTileKind(other.to_string())),
        }
    }

    /// Returns the TMX GID for this tile (1-based; GID 0 is reserved by Tiled for "empty").
    ///
    /// Assumes a single tileset whose first GID is 1.
    pub fn to_gid(self) -> u32 {
        self.tile_id() + 1
    }

    /// Constructs a [`Tiles`] from a TMX GID (1-based).
    ///
    /// # Errors
    /// Returns [`Error::InvalidTileKind`] if the GID does not map to a known tile.
    pub fn from_gid(gid: u32) -> Result<Self, Error> {
        if gid == 0 {
            return Err(Error::InvalidTileKind("GID 0 is reserved (empty)".into()));
        }
        Self::from_id(gid - 1)
    }

    /// Constructs a [`Tiles`] from a zero-based tile ID.
    ///
    /// # Errors
    /// Returns [`Error::InvalidTileKind`] if the ID is out of range.
    pub fn from_id(id: u32) -> Result<Self, Error> {
        match id {
            0 => Ok(Tiles::Meadow),
            1 => Ok(Tiles::Forest),
            2 => Ok(Tiles::Mountain),
            3 => Ok(Tiles::Water),
            4 => Ok(Tiles::City),
            5 => Ok(Tiles::CityEntrance),
            6 => Ok(Tiles::Road),
            7 => Ok(Tiles::River),
            8 => Ok(Tiles::Bridge),
            9 => Ok(Tiles::Village),
            10 => Ok(Tiles::Merchant),
            11 => Ok(Tiles::Ruins),
            12 => Ok(Tiles::Gold),
            13 => Ok(Tiles::Resource),
            other => Err(Error::InvalidTileKind(format!("unknown tile ID {other}"))),
        }
    }

    /// Returns all tile variants in tile-ID order.
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
}

impl Default for Tile {
    /// Returns a default meadow tile.
    fn default() -> Self {
        Self {
            kind: Tiles::Meadow,
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tiles_have_unique_ids() {
        let mut ids: Vec<u32> = Tiles::all().iter().map(|t| t.tile_id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), Tiles::all().len());
    }

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
        for &tile in Tiles::all() {
            let gid = tile.to_gid();
            let restored = Tiles::from_gid(gid).unwrap();
            assert_eq!(tile, restored, "GID round-trip failed for {tile:?}");
        }
    }

    #[test]
    fn gid_zero_is_error() {
        assert!(Tiles::from_gid(0).is_err());
    }

    #[test]
    fn invalid_str_returns_error() {
        assert!(Tiles::from_str("lava").is_err());
    }

    #[test]
    fn passability_rules() {
        assert!(Tiles::Meadow.is_passable());
        assert!(Tiles::Road.is_passable());
        assert!(Tiles::Bridge.is_passable());
        assert!(!Tiles::Water.is_passable());
        assert!(!Tiles::Mountain.is_passable());
        assert!(!Tiles::River.is_passable());
    }

    #[test]
    fn tile_count_matches_tileset() {
        // tileset.tsx has 14 tiles (id 0..13)
        assert_eq!(Tiles::all().len(), 14);
    }
}
