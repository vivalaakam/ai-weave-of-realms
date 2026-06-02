//! [`GameMap`] — the full assembled game map stored as a flat tile array.

use alloc::{format, vec::Vec};

use serde::{Deserialize, Serialize};

use crate::error::EngineError;
use crate::game_state::GameState;
use crate::hero::Hero;
use crate::hero_class::HeroClass;
use crate::map::tile::{Tile, Tiles};
use crate::spawn;
use crate::team::Team;
// ─── MapCoord ─────────────────────────────────────────────────────────────────

/// Absolute tile coordinates within the full game map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MapCoord {
    /// Horizontal tile index from the left edge of the map.
    pub x: u32,
    /// Vertical tile index from the top edge of the map.
    pub y: u32,
}

/// Resource node subtype used for resource deposits on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceKind {
    /// First common resource.
    Resource1,
    /// Second common resource.
    Resource2,
    /// Third common resource.
    Resource3,
    /// Fourth common resource.
    Resource4,
    /// Gold mine resource.
    GoldMine,
}

impl ResourceKind {
    /// Returns `true` when this node represents a gold mine.
    pub fn is_gold(self) -> bool {
        matches!(self, Self::GoldMine)
    }

    /// Returns a compact stable id for save serialization.
    pub fn to_id(self) -> u8 {
        match self {
            Self::Resource1 => 0,
            Self::Resource2 => 1,
            Self::Resource3 => 2,
            Self::Resource4 => 3,
            Self::GoldMine => 4,
        }
    }

    /// Constructs a resource kind from a compact save id.
    pub fn from_id(id: u8) -> Result<Self, EngineError> {
        match id {
            0 => Ok(Self::Resource1),
            1 => Ok(Self::Resource2),
            2 => Ok(Self::Resource3),
            3 => Ok(Self::Resource4),
            4 => Ok(Self::GoldMine),
            _ => Err(EngineError::InvalidTileKind(format!("unknown resource kind {id}"))),
        }
    }

    /// Treasury slot index (0–3) for the four common resources, or `None` for
    /// gold mines (which pay into the gold balance instead).
    pub fn resource_index(self) -> Option<usize> {
        match self {
            Self::Resource1 => Some(0),
            Self::Resource2 => Some(1),
            Self::Resource3 => Some(2),
            Self::Resource4 => Some(3),
            Self::GoldMine => None,
        }
    }
}

/// Number of distinct common (non-gold) resource types a team can stockpile.
pub const RESOURCE_KIND_COUNT: usize = 4;

