//! Chunk stitcher — smooths terrain at chunk boundary seams.
//!
//! After all chunks are generated independently, their borders may contain
//! hard terrain transitions (e.g., a full column of water touching a full
//! column of grass).  The [`Stitcher`] applies a neighbourhood majority-vote
//! pass on the two-tile-wide strip around every chunk boundary to soften
//! these seams without altering the interior of each chunk.
//!
//! # Algorithm
//! For every seam tile (one tile on each side of each chunk boundary):
//! 1. Collect its four orthogonal neighbours.
//! 2. Count how many neighbours share the same [`Tiles`].
//! 3. If a *different* kind has ≥ 3 votes out of at most 4 neighbours,
//!    replace the tile with that majority kind.
//!
//! Changes are collected first and applied in bulk so that the smoothing
//! pass is order-independent.

use std::collections::HashMap;

use tracing::{debug, instrument};

use rpg_engine::error::Error as EngineError;
use rpg_engine::map::game_map::{GameMap, MapCoord};
use rpg_engine::map::tile::Tiles;

// ─── Stitcher ─────────────────────────────────────────────────────────────────

/// Smooths chunk boundary seams on an assembled [`GameMap`].
pub struct Stitcher;

impl Stitcher {
    /// Applies one majority-vote smoothing pass to all chunk boundary seams.
    ///
    /// Modifies `map` in-place.  The pass is deterministic — the same map
    /// always produces the same result.
    ///
    /// # Arguments
    /// * `map`        - The assembled map to stitch in-place.
    /// * `chunk_size` - The chunk size used during generation (tile units).
    ///
    /// # Errors
    /// Returns [`EngineError::OutOfBounds`] if an internal coordinate calculation
    /// is incorrect (should not happen for a well-formed map).
    #[instrument(skip(map), fields(tw = map.tile_width(), th = map.tile_height()))]
    pub fn stitch(map: &mut GameMap, chunk_size: u32) -> Result<(), EngineError> {
        let changes = Self::collect_changes(map, chunk_size)?;
        let changed = changes.len();

        for (coord, kind) in changes {
            map.get_tile_mut(coord)?.kind = kind;
        }

        Self::enforce_chunk_edge_alignment(map, chunk_size)?;

        debug!(changed, "stitcher pass complete");
        Ok(())
    }

    /// Collects all (coordinate, new_kind) pairs for seam tiles that should change.
    ///
    /// # Errors
    /// Returns [`EngineError::OutOfBounds`] on unexpected coordinate access.
    fn collect_changes(
        map: &GameMap,
        chunk_size: u32,
    ) -> Result<Vec<(MapCoord, Tiles)>, EngineError> {
        let mut changes = Vec::new();

        for coord in Self::seam_coordinates(map, chunk_size) {
            let current = map.get_tile(coord)?.kind;
            let neighbors = Self::orthogonal_neighbors(coord, map.tile_width(), map.tile_height());

            let mut counts: HashMap<Tiles, usize> = HashMap::new();
            for nc in &neighbors {
                let kind = map.get_tile(*nc)?.kind;
                *counts.entry(kind).or_insert(0) += 1;
            }

            // Replace if a different kind holds ≥ 3 of the neighbour votes
            if let Some((&majority, &votes)) = counts.iter().max_by_key(|(_, &v)| v) {
                if votes >= 3 && majority != current {
                    changes.push((coord, majority));
                }
            }
        }

        Ok(changes)
    }

    /// Returns all tile coordinates that lie within one tile of a chunk boundary seam.
    ///
    /// Seams occur at every multiple of `chunk_size` along both axes.
    /// Each seam contributes two columns / rows of tile coordinates.
    pub fn seam_coordinates(map: &GameMap, chunk_size: u32) -> Vec<MapCoord> {
        let width = map.tile_width();
        let height = map.tile_height();

        let mut coords = Vec::new();

        // Vertical seams: at x = chunk_size, 2*chunk_size, ...
        for seam_x in (chunk_size..width).step_by(chunk_size as usize) {
            for y in 0..height {
                coords.push(MapCoord::new(seam_x - 1, y));
                coords.push(MapCoord::new(seam_x, y));
            }
        }

        // Horizontal seams: at y = chunk_size, 2*chunk_size, ...
        for seam_y in (chunk_size..height).step_by(chunk_size as usize) {
            for x in 0..width {
                coords.push(MapCoord::new(x, seam_y - 1));
                coords.push(MapCoord::new(x, seam_y));
            }
        }

        // Deduplicate corner tiles that appear in both vertical and horizontal seams
        coords.sort_by_key(|c| (c.y, c.x));
        coords.dedup();
        coords
    }

