//! Spawn point selection for generated maps.
//!
//! This module derives deterministic starting positions for the first
//! player-controlled hero and the first enemy unit from the generated
//! [`GameMap`](crate::map::game_map::GameMap).

use crate::config::TileConfig;
use crate::map::game_map::GameMap;
use crate::map::tile::Tiles;
use crate::map_coord::MapCoord;

/// Recommended starting positions for the initial player and enemy heroes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnPositions {
    /// Preferred player start tile.
    pub player: MapCoord,
    /// Preferred enemy start tile.
    pub enemy: MapCoord,
}

#[derive(Debug, thiserror::Error)]
pub enum SpawnError {
    #[error("No valid spawnable tiles found on the map")]
    OutOfBounds,
}

/// Selects deterministic starting positions for the initial heroes.
///
/// Player start prioritises `CityEntrance`, then `Road`, then `Meadow`,
/// then any remaining passable tile. Enemy start prefers a distant
/// non-POI passable tile.
///
/// # Arguments
/// * `map` - The game map to search.
/// * `cfg` - Tile configuration used for passability checks.
///
/// # Returns
/// Recommended coordinates for the player and enemy heroes.
///
/// # Errors
/// Returns [`Error::OutOfBounds`] if the map has no valid spawnable tiles.
pub fn find_spawn_positions(map: &GameMap, cfg: &TileConfig) -> Result<SpawnPositions, SpawnError> {
    let player = find_player_spawn(map, cfg)?;
    let enemy = find_enemy_spawn(map, player, cfg)?;
    Ok(SpawnPositions { player, enemy })
}

/// Selects a deterministic player start tile.
///
/// # Arguments
/// * `map` - The game map to search.
/// * `cfg` - Tile configuration used for passability checks.
///
/// # Returns
/// The best available player start coordinate.
///
/// # Errors
/// Returns [`Error::OutOfBounds`] if the map has no passable tiles.
pub fn find_player_spawn(map: &GameMap, cfg: &TileConfig) -> Result<MapCoord, SpawnError> {
    find_best_tile(map, cfg, player_priority)
}

/// Selects a deterministic enemy start tile far from `player`.
///
/// # Arguments
/// * `map` - The game map to search.
/// * `player` - The already chosen player spawn coordinate.
/// * `cfg` - Tile configuration used for passability checks.
///
/// # Returns
/// The best available enemy start coordinate.
///
/// # Errors
/// Returns [`Error::OutOfBounds`] if the map has no valid enemy spawn tile.
pub fn find_enemy_spawn(
    map: &GameMap,
    player: MapCoord,
    cfg: &TileConfig,
) -> Result<MapCoord, SpawnError> {
    let mut best: Option<(MapCoord, i32, i64)> = None;

    for_each_coord(map, |coord, kind| {
        if !is_enemy_spawnable(kind, cfg) || coord == player {
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

    find_best_tile(map, cfg, fallback_passable_priority)
}

/// Selects up to `count` [`Tiles::CityEntrance`] tiles spread across the map.
///
/// Uses greedy farthest-point selection to maximise distance between the
/// chosen spawns.  If fewer than `count` city entrance tiles exist, falls
/// back to plain [`Tiles::City`] tiles, then to any passable tile.
///
/// # Arguments
/// * `map` - The game map to search.
/// * `count` - Maximum number of spawn points to return.
/// * `cfg` - Tile configuration used for passability checks.
///
/// Returns an empty `Vec` only when the map has no passable tiles at all.
pub fn find_city_entrance_spawns(map: &GameMap, count: usize, cfg: &TileConfig) -> Vec<MapCoord> {
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
            if kind.is_passable_with_config(cfg) {
                candidates.push(coord);
            }
        });
    }

    if candidates.len() <= count {
        return candidates;
    }

    // Greedy farthest-point selection with cached minimum distances.
    let mut selected: Vec<MapCoord> = vec![candidates[0]];
    let mut selected_flags = vec![false; candidates.len()];
    selected_flags[0] = true;

    let mut min_distances: Vec<u32> =
        candidates.iter().map(|&coord| manhattan_distance(coord, candidates[0])).collect();

    while selected.len() < count {
        let mut best_idx: Option<usize> = None;
        let mut best_distance = 0;

        for (idx, _) in candidates.iter().enumerate() {
            if selected_flags[idx] {
                continue;
            }
            let distance = min_distances[idx];
            let take = match best_idx {
                None => true,
                Some(current) => {
                    distance > best_distance || (distance == best_distance && idx < current)
                }
            };
            if take {
                best_idx = Some(idx);
                best_distance = distance;
            }
        }

        let Some(chosen_idx) = best_idx else {
            break;
        };
        selected_flags[chosen_idx] = true;
        let chosen = candidates[chosen_idx];
        selected.push(chosen);

        for (idx, &coord) in candidates.iter().enumerate() {
            if selected_flags[idx] {
                continue;
            }
            let distance = manhattan_distance(coord, chosen);
            if distance < min_distances[idx] {
                min_distances[idx] = distance;
            }
        }
    }

    selected
}

