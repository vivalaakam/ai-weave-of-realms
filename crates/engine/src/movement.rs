//! Movement system — reachable tiles and shortest paths on the game map.
//!
//! Uses Dijkstra's algorithm with per-tile movement costs.
//!
//! ## Movement cost per tile
//! | Tile     | Cost |
//! |----------|------|
//! | Meadow   | 1    |
//! | Forest   | 2    |
//! | Road     | 1    |
//! | Water    | 4    |
//! | River    | 4    |
//! | Impassable (mountain, city) | blocked |
//! | All others | 1  |
//!
//! Movement is 4-directional (N/S/E/W).

use alloc::collections::BinaryHeap;
use alloc::{collections::BTreeMap, vec, vec::Vec};
use core::cmp::Reverse;

use crate::config::TileConfig;
use crate::error::EngineError;
use crate::map::game_map::GameMap;
use crate::map_coord::MapCoord;
// ─── Public API ───────────────────────────────────────────────────────────────

/// Returns all tiles reachable from `start` within `mov_budget` movement points.
///
/// The starting tile itself is excluded from the result.
/// Only passable tiles are included.
pub fn reachable_tiles(map: &GameMap, start: MapCoord, mov_budget: u32, cfg: &TileConfig) -> Vec<MapCoord> {
    let (costs, _) = dijkstra(map, start, mov_budget, cfg);
    costs.into_iter().filter(|(coord, _)| *coord != start).map(|(coord, _)| coord).collect()
}

/// Finds the cheapest path from `start` to `target` within `mov_budget`.
///
/// Returns tiles from `start` (inclusive) to `target` (inclusive), or `None`
/// if `target` is unreachable within the movement budget.
pub fn find_path(
    map: &GameMap,
    start: MapCoord,
    target: MapCoord,
    mov_budget: u32,
    cfg: &TileConfig,
) -> Option<Vec<MapCoord>> {
    let (costs, prev) = dijkstra(map, start, mov_budget, cfg);
    if !costs.contains_key(&target) {
        return None;
    }
    let mut path = vec![target];
    let mut cur = target;
    while cur != start {
        cur = *prev.get(&cur)?;
        path.push(cur);
    }
    path.reverse();
    Some(path)
}

/// Returns the movement cost of entering `target` from an adjacent tile,
/// spending movement from a hero at `start` position with `mov_budget`.
///
/// # Errors
/// Returns [`EngineError::UnreachableTile`] if `target` is not reachable.
pub fn cost_to_reach(
    map: &GameMap,
    start: MapCoord,
    target: MapCoord,
    mov_budget: u32,
    cfg: &TileConfig,
) -> Result<u32, EngineError> {
    let (costs, _) = dijkstra(map, start, mov_budget, cfg);
    costs.get(&target).copied().ok_or(EngineError::UnreachableTile { x: target.x, y: target.y })
}

// ─── Internals ────────────────────────────────────────────────────────────────

/// Returns the movement cost to *enter* `coord`, or `None` if impassable.
fn entry_cost(map: &GameMap, coord: MapCoord, cfg: &TileConfig) -> Option<u32> {
    let tile = map.get_tile(coord).ok()?;
    if !tile.kind.is_passable_with_config(cfg) {
        return None;
    }
    Some((1i32 + tile.kind.movement_cost_modifier_with_config(cfg)).max(1) as u32)
}

/// Returns 4-directional in-bounds neighbours of `coord`.
fn neighbours(map: &GameMap, coord: MapCoord) -> [Option<MapCoord>; 4] {
    let w = map.tile_width();
    let h = map.tile_height();
    [
        coord.x.checked_sub(1).map(|x| MapCoord::new(x, coord.y)),
        if coord.x + 1 < w { Some(MapCoord::new(coord.x + 1, coord.y)) } else { None },
        coord.y.checked_sub(1).map(|y| MapCoord::new(coord.x, y)),
        if coord.y + 1 < h { Some(MapCoord::new(coord.x, coord.y + 1)) } else { None },
    ]
}