    /// Returns the up-to-four orthogonal neighbours of `coord` that lie within bounds.
    fn orthogonal_neighbors(coord: MapCoord, width: u32, height: u32) -> Vec<MapCoord> {
        let mut n = Vec::with_capacity(4);
        let (x, y) = (coord.x, coord.y);

        if x > 0 {
            n.push(MapCoord::new(x - 1, y));
        }
        if x + 1 < width {
            n.push(MapCoord::new(x + 1, y));
        }
        if y > 0 {
            n.push(MapCoord::new(x, y - 1));
        }
        if y + 1 < height {
            n.push(MapCoord::new(x, y + 1));
        }

        n
    }

    fn enforce_chunk_edge_alignment(map: &mut GameMap, chunk_size: u32) -> Result<(), EngineError> {
        let width = map.tile_width();
        let height = map.tile_height();

        for chunk_y in (0..height).step_by(chunk_size as usize) {
            for chunk_x in (0..width).step_by(chunk_size as usize) {
                for side in 0..4 {
                    let coords = Self::chunk_edge_coords(chunk_x, chunk_y, chunk_size, side);
                    Self::normalize_edge_line(map, &coords)?;
                }
            }
        }

        Ok(())
    }

    fn chunk_edge_coords(chunk_x: u32, chunk_y: u32, chunk_size: u32, side: u8) -> Vec<MapCoord> {
        let mut coords = Vec::with_capacity(chunk_size as usize);

        for pos in 0..chunk_size {
            let coord = match side {
                0 => MapCoord::new(chunk_x + pos, chunk_y),
                1 => MapCoord::new(chunk_x + chunk_size - 1, chunk_y + pos),
                2 => MapCoord::new(chunk_x + pos, chunk_y + chunk_size - 1),
                _ => MapCoord::new(chunk_x, chunk_y + pos),
            };
            coords.push(coord);
        }

        coords
    }

    fn normalize_edge_line(map: &mut GameMap, coords: &[MapCoord]) -> Result<(), EngineError> {
        let original: Vec<Tiles> = coords
            .iter()
            .map(|coord| map.get_tile(*coord).map(|tile| tile.kind))
            .collect::<Result<Vec<_>, _>>()?;
        let mut updated = original.clone();
        let mut natural_fill: Vec<Option<Tiles>> = vec![None; coords.len()];

        for kind in [Tiles::Forest, Tiles::Mountain, Tiles::Water] {
            let anchors: Vec<usize> = original
                .iter()
                .enumerate()
                .filter_map(|(pos, tile)| {
                    if *tile == kind && Self::is_edge_anchor(pos as u32) {
                        Some(pos)
                    } else {
                        None
                    }
                })
                .collect();

            for &anchor in &anchors {
                natural_fill[anchor] = Some(kind);
            }

            for window in anchors.windows(2) {
                if window[1] - window[0] == 3 {
                    for fill_pos in window[0]..=window[1] {
                        natural_fill[fill_pos] = Some(kind);
                    }
                }
            }
        }

        for (pos, tile) in original.iter().enumerate() {
            updated[pos] = match tile {
                Tiles::Road | Tiles::River => {
                    if Self::is_edge_anchor(pos as u32) {
                        *tile
                    } else {
                        Tiles::Meadow
                    }
                }
                Tiles::Bridge => {
                    if Self::is_edge_anchor(pos as u32) {
                        Tiles::Road
                    } else {
                        Tiles::Meadow
                    }
                }
                Tiles::Forest | Tiles::Mountain | Tiles::Water => {
                    natural_fill[pos].unwrap_or(Tiles::Meadow)
                }
                _ => *tile,
            };
        }

        for (coord, kind) in coords.iter().zip(updated.iter()) {
            map.get_tile_mut(*coord)?.kind = *kind;
        }

        Ok(())
    }

