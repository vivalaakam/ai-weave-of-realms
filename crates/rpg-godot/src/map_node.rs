//! `MapNode` GDExtension node — populates a `TileMapLayer` from `GameManager` map data.
//!
//! `MapNode` is intentionally thin — it reads tile data from `GameManager`
//! and writes GIDs to a `TileMapLayer`.  All game logic stays in `GameManager`.

use godot::classes::tile_set::TileShape;
use godot::classes::{
    INode, Node, ResourceLoader, Texture2D, TileMapLayer, TileSet, TileSetAtlasSource,
    TileSetSource,
};
use godot::prelude::*;

/// Number of tiles in `res://assets/tileset.png` (832 px ÷ 64 px).
const ATLAS_COLS: i32 = 13;
/// Pixel size of each tile in the atlas sheet.
const ATLAS_TILE_PX: i32 = 64;

// ─── MapNode ──────────────────────────────────────────────────────────────────

#[derive(GodotClass)]
#[class(base=Node)]
pub struct MapNode {
    base: Base<Node>,
    /// Path to the `GameManager` node in the scene tree.
    #[var]
    pub game_manager_path: NodePath,
}

#[godot_api]
impl INode for MapNode {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            game_manager_path: NodePath::from("/root/GameManager"),
        }
    }
}

#[godot_api]
impl MapNode {
    #[signal]
    fn tilemap_populated(width: i64, height: i64);

    /// Builds a `TileSet` from `res://assets/tileset.png`, assigns it to
    /// `tilemap`, then fills every cell with the GID supplied by `GameManager`.
    #[func]
    pub fn populate_tilemap(&mut self, tilemap: Gd<TileMapLayer>) {
        let manager = self.resolve_manager();
        let Some(gm) = manager else {
            godot_warn!("MapNode: GameManager not found at {:?}", self.game_manager_path);
            return;
        };

        let width  = gm.bind().get_map_width();
        let height = gm.bind().get_map_height();

        let mut tm = tilemap;

        // ── Create and assign TileSet ─────────────────────────────────────────
        if let Some(ts) = Self::build_tileset() {
            tm.set_tile_set(&ts);
        } else {
            godot_warn!("MapNode: failed to build TileSet — tiles will not render");
        }

        // ── Fill cells ────────────────────────────────────────────────────────
        for y in 0..height {
            for x in 0..width {
                let gid  = gm.bind().get_tile_gid(x, y);
                if gid <= 0 { continue; }
                let col  = ((gid - 1) as i32).clamp(0, ATLAS_COLS - 1);
                let cell = Vector2i::new(x as i32, y as i32);
                tm.set_cell_ex(cell)
                    .source_id(0)
                    .atlas_coords(Vector2i::new(col, 0))
                    .done();
            }
        }

        self.base_mut().emit_signal("tilemap_populated", &[
            width.to_variant(),
            height.to_variant(),
        ]);
    }

    /// Returns the tile kind string at `(x, y)` from `GameManager`.
    #[func]
    pub fn get_tile_kind(&self, x: i64, y: i64) -> GString {
        self.resolve_manager()
            .map(|gm| gm.bind().get_tile_kind(x, y))
            .unwrap_or_default()
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Builds a [`TileSet`] with one [`TileSetAtlasSource`] (source id 0)
    /// loaded from `res://assets/tileset.png`.
    fn build_tileset() -> Option<Gd<TileSet>> {
        // Load texture
        let tex: Gd<Texture2D> = ResourceLoader::singleton()
            .load("res://assets/tileset.png")
            .and_then(|r| r.try_cast::<Texture2D>().ok())?;

        // Atlas source
        let mut atlas = TileSetAtlasSource::new_gd();
        atlas.set_texture(&tex);
        atlas.set_texture_region_size(Vector2i::new(ATLAS_TILE_PX, ATLAS_TILE_PX));

        for col in 0..ATLAS_COLS {
            atlas.create_tile(Vector2i::new(col, 0));
        }

        // TileSet — isometric mode
        let mut ts = TileSet::new_gd();
        ts.set_tile_shape(TileShape::ISOMETRIC);
        // For isometric: tile_size is the cell size in world space (diamond footprint).
        // With 64x64 source tiles displayed as isometric diamonds, use 64x32 (2:1 ratio).
        ts.set_tile_size(Vector2i::new(ATLAS_TILE_PX, ATLAS_TILE_PX / 2));

        let source: Gd<TileSetSource> = atlas.upcast();
        ts.add_source_ex(&source).atlas_source_id_override(0).done();

        Some(ts)
    }

    fn resolve_manager(&self) -> Option<Gd<super::game_manager::GameManager>> {
        let path = self.game_manager_path.clone();
        self.base()
            .get_node_or_null(&path)
            .and_then(|n| n.try_cast::<super::game_manager::GameManager>().ok())
    }
}
