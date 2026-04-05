//! TMX exporter — serialises a [`GameMap`] to Tiled `.tmx` XML.
//!
//! ## TMX format
//! Isometric staggered layout (`staggeraxis="x"`, `staggerindex="odd"`).
//! Tile data is encoded as CSV.  The map seed is stored as a custom string
//! property so that a full round-trip through [`crate::importer`] is lossless.
//! Enemy/chest spawn points are exported as a `Spawns` object layer.

use std::path::Path;

use rpg_engine::map::game_map::{GameMap, MapCoord};

use crate::error::Error;

/// Pixel width and height of each isometric tile in the tileset.
pub const TILE_PIXEL_WIDTH: u32 = 64;
/// Pixel width and height of each isometric tile in the tileset.
pub const TILE_PIXEL_HEIGHT: u32 = 64;

// ─── Public API ───────────────────────────────────────────────────────────────

/// Serialises `map` to a TMX XML string.
///
/// `tileset_source` is the path written into the `<tileset source="…"/>` element.
/// It should be relative to the directory where the `.tmx` file will be saved.
pub fn export_tmx(map: &GameMap, tileset_source: &str) -> String {
    let seed_hex = hex_encode(&map.seed);
    let csv = tile_csv(map);
    let spawn_objects = spawn_objects_xml(map);
    let spawn_layer = spawn_layer_xml(&spawn_objects);
    let next_object_id = spawn_objects.len() as u32 + 1;

    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<map version=\"1.10\" tiledversion=\"1.11.0\" orientation=\"staggered\" ",
            "renderorder=\"right-down\" width=\"{width}\" height=\"{height}\" ",
            "tilewidth=\"{tw}\" tileheight=\"{th}\" infinite=\"0\" ",
            "staggeraxis=\"x\" staggerindex=\"odd\" nextlayerid=\"3\" nextobjectid=\"{next_object_id}\">\n",
            "  <properties>\n",
            "    <property name=\"seed\" type=\"string\" value=\"{seed}\"/>\n",
            "  </properties>\n",
            "  <tileset firstgid=\"1\" source=\"{tileset}\"/>\n",
            "  <layer id=\"1\" name=\"Terrain\" width=\"{width}\" height=\"{height}\">\n",
            "    <data encoding=\"csv\">\n",
            "{csv}\n",
            "    </data>\n",
            "  </layer>\n",
            "{spawn_layer}",
            "</map>\n"
        ),
        width = map.tile_width(),
        height = map.tile_height(),
        tw = TILE_PIXEL_WIDTH,
        th = TILE_PIXEL_HEIGHT,
        seed = seed_hex,
        tileset = xml_escape_attr(tileset_source),
        next_object_id = next_object_id,
        csv = csv,
        spawn_layer = spawn_layer,
    )
}