fn find_best_tile(
    map: &GameMap,
    cfg: &TileConfig,
    priority: fn(MapCoord, Tiles, u32, u32, &TileConfig) -> Option<i32>,
) -> Result<MapCoord, SpawnError> {
    let center_x = map.tile_width() / 2;
    let center_y = map.tile_height() / 2;
    let mut best: Option<(MapCoord, i32, u32)> = None;

    for_each_coord(map, |coord, kind| {
        let Some(rank) = priority(coord, kind, center_x, center_y, cfg) else {
            return;
        };
        let center_distance = manhattan_distance(coord, MapCoord::new(center_x, center_y));

        match best {
            Some((_, best_rank, best_distance))
                if rank > best_rank || (rank == best_rank && center_distance >= best_distance) => {}
            _ => best = Some((coord, rank, center_distance)),
        }
    });

    best.map(|(coord, _, _)| coord).ok_or(SpawnError::OutOfBounds)
}

fn player_priority(
    coord: MapCoord,
    kind: Tiles,
    center_x: u32,
    center_y: u32,
    cfg: &TileConfig,
) -> Option<i32> {
    let _ = (coord, center_x, center_y);
    match kind {
        Tiles::CityEntrance => Some(0),
        Tiles::Road => Some(1),
        Tiles::Meadow => Some(2),
        other if other.is_passable_with_config(cfg) => Some(3),
        _ => None,
    }
}

fn fallback_passable_priority(
    coord: MapCoord,
    kind: Tiles,
    center_x: u32,
    center_y: u32,
    cfg: &TileConfig,
) -> Option<i32> {
    let _ = (coord, center_x, center_y);
    kind.is_passable_with_config(cfg).then_some(0)
}

fn is_enemy_spawnable(kind: Tiles, cfg: &TileConfig) -> bool {
    kind.is_passable_with_config(cfg)
        && !matches!(kind, Tiles::CityEntrance | Tiles::Gold | Tiles::Resource)
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
    use crate::config::tile_config::test_tile_config;
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

        let spawn = find_player_spawn(&map, &test_tile_config()).unwrap();
        assert_eq!(spawn, MapCoord::new(1, 1));
    }

    #[test]
    fn player_falls_back_to_road_then_meadow() {
        let map = map_from_rows(&[
            &[Tiles::Water, Tiles::Road, Tiles::Water],
            &[Tiles::Mountain, Tiles::Meadow, Tiles::Mountain],
            &[Tiles::Water, Tiles::Water, Tiles::Water],
        ]);

        let spawn = find_player_spawn(&map, &test_tile_config()).unwrap();
        assert_eq!(spawn, MapCoord::new(1, 0));
    }

    #[test]
    fn enemy_prefers_far_passable_non_poi_tile() {
        let map = map_from_rows(&[
            &[Tiles::CityEntrance, Tiles::Road, Tiles::Meadow, Tiles::Meadow],
            &[Tiles::Meadow, Tiles::Water, Tiles::Gold, Tiles::Meadow],
            &[Tiles::Meadow, Tiles::Forest, Tiles::Meadow, Tiles::Meadow],
        ]);

        let player = find_player_spawn(&map, &test_tile_config()).unwrap();
        let enemy = find_enemy_spawn(&map, player, &test_tile_config()).unwrap();

        assert_eq!(player, MapCoord::new(0, 0));
        assert_eq!(enemy, MapCoord::new(3, 2));
    }

    #[test]
    fn spawn_selection_errors_on_fully_blocked_map() {
        let map =
            map_from_rows(&[&[Tiles::City, Tiles::Mountain], &[Tiles::Mountain, Tiles::City]]);

        let result = find_spawn_positions(&map, &test_tile_config());
        assert!(matches!(result, Err(SpawnError::OutOfBounds)));
    }
}
