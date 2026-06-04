use crate::map::game_map::{Direction, GameMap};
use crate::map::tile::Tiles;
use crate::map_coord::MapCoord;
use std::collections::{BTreeSet, VecDeque};

pub fn flood_city(map: &GameMap, start: MapCoord) -> Vec<MapCoord> {
    let is_city = map
        .get_tile(start)
        .map(|t| matches!(t.kind, Tiles::City | Tiles::CityEntrance))
        .unwrap_or(false);

    if !is_city {
        return vec![start];
    }

    let w = map.tile_width();
    let h = map.tile_height();
    let mut visited: BTreeSet<MapCoord> = BTreeSet::new();
    let mut queue: VecDeque<MapCoord> = VecDeque::new();
    let mut result: Vec<MapCoord> = Vec::new();

    visited.insert(start);
    queue.push_back(start);

    while let Some(coord) = queue.pop_front() {
        result.push(coord);

        for dir in [Direction::North, Direction::East, Direction::South, Direction::West] {
            if let Some(neighbor) = dir.apply(coord, w, h)
                && !visited.contains(&neighbor)
                && map
                    .get_tile(neighbor)
                    .map(|t| matches!(t.kind, Tiles::City | Tiles::CityEntrance))
                    .unwrap_or(false)
            {
                visited.insert(neighbor);
                queue.push_back(neighbor);
            }
        }
    }

    result
}