/// Writes `map` as a `.tmx` file to `path`.
///
/// # Errors
/// Returns [`Error::Io`] if the file cannot be written.
pub fn write_tmx(map: &GameMap, path: &Path, tileset_source: &str) -> Result<(), Error> {
    let xml = export_tmx(map, tileset_source);
    std::fs::write(path, xml)?;
    tracing::info!(path = %path.display(), "TMX written");
    Ok(())
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Serialises all tile GIDs as a TMX-compatible CSV string.
fn tile_csv(map: &GameMap) -> String {
    let mut rows: Vec<String> = Vec::with_capacity(map.tile_height() as usize);
    for ty in 0..map.tile_height() {
        let mut gids: Vec<String> = Vec::with_capacity(map.tile_width() as usize);
        for tx in 0..map.tile_width() {
            let coord = MapCoord::new(tx, ty);
            let gid = map.get_tile(coord).map(|t| t.kind.to_gid()).unwrap_or(0);
            gids.push(gid.to_string());
        }
        rows.push(format!("      {}", gids.join(",")));
    }
    rows.join(",\n")
}

fn spawn_layer_xml(spawn_objects: &[String]) -> String {
    let mut out = String::from("  <objectgroup id=\"2\" name=\"Spawns\">\n");
    for obj in spawn_objects {
        out.push_str(obj);
    }
    out.push_str("  </objectgroup>\n");
    out
}

fn spawn_objects_xml(map: &GameMap) -> Vec<String> {
    let mut objects: Vec<String> = Vec::new();
    let mut next_id: u32 = 1;

    for coord in map.enemy_spawns() {
        objects.push(spawn_object_xml(next_id, coord, "enemy", "EnemySpawn"));
        next_id += 1;
    }

    for coord in map.chest_spawns() {
        objects.push(spawn_object_xml(next_id, coord, "chest", "ChestSpawn"));
        next_id += 1;
    }

    objects
}

fn spawn_object_xml(id: u32, coord: &MapCoord, kind: &str, name: &str) -> String {
    let (x, y) = spawn_object_point(coord.x, coord.y);
    format!(
        concat!(
            "    <object id=\"{id}\" name=\"{name}\" type=\"{kind}\" x=\"{x}\" y=\"{y}\">\n",
            "      <properties>\n",
            "        <property name=\"tile_x\" type=\"int\" value=\"{tile_x}\"/>\n",
            "        <property name=\"tile_y\" type=\"int\" value=\"{tile_y}\"/>\n",
            "      </properties>\n",
            "      <point/>\n",
            "    </object>\n"
        ),
        id = id,
        name = name,
        kind = kind,
        x = x,
        y = y,
        tile_x = coord.x,
        tile_y = coord.y,
    )
}

fn spawn_object_point(tile_x: u32, tile_y: u32) -> (u32, u32) {
    let half_w = TILE_PIXEL_WIDTH / 2;
    let half_h = TILE_PIXEL_HEIGHT / 2;
    let x = tile_x * half_w + half_w;
    let mut y = tile_y * TILE_PIXEL_HEIGHT + half_h;
    if tile_x % 2 == 1 {
        y += half_h;
    }
    (x, y)
}

/// Hex-encodes a 32-byte seed into a 64-character lowercase string.
pub(crate) fn hex_encode(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Escapes special XML attribute characters.
fn xml_escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rpg_engine::map::game_map::GameMap;
    use rpg_engine::map::tile::{Tile, Tiles};

    fn meadow_map(w: u32, h: u32) -> GameMap {
        let tiles = vec![
            Tile {
                kind: Tiles::Meadow
            };
            (w * h) as usize
        ];
        GameMap::new(w, h, tiles, [0u8; 32]).unwrap()
    }

    #[test]
    fn export_contains_dimensions() {
        let map = meadow_map(32, 32);
        let xml = export_tmx(&map, "tileset.tsx");
        assert!(xml.contains("width=\"32\""));
        assert!(xml.contains("height=\"32\""));
    }

    #[test]
    fn export_contains_tileset_source() {
        let map = meadow_map(4, 4);
        let xml = export_tmx(&map, "../tileset/tileset.tsx");
        assert!(xml.contains("source=\"../tileset/tileset.tsx\""));
    }

    #[test]
    fn export_contains_seed_property() {
        let seed = [0xabu8; 32];
        let tiles = vec![
            Tile {
                kind: Tiles::Meadow
            };
            4
        ];
        let map = GameMap::new(2, 2, tiles, seed).unwrap();
        let xml = export_tmx(&map, "t.tsx");
        let expected_hex = "ab".repeat(32);
        assert!(xml.contains(&expected_hex));
    }

    #[test]
    fn csv_gid_count_matches_tile_count() {
        let map = meadow_map(4, 3);
        let xml = export_tmx(&map, "t.tsx");
        // Extract CSV text (skip past the opening tag)
        let tag = "<data encoding=\"csv\">";
        let start = xml.find(tag).unwrap() + tag.len();
        let end = xml.find("</data>").unwrap();
        let csv_block = &xml[start..end];
        let gid_count = csv_block
            .split(',')
            .filter(|s| s.trim().parse::<u32>().is_ok())
            .count();
        assert_eq!(gid_count, 12);
    }
}