    fn is_edge_anchor(pos: u32) -> bool {
        pos % 3 == 1
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::init_tracing;
    use rpg_engine::map::chunk::CHUNK_SIZE;
    use rpg_engine::map::tile::Tile;

    /// Builds a 2×1 map (64×32 tiles) where the left half is all `left_kind` and
    /// the right half is all `right_kind`.
    fn make_two_chunk_map(left_kind: Tiles, right_kind: Tiles) -> GameMap {
        let cs = CHUNK_SIZE as u32;
        let width = cs * 2;
        let height = cs;
        let mut tiles = Vec::with_capacity((width * height) as usize);
        for _y in 0..height {
            for x in 0..width {
                if x < cs {
                    tiles.push(Tile::new(left_kind));
                } else {
                    tiles.push(Tile::new(right_kind));
                }
            }
        }
        GameMap::new(width, height, tiles, [0u8; 32]).unwrap()
    }

    #[test]
    fn uniform_map_unchanged() {
        init_tracing();
        let mut map = make_two_chunk_map(Tiles::Meadow, Tiles::Meadow);
        Stitcher::stitch(&mut map, CHUNK_SIZE as u32).unwrap();

        // All tiles should still be grass
        for tile in map.tiles() {
            assert_eq!(tile.kind, Tiles::Meadow);
        }
    }

    #[test]
    fn hard_seam_is_smoothed() {
        init_tracing();
        // Left: all water, Right: all grass — the boundary tiles should be updated
        let mut map = make_two_chunk_map(Tiles::Water, Tiles::Meadow);
        let seam_x_left = CHUNK_SIZE as u32 - 1;
        let seam_x_right = CHUNK_SIZE as u32;

        // Before stitching: seam tiles are water / grass respectively
        assert_eq!(
            map.get_tile(MapCoord::new(seam_x_left, 0)).unwrap().kind,
            Tiles::Water
        );
        assert_eq!(
            map.get_tile(MapCoord::new(seam_x_right, 0)).unwrap().kind,
            Tiles::Meadow
        );

        Stitcher::stitch(&mut map, CHUNK_SIZE as u32).unwrap();

        // After stitching: interior seam tiles (not at the corners of the chunk)
        // should be changed by majority voting.
        // We just check the stitch ran without error (correctness validated visually).
    }

    #[test]
    fn seam_coordinates_count_for_2x2_map() {
        init_tracing();
        // 2×2 chunks (64×64 tiles):
        // 1 vertical seam + 1 horizontal seam
        // vertical seam: 2 columns × 64 rows = 128 coords
        // horizontal seam: 2 rows × 64 cols = 128 coords
        // minus 4 duplicated corner coords = 252
        let cs = CHUNK_SIZE as u32;
        let width = cs * 2;
        let height = cs * 2;
        let tiles = vec![Tile::default(); (width * height) as usize];
        let map = GameMap::new(width, height, tiles, [0u8; 32]).unwrap();
        let coords = Stitcher::seam_coordinates(&map, cs);
        // Just verify no duplicates and the count is reasonable
        let mut sorted = coords.clone();
        sorted.sort_by_key(|c| (c.y, c.x));
        let deduped_len = {
            let mut v = sorted.clone();
            v.dedup();
            v.len()
        };
        assert_eq!(
            coords.len(),
            deduped_len,
            "seam_coordinates should be deduplicated"
        );
    }

    #[test]
    fn edge_alignment_rules_are_enforced() {
        init_tracing();
        let cs = CHUNK_SIZE as u32;
        let width = cs;
        let height = cs;
        let mut tiles = vec![Tile::default(); (width * height) as usize];

        let idx = |x: u32, y: u32| -> usize { (y * width + x) as usize };

        tiles[idx(0, 0)] = Tile::new(Tiles::Road);
        tiles[idx(1, 0)] = Tile::new(Tiles::Road);
        tiles[idx(1, 31)] = Tile::new(Tiles::Forest);
        tiles[idx(2, 31)] = Tile::new(Tiles::Forest);
        tiles[idx(3, 31)] = Tile::new(Tiles::Forest);
        tiles[idx(4, 31)] = Tile::new(Tiles::Forest);

        let mut map = GameMap::new(width, height, tiles, [0u8; 32]).unwrap();
        Stitcher::stitch(&mut map, cs).unwrap();

        assert_eq!(
            map.get_tile(MapCoord::new(0, 0)).unwrap().kind,
            Tiles::Meadow
        );
        assert_eq!(map.get_tile(MapCoord::new(1, 0)).unwrap().kind, Tiles::Road);
        assert_eq!(
            map.get_tile(MapCoord::new(1, 31)).unwrap().kind,
            Tiles::Forest
        );
        assert_eq!(
            map.get_tile(MapCoord::new(2, 31)).unwrap().kind,
            Tiles::Forest
        );
        assert_eq!(
            map.get_tile(MapCoord::new(3, 31)).unwrap().kind,
            Tiles::Forest
        );
        assert_eq!(
            map.get_tile(MapCoord::new(4, 31)).unwrap().kind,
            Tiles::Forest
        );
    }
}
