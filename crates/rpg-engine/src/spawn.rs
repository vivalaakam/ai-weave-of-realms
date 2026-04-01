//! Spawn point selection for generated maps.
//!
//! This module derives deterministic starting positions for the first
//! player-controlled hero and the first enemy unit from the generated
//! [`GameMap`](crate::map::game_map::GameMap).

use crate::error::Error;
use crate::map::game_map::{GameMap, MapCoord};
use crate::map::tile::Tiles;

/// Recommended starting positions for the initial player and enemy heroes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnPositions {
    /// Preferred player start tile.
    pub player: MapCoord,
    /// Preferred enemy start tile.
    pub enemy: MapCoord,
}

/// Selects deterministic starting positions for the initial heroes.
///
/// Player start prioritises `CityEntrance`, then `Road`, then `Meadow`,
/// then any remaining passable tile. Enemy start prefers a distant
/// non-POI passable tile.
///
/// # Returns
/// Recommended coordinates for the player and enemy heroes.
///
/// # Errors
/// Returns [`Error::OutOfBounds`] if the map has no valid spawnable tiles.
pub fn find_spawn_positions(map: &GameMap) -> Result<SpawnPositions, Error> {
    let player = find_player_spawn(map)?;
    let enemy = find_enemy_spawn(map, player)?;
    Ok(SpawnPositions { player, enemy })
}

/// Selects a deterministic player start tile.
///
/// # Returns
/// The best available player start coordinate.
///
/// # Errors
/// Returns [`Error::OutOfBounds`] if the map has no passable tiles.
pub fn find_player_spawn(map: &GameMap) -> Result<MapCoord, Error> {
    find_best_tile(map, player_priority)
}

/// Selects a deterministic enemy start tile far from `player`.
///
/// # Arguments
/// * `player` - The already chosen player spawn coordinate.
///
/// # Returns
/// The best available enemy start coordinate.
///
/// # Errors
/// Returns [`Error::OutOfBounds`] if the map has no valid enemy spawn tile.
pub fn find_enemy_spawn(map: &GameMap, player: MapCoord) -> Result<MapCoord, Error> {
    let mut best: Option<(MapCoord, i32, i64)> = None;

    for_each_coord(map, |coord, kind| {
        if !is_enemy_spawnable(kind) || coord == player {
            return;
        }

        let distance = manhattan_distance(coord, player) as i32;
        let tie_break = i64::from(coord.y) * i64::from(map.tile_width()) + i64::from(coord.x);

        match best {
            Some((_, best_distance, best_tie_break))
                if distance < best_distance
                    || (distance == best_distance && tie_break <= best_tie_break) => {}
            _ => best = Some((coord, distance, tie_break)),
        }
    });

    if let Some((coord, _, _)) = best {
        return Ok(coord);
    }

    find_best_tile(map, fallback_passable_priority)
}

/// Selects up to `count` [`Tiles::CityEntrance`] tiles spread across the map.
///
/// Uses greedy farthest-point selection to maximise distance between the
/// chosen spawns.  If fewer than `count` city entrance tiles exist, falls
/// back to plain [`Tiles::City`] tiles, then to any passable tile.
///
/// Returns an empty `Vec` only when the map has no passable tiles at all.
pub fn find_city_entrance_spawns(map: &GameMap, count: usize) -> Vec<MapCoord> {
    if count == 0 {
        return Vec::new();
    }

    // Collect CityEntrance tiles first, then City as fallback.
    let mut candidates: Vec<MapCoord> = Vec::new();
    for_each_coord(map, |coord, kind| {
        if kind == Tiles::CityEntrance {
            candidates.push(coord);
        }
    });
    if candidates.is_empty() {
        for_each_coord(map, |coord, kind| {
            if kind == Tiles::City {
                candidates.push(coord);
            }
        });
    }
    if candidates.is_empty() {
        // Last resort: any passable tile.
        for_each_coord(map, |coord, kind| {
            if kind.is_passable() {
                candidates.push(coord);
            }
        });
    }

    if candidates.len() <= count {
        return candidates;
    }

    // Greedy farthest-point selection: start from the first candidate and
    // iteratively pick the candidate farthest from all already-selected points.
    let mut selected: Vec<MapCoord> = vec![candidates[0]];
    while selected.len() < count {
        let next = candidates
            .iter()
            .filter(|c| !selected.contains(c))
            .max_by_key(|&&c| {
                // Key = minimum Manhattan distance to any already-selected point.
                selected
                    .iter()
                    .map(|&s| manhattan_distance(c, s))
                    .min()
                    .unwrap_or(0)
            });
        match next {
            Some(&coord) => selected.push(coord),
            None => break,
        }
    }
    selected
}