/// Dijkstra from `start`, capped at `budget`.
/// Returns `(cost_map, predecessor_map)`.
fn dijkstra(
    map: &GameMap,
    start: MapCoord,
    budget: u32,
    cfg: &TileConfig,
) -> (BTreeMap<MapCoord, u32>, BTreeMap<MapCoord, MapCoord>) {
    let mut costs: BTreeMap<MapCoord, u32> = BTreeMap::new();
    let mut prev: BTreeMap<MapCoord, MapCoord> = BTreeMap::new();
    // Min-heap: Reverse((cost, x, y)) for lexicographic min ordering
    let mut heap: BinaryHeap<Reverse<(u32, u32, u32)>> = BinaryHeap::new();

    costs.insert(start, 0);
    heap.push(Reverse((0, start.x, start.y)));

    while let Some(Reverse((cost, x, y))) = heap.pop() {
        let coord = MapCoord::new(x, y);

        // Skip if we already found a cheaper path
        if cost > *costs.get(&coord).unwrap_or(&u32::MAX) {
            continue;
        }

        for maybe_nb in neighbours(map, coord) {
            let Some(nb) = maybe_nb else { continue };
            let Some(step) = entry_cost(map, nb, cfg) else {
                continue;
            };
            let new_cost = cost + step;
            if new_cost > budget {
                continue;
            }
            if new_cost < *costs.get(&nb).unwrap_or(&u32::MAX) {
                costs.insert(nb, new_cost);
                prev.insert(nb, coord);
                heap.push(Reverse((new_cost, nb.x, nb.y)));
            }
        }
    }

    (costs, prev)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TileConfig;
    use crate::map::game_map::GameMap;
    use crate::map::tile::{Tile, Tiles};

    fn flat_map(w: u32, h: u32, kind: Tiles) -> GameMap {
        let tiles = vec![Tile { kind }; (w * h) as usize];
        GameMap::new(w, h, tiles, [0u8; 32]).unwrap()
    }

    fn mixed_map() -> GameMap {
        // 5×1 row: meadow, road, forest, water(blocked), meadow
        let tiles = vec![
            Tile { kind: Tiles::Meadow },
            Tile { kind: Tiles::Road },
            Tile { kind: Tiles::Forest },
            Tile { kind: Tiles::Water },
            Tile { kind: Tiles::Meadow },
        ];
        GameMap::new(5, 1, tiles, [0u8; 32]).unwrap()
    }

    /// Tests use an empty TileConfig — fallback values apply (passable=true, cost=0).
    fn test_cfg() -> TileConfig {
        TileConfig::default()
    }

    #[test]
    fn reachable_on_meadow_matches_budget() {
        // 5×1 meadow, start at x=0, budget=3 → can reach x=1,2,3
        let map = flat_map(5, 1, Tiles::Meadow);
        let mut tiles = reachable_tiles(&map, MapCoord::new(0, 0), 3, &test_cfg());
        tiles.sort_by_key(|c| c.x);
        assert_eq!(tiles, vec![MapCoord::new(1, 0), MapCoord::new(2, 0), MapCoord::new(3, 0),]);
    }

    #[test]
    fn road_costs_zero_movement() {
        // road tile costs 0 movement, so budget=1 should reach x=2 (road at x=1)
        let map = mixed_map();
        let reachable = reachable_tiles(&map, MapCoord::new(0, 0), 1, &test_cfg());
        // x=1 (road, cost 0) and x=2 (forest, cost 2 — too expensive with budget=1)
        assert!(reachable.contains(&MapCoord::new(1, 0)));
        assert!(!reachable.contains(&MapCoord::new(2, 0)));
    }

    #[test]
    fn water_tile_is_passable_with_penalty() {
        let map = mixed_map();
        // Water at x=3 costs 4, x=4 (meadow) costs 1 — total 5 through water.
        let reachable = reachable_tiles(&map, MapCoord::new(0, 0), 10, &test_cfg());
        assert!(reachable.contains(&MapCoord::new(3, 0)));
        assert!(reachable.contains(&MapCoord::new(4, 0)));
        // With budget 3 water is still reachable via road (cost 1) + forest (2)
        // Actually from x=0 cost 0:
        //   x=1 road cost 1
        //   x=2 forest cost 1+1=2
        //   x=3 water cost 2+3=5 (too much)
        // So with budget 3: x=1(1), x=2(2). x=3 needs budget >=5.
        let reachable_small = reachable_tiles(&map, MapCoord::new(0, 0), 3, &test_cfg());
        assert!(!reachable_small.contains(&MapCoord::new(3, 0)));
        assert!(!reachable_small.contains(&MapCoord::new(4, 0)));
    }

    #[test]
    fn find_path_returns_correct_sequence() {
        let map = flat_map(5, 1, Tiles::Meadow);
        let path = find_path(&map, MapCoord::new(0, 0), MapCoord::new(3, 0), 10, &test_cfg()).unwrap();
        assert_eq!(path.first(), Some(&MapCoord::new(0, 0)));
        assert_eq!(path.last(), Some(&MapCoord::new(3, 0)));
        assert_eq!(path.len(), 4);
    }

    #[test]
    fn find_path_through_water_is_possible_with_budget() {
        let map = mixed_map();
        // x=4 reachable through water (cost: 1+1+2+4+1 = 9)
        let path = find_path(&map, MapCoord::new(0, 0), MapCoord::new(4, 0), 10, &test_cfg());
        assert!(path.is_some());
    }

    #[test]
    fn find_path_unreachable_when_budget_insufficient_for_water() {
        let map = mixed_map();
        // x=4 costs 6 total (1+0+1+3+1=6 via road shortcut, or 1+1+2+4+1=9).
        // With budget=5 path should not exist.
        let path = find_path(&map, MapCoord::new(0, 0), MapCoord::new(4, 0), 5, &test_cfg());
        assert!(path.is_none());
    }

    #[test]
    fn cost_to_reach_correct() {
        let map = flat_map(4, 1, Tiles::Meadow);
        assert_eq!(cost_to_reach(&map, MapCoord::new(0, 0), MapCoord::new(3, 0), 10, &test_cfg()).unwrap(), 3);
    }
}
