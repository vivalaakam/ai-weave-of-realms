//! Tile primitives: [`TileKind`] and [`Tile`].

use serde::{Deserialize, Serialize};

use crate::error::Error;

// ─── TileKind ─────────────────────────────────────────────────────────────────

/// The terrain type of a single map tile.
///
/// Determines passability, movement cost modifiers, and visual appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum TileKind {
    /// Open grassland. Passable, normal movement cost.
    Grass = 0,
    /// Water body. Impassable.
    Water = 1,
    /// Dense forest. Passable, +1 movement cost.
    Forest = 2,
    /// Mountain terrain. Impassable.
    Mountain = 3,
    /// Paved road. Passable, −1 movement cost (minimum 1).
    Road = 4,
    /// Ancient ruins. Passable, acts as a point of interest.
    Ruins = 5,
}

impl TileKind {
    /// Returns `true` if a unit can enter this tile.
    pub fn is_passable(self) -> bool {
        !matches!(self, TileKind::Water | TileKind::Mountain)
    }

    /// Returns the extra movement point cost to enter this tile.
    ///
    /// A positive value increases cost; a negative value decreases it.
    /// Base cost is always 1, so the effective cost is `1 + movement_cost_modifier()`.
    pub fn movement_cost_modifier(self) -> i32 {
        match self {
            TileKind::Road => -1,
            TileKind::Forest => 1,
            _ => 0,
        }
    }

    /// Returns `true` if this tile is a point of interest (triggers events on entry).
    pub fn is_point_of_interest(self) -> bool {
        matches!(self, TileKind::Ruins)
    }

    /// Returns the numeric GID used in the TMX tileset for this tile kind.
    ///
    /// GID 0 is reserved by Tiled for "empty"; tile GIDs start at 1.
    pub fn to_gid(self) -> u32 {
        self as u32 + 1
    }

    /// Constructs a [`TileKind`] from a TMX GID.
    ///
    /// # Errors
    /// Returns [`Error::InvalidTileKind`] if the GID does not map to any known tile.
    pub fn from_gid(gid: u32) -> Result<Self, Error> {
        match gid {
            1 => Ok(TileKind::Grass),
            2 => Ok(TileKind::Water),
            3 => Ok(TileKind::Forest),
            4 => Ok(TileKind::Mountain),
            5 => Ok(TileKind::Road),
            6 => Ok(TileKind::Ruins),
            other => Err(Error::InvalidTileKind(format!("unknown GID {other}"))),
        }
    }

    /// Returns the string identifier used in Lua scripts.
    pub fn as_str(self) -> &'static str {
        match self {
            TileKind::Grass => "grass",
            TileKind::Water => "water",
            TileKind::Forest => "forest",
            TileKind::Mountain => "mountain",
            TileKind::Road => "road",
            TileKind::Ruins => "ruins",
        }
    }

    /// Constructs a [`TileKind`] from the string identifier used in Lua scripts.
    ///
    /// # Errors
    /// Returns [`Error::InvalidTileKind`] if the string is not recognised.
    pub fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "grass" => Ok(TileKind::Grass),
            "water" => Ok(TileKind::Water),
            "forest" => Ok(TileKind::Forest),
            "mountain" => Ok(TileKind::Mountain),
            "road" => Ok(TileKind::Road),
            "ruins" => Ok(TileKind::Ruins),
            other => Err(Error::InvalidTileKind(other.to_string())),
        }
    }
}

// ─── Tile ─────────────────────────────────────────────────────────────────────

/// A single isometric map tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tile {
    /// The terrain type of this tile.
    pub kind: TileKind,
}

impl Tile {
    /// Creates a new tile with the given terrain kind.
    pub fn new(kind: TileKind) -> Self {
        Self { kind }
    }
}

impl Default for Tile {
    /// Returns a default grass tile.
    fn default() -> Self {
        Self {
            kind: TileKind::Grass,
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passability_rules() {
        assert!(TileKind::Grass.is_passable());
        assert!(TileKind::Forest.is_passable());
        assert!(TileKind::Road.is_passable());
        assert!(TileKind::Ruins.is_passable());
        assert!(!TileKind::Water.is_passable());
        assert!(!TileKind::Mountain.is_passable());
    }

    #[test]
    fn gid_round_trip() {
        for kind in [
            TileKind::Grass,
            TileKind::Water,
            TileKind::Forest,
            TileKind::Mountain,
            TileKind::Road,
            TileKind::Ruins,
        ] {
            let gid = kind.to_gid();
            let restored = TileKind::from_gid(gid).unwrap();
            assert_eq!(kind, restored);
        }
    }

    #[test]
    fn str_round_trip() {
        for kind in [
            TileKind::Grass,
            TileKind::Water,
            TileKind::Forest,
            TileKind::Mountain,
            TileKind::Road,
            TileKind::Ruins,
        ] {
            let s = kind.as_str();
            let restored = TileKind::from_str(s).unwrap();
            assert_eq!(kind, restored);
        }
    }

    #[test]
    fn invalid_gid_returns_error() {
        assert!(TileKind::from_gid(99).is_err());
    }

    #[test]
    fn invalid_str_returns_error() {
        assert!(TileKind::from_str("lava").is_err());
    }
}