/// A resource point placed on the world map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceNode {
    /// Absolute map coordinate of this resource point.
    pub coord: MapCoord,
    /// Resource subtype, including gold mines.
    pub kind: ResourceKind,
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
    /// Absolute tile coordinates where enemies can spawn.
    #[serde(default)]
    enemy_spawns: Vec<MapCoord>,
    /// Absolute tile coordinates where chests can spawn.
    #[serde(default)]
    chest_spawns: Vec<MapCoord>,
    /// Resource points placed on the map.
    #[serde(default)]
    resource_nodes: Vec<ResourceNode>,
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
    /// Returns [`EngineError::InvalidChunksSize`] if `tiles.len() != width * height`.
    pub fn new(
        width: u32,
        height: u32,
        tiles: Vec<Tile>,
        seed: [u8; 32],
    ) -> Result<Self, EngineError> {
        let expected = (width * height) as usize;
        if tiles.len() != expected {
            return Err(EngineError::InvalidTilesSize { expected, got: tiles.len() });
        }
        Ok(Self {
            width,
            height,
            tiles,
            enemy_spawns: Vec::new(),
            chest_spawns: Vec::new(),
            resource_nodes: Vec::new(),
            seed,
        })
    }

    pub fn default_state(&self, seed: &str) -> Result<GameState, EngineError> {
        let spawns = spawn::find_spawn_positions(self)?;
        let map_width = self.tile_width();
        let mut state = GameState::new(self.clone(), seed);
        let player_team_id = state.add_team(Team::new(0, "Red", (220, 50, 50), true));
        let enemy_team_id = state.add_team(Team::new(2, "Enemy", (150, 80, 200), false));

        let offset =
            MapCoord::new(spawns.player.x.saturating_add(1).min(map_width - 1), spawns.player.y);
        state.add_hero(Hero::new(0, HeroClass::Knight, "Red Hero", spawns.player, player_team_id));
        state.add_hero(Hero::new(1, HeroClass::Rogue, "Orange Hero", offset, player_team_id));

        let enemy_offset =
            MapCoord::new(spawns.enemy.x.saturating_add(1).min(map_width - 1), spawns.enemy.y);
        state.add_hero(Hero::new(2, HeroClass::Warrior, "Enemy 1", spawns.enemy, enemy_team_id));
        state.add_hero(Hero::new(3, HeroClass::Paladin, "Big Boss", enemy_offset, enemy_team_id));
        let _ = state.set_city_owner(spawns.player, Some(player_team_id));
        let _ = state.on_turn();
        Ok(state)
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

    /// Returns all configured enemy spawn points.
    pub fn enemy_spawns(&self) -> &[MapCoord] {
        &self.enemy_spawns
    }

    /// Returns all configured chest spawn points.
    pub fn chest_spawns(&self) -> &[MapCoord] {
        &self.chest_spawns
    }

    /// Returns all configured resource points.
    pub fn resource_nodes(&self) -> &[ResourceNode] {
        &self.resource_nodes
    }

    /// Returns the resource point at `coord`, if one exists.
    pub fn resource_node_at(&self, coord: MapCoord) -> Option<ResourceNode> {
        self.resource_nodes
            .binary_search_by_key(&coord, |node| node.coord)
            .ok()
            .map(|idx| self.resource_nodes[idx])
    }

    /// Returns `true` if an enemy spawn exists at `coord`.
    pub fn has_enemy_spawn(&self, coord: MapCoord) -> bool {
        self.enemy_spawns.binary_search(&coord).is_ok()
    }

    /// Returns `true` if a chest spawn exists at `coord`.
    pub fn has_chest_spawn(&self, coord: MapCoord) -> bool {
        self.chest_spawns.binary_search(&coord).is_ok()
    }

    /// Replaces the enemy and chest spawn point lists.
    ///
    /// # Arguments
    /// * `enemy_spawns` - Absolute tile coordinates for enemy spawn points.
    /// * `chest_spawns` - Absolute tile coordinates for chest spawn points.
    ///
    /// # Errors
    /// Returns [`EngineError::OutOfBounds`] if any coordinate is outside the map.
    pub fn set_spawn_points(
        &mut self,
        enemy_spawns: Vec<MapCoord>,
        chest_spawns: Vec<MapCoord>,
    ) -> Result<(), EngineError> {
        Self::validate_spawn_points(self.width, self.height, &enemy_spawns)?;
        Self::validate_spawn_points(self.width, self.height, &chest_spawns)?;

        let mut enemy_spawns = enemy_spawns;
        let mut chest_spawns = chest_spawns;
        Self::sort_spawn_points(&mut enemy_spawns);
        Self::sort_spawn_points(&mut chest_spawns);

        self.enemy_spawns = enemy_spawns;
        self.chest_spawns = chest_spawns;
        Ok(())
    }

    /// Replaces the resource point list and synchronizes matching tile kinds.
    ///
    /// # Errors
    /// Returns [`EngineError::OutOfBounds`] if any resource coordinate is outside the map.
    pub fn set_resource_nodes(
        &mut self,
        resource_nodes: Vec<ResourceNode>,
    ) -> Result<(), EngineError> {
        for node in &resource_nodes {
            if node.coord.x >= self.width || node.coord.y >= self.height {
                return Err(EngineError::OutOfBounds(format!(
                    "resource ({}, {}) outside {}×{} map",
                    node.coord.x, node.coord.y, self.width, self.height
                )));
            }
        }

        let mut resource_nodes = resource_nodes;
        resource_nodes.sort_by_key(|node| node.coord);
        resource_nodes.dedup_by_key(|node| node.coord);

        for node in &resource_nodes {
            let tile = self.get_tile_mut(node.coord)?;
            tile.kind = if node.kind.is_gold() { Tiles::Gold } else { Tiles::Resource };
        }

        self.resource_nodes = resource_nodes;
        Ok(())
    }

    /// Returns a reference to the tile at the given absolute map coordinate.
    ///
    /// # Errors
    /// Returns [`EngineError::OutOfBounds`] if the coordinate is outside the map.
    pub fn get_tile(&self, coord: MapCoord) -> Result<&Tile, EngineError> {
        let idx = self.tile_index(coord)?;
        Ok(&self.tiles[idx])
    }

    /// Returns a mutable reference to the tile at the given absolute map coordinate.
    ///
    /// # Errors
    /// Returns [`EngineError::OutOfBounds`] if the coordinate is outside the map.
    pub fn get_tile_mut(&mut self, coord: MapCoord) -> Result<&mut Tile, EngineError> {
        let idx = self.tile_index(coord)?;
        Ok(&mut self.tiles[idx])
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// Computes the flat index of a tile in the `tiles` vector.
    ///
    /// # Errors
    /// Returns [`EngineError::OutOfBounds`] if the coordinate is outside the map.
    fn tile_index(&self, coord: MapCoord) -> Result<usize, EngineError> {
        if coord.x >= self.width || coord.y >= self.height {
            return Err(EngineError::OutOfBounds(format!(
                "tile ({}, {}) outside {}×{} map",
                coord.x, coord.y, self.width, self.height
            )));
        }
        Ok((coord.y * self.width + coord.x) as usize)
    }

    fn validate_spawn_points(
        width: u32,
        height: u32,
        points: &[MapCoord],
    ) -> Result<(), EngineError> {
        for coord in points {
            if coord.x >= width || coord.y >= height {
                return Err(EngineError::OutOfBounds(format!(
                    "spawn ({}, {}) outside {}×{} map",
                    coord.x, coord.y, width, height
                )));
            }
        }
        Ok(())
    }

    fn sort_spawn_points(points: &mut Vec<MapCoord>) {
        points.sort_unstable();
        points.dedup();
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
