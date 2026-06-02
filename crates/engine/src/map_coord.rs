use serde::{Deserialize, Serialize};

/// Absolute tile coordinates within the full game map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MapCoord {
    /// Horizontal tile index from the left edge of the map.
    pub x: u32,
    /// Vertical tile index from the top edge of the map.
    pub y: u32,
}

impl MapCoord {
    /// Creates a new map coordinate.
    pub fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }
}

impl Default for MapCoord {
    fn default() -> Self {
        Self { x: 0, y: 0 }
    }
}
