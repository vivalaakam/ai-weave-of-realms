//! [`GameMap`] — the full assembled game map stored as a flat tile array.

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::map::tile::Tile;

// ─── MapCoord ─────────────────────────────────────────────────────────────────

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

// ─── Direction ────────────────────────────────────────────────────────────────

/// Cardinal direction used for single-step hero movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    /// Move one tile up (decreasing Y).
    North,
    /// Move one tile right (increasing X).
    East,
    /// Move one tile down (increasing Y).
    South,
    /// Move one tile left (decreasing X).
    West,
}

impl Direction {
    /// Computes the target [`MapCoord`] one step in this direction from `coord`.
    ///
    /// # Arguments
    /// * `coord`  - The starting tile coordinate.
    /// * `width`  - Map width in tiles (used for East boundary check).
    /// * `height` - Map height in tiles (used for South boundary check).
    ///
    /// # Returns
    /// `Some(target)` if the step stays within map bounds, `None` otherwise.
    pub fn apply(self, coord: MapCoord, width: u32, height: u32) -> Option<MapCoord> {
        match self {
            Direction::North => coord.y.checked_sub(1).map(|y| MapCoord::new(coord.x, y)),
            Direction::East => {
                let x = coord.x + 1;
                if x < width {
                    Some(MapCoord::new(x, coord.y))
                } else {
                    None
                }
            }
            Direction::South => {
                let y = coord.y + 1;
                if y < height {
                    Some(MapCoord::new(coord.x, y))
                } else {
                    None
                }
            }
            Direction::West => coord.x.checked_sub(1).map(|x| MapCoord::new(x, coord.y)),
        }
    }
}

// ─── GameMap ──────────────────────────────────────────────────────────────────

/// The full game map stored as a flat row-major tile array.
///
/// Tiles are stored in row-major order: `tiles[y * width + x]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameMap {
    /// Map width in tiles.
    width: u32,
    /// Map height in tiles.
    height: u32,
    /// Flat array of all tiles in row-major order.
    tiles: Vec<Tile>,
    /// The 32-byte seed this map was generated from.
    pub seed: [u8; 32],
}

impl GameMap {
    /// Creates a new [`GameMap`] from a flat `Vec<Tile>`.
    ///
    /// # Arguments
    /// * `width`  - Map width in tiles.
    /// * `height` - Map height in tiles.
    /// * `tiles`  - Pre-generated tiles in row-major order.
    /// * `seed`   - The map seed used during generation.
    ///
    /// # Errors
    /// Returns [`Error::InvalidChunksSize`] if `tiles.len() != width * height`.
    pub fn new(width: u32, height: u32, tiles: Vec<Tile>, seed: [u8; 32]) -> Result<Self, Error> {
        let expected = (width * height) as usize;
        if tiles.len() != expected {
            return Err(Error::InvalidTilesSize {
                expected,
                got: tiles.len(),
            });
        }
        Ok(Self {
            width,
            height,
            tiles,
            seed,
        })
    }

    /// Returns the total width of the map in tiles.
    pub fn tile_width(&self) -> u32 {
        self.width
    }

    /// Returns the total height of the map in tiles.
    pub fn tile_height(&self) -> u32 {
        self.height
    }

    /// Returns a flat slice of all tiles in row-major order.
    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    /// Returns a reference to the tile at the given absolute map coordinate.
    ///
    /// # Errors
    /// Returns [`Error::OutOfBounds`] if the coordinate is outside the map.
    pub fn get_tile(&self, coord: MapCoord) -> Result<&Tile, Error> {
        let idx = self.tile_index(coord)?;
        Ok(&self.tiles[idx])
    }

    /// Returns a mutable reference to the tile at the given absolute map coordinate.
    ///
    /// # Errors
    /// Returns [`Error::OutOfBounds`] if the coordinate is outside the map.
    pub fn get_tile_mut(&mut self, coord: MapCoord) -> Result<&mut Tile, Error> {
        let idx = self.tile_index(coord)?;
        Ok(&mut self.tiles[idx])
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// Computes the flat index of a tile in the `tiles` vector.
    ///
    /// # Errors
    /// Returns [`Error::OutOfBounds`] if the coordinate is outside the map.
    fn tile_index(&self, coord: MapCoord) -> Result<usize, Error> {
        if coord.x >= self.width || coord.y >= self.height {
            return Err(Error::OutOfBounds(format!(
                "tile ({}, {}) outside {}×{} map",
                coord.x, coord.y, self.width, self.height
            )));
        }
        Ok((coord.y * self.width + coord.x) as usize)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::tile::{Tile, Tiles};

    fn make_map(width: u32, height: u32) -> GameMap {
        let tiles = vec![Tile::new(Tiles::Meadow); (width * height) as usize];
        GameMap::new(width, height, tiles, [0u8; 32]).unwrap()
    }

    #[test]
    fn tile_dimensions_are_correct() {
        let map = make_map(96, 96);
        assert_eq!(map.tile_width(), 96);
        assert_eq!(map.tile_height(), 96);
    }

    #[test]
    fn get_tile_out_of_bounds_returns_error() {
        let map = make_map(96, 96);
        assert!(map.get_tile(MapCoord::new(96, 0)).is_err());
    }

    #[test]
    fn get_tile_returns_correct_tile() {
        let mut map = make_map(96, 96);
        let coord = MapCoord::new(33, 1);
        map.get_tile_mut(coord).unwrap().kind = Tiles::Water;
        assert_eq!(map.get_tile(coord).unwrap().kind, Tiles::Water);
    }

    #[test]
    fn tile_count_mismatch_returns_error() {
        let tiles = vec![Tile::default()];
        assert!(GameMap::new(3, 3, tiles, [0u8; 32]).is_err());
    }

    #[test]
    fn tiles_slice_length_matches_dimensions() {
        let map = make_map(4, 5);
        assert_eq!(map.tiles().len(), 20);
    }
}