fn find_best_tile(
    map: &GameMap,
    priority: fn(MapCoord, Tiles, u32, u32) -> Option<i32>,
) -> Result<MapCoord, Error> {
    let center_x = map.tile_width() / 2;
    let center_y = map.tile_height() / 2;
    let mut best: Option<(MapCoord, i32, u32)> = None;

    for_each_coord(map, |coord, kind| {
        let Some(rank) = priority(coord, kind, center_x, center_y) else {
            return;
        };
        let center_distance = manhattan_distance(coord, MapCoord::new(center_x, center_y));

        match best {
            Some((_, best_rank, best_distance))
                if rank > best_rank || (rank == best_rank && center_distance >= best_distance) => {}
            _ => best = Some((coord, rank, center_distance)),
        }
    });

    best.map(|(coord, _, _)| coord)
        .ok_or_else(|| Error::OutOfBounds("map does not contain any valid spawn tiles".to_string()))
}

fn player_priority(coord: MapCoord, kind: Tiles, center_x: u32, center_y: u32) -> Option<i32> {
    let _ = (coord, center_x, center_y);
    match kind {
        Tiles::CityEntrance => Some(0),
        Tiles::Road => Some(1),
        Tiles::Meadow => Some(2),
        other if other.is_passable() => Some(3),
        _ => None,
    }
}

fn fallback_passable_priority(
    coord: MapCoord,
    kind: Tiles,
    center_x: u32,
    center_y: u32,
) -> Option<i32> {
    let _ = (coord, center_x, center_y);
    kind.is_passable().then_some(0)
}

fn is_enemy_spawnable(kind: Tiles) -> bool {
    kind.is_passable()
        && !matches!(
            kind,
            Tiles::City | Tiles::CityEntrance | Tiles::Gold | Tiles::Resource
        )
}

fn for_each_coord(map: &GameMap, mut callback: impl FnMut(MapCoord, Tiles)) {
    for y in 0..map.tile_height() {
        for x in 0..map.tile_width() {
            let coord = MapCoord::new(x, y);
            if let Ok(tile) = map.get_tile(coord) {
                callback(coord, tile.kind);
            }
        }
    }
}

fn manhattan_distance(a: MapCoord, b: MapCoord) -> u32 {
    a.x.abs_diff(b.x) + a.y.abs_diff(b.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::tile::Tile;

    fn map_from_rows(rows: &[&[Tiles]]) -> GameMap {
        let height = rows.len() as u32;
        let width = rows.first().map(|row| row.len()).unwrap_or(0) as u32;
        let mut tiles = Vec::with_capacity((width * height) as usize);
        for row in rows {
            for kind in *row {
                tiles.push(Tile::new(*kind));
            }
        }
        GameMap::new(width, height, tiles, [0u8; 32]).unwrap()
    }

    #[test]
    fn player_prefers_city_entrance() {
        let map = map_from_rows(&[
            &[Tiles::Meadow, Tiles::Road, Tiles::Meadow],
            &[Tiles::Meadow, Tiles::CityEntrance, Tiles::Meadow],
            &[Tiles::Meadow, Tiles::Meadow, Tiles::Meadow],
        ]);

        let spawn = find_player_spawn(&map).unwrap();
        assert_eq!(spawn, MapCoord::new(1, 1));
    }

    #[test]
    fn player_falls_back_to_road_then_meadow() {
        let map = map_from_rows(&[
            &[Tiles::Water, Tiles::Road, Tiles::Water],
            &[Tiles::Mountain, Tiles::Meadow, Tiles::Mountain],
            &[Tiles::Water, Tiles::Water, Tiles::Water],
        ]);

        let spawn = find_player_spawn(&map).unwrap();
        assert_eq!(spawn, MapCoord::new(1, 0));
    }

    #[test]
    fn enemy_prefers_far_passable_non_poi_tile() {
        let map = map_from_rows(&[
            &[
                Tiles::CityEntrance,
                Tiles::Road,
                Tiles::Meadow,
                Tiles::Meadow,
            ],
            &[Tiles::Meadow, Tiles::Water, Tiles::Gold, Tiles::Meadow],
            &[Tiles::Meadow, Tiles::Forest, Tiles::Meadow, Tiles::Meadow],
        ]);

        let player = find_player_spawn(&map).unwrap();
        let enemy = find_enemy_spawn(&map, player).unwrap();

        assert_eq!(player, MapCoord::new(0, 0));
        assert_eq!(enemy, MapCoord::new(3, 2));
    }

    #[test]
    fn spawn_selection_errors_on_fully_blocked_map() {
        let map = map_from_rows(&[
            &[Tiles::Water, Tiles::Mountain],
            &[Tiles::Mountain, Tiles::Water],
        ]);

        let result = find_spawn_positions(&map);
        assert!(matches!(result, Err(Error::OutOfBounds(_))));
    }
}
