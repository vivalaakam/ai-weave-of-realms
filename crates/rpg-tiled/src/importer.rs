//! TMX importer — deserialises a Tiled `.tmx` XML file into a [`GameMap`].
//!
//! Only the features produced by [`crate::exporter`] are supported:
//! - Isometric staggered layout
//! - A single `<layer>` with `encoding="csv"`
//! - An optional `seed` string property (64-character hex)
//!
//! Unknown attributes and extra layers are silently ignored.

use std::path::Path;

use quick_xml::Reader;
use quick_xml::events::Event;

use rpg_engine::map::game_map::GameMap;
use rpg_engine::map::tile::{Tile, Tiles};

use crate::error::Error;

// ─── Public API ───────────────────────────────────────────────────────────────

/// Parses a TMX XML string and returns a [`GameMap`].
///
/// # Errors
/// - [`Error::Parse`] — malformed XML
/// - [`Error::MissingField`] — required attribute absent
/// - [`Error::InvalidAttribute`] — attribute value not parseable
/// - [`Error::UnknownGid`] — CSV contains an unrecognised tile GID
/// - [`Error::Engine`] — `GameMap::new` rejected the assembled tiles
pub fn import_tmx(xml: &str) -> Result<GameMap, Error> {
    let mut parser = TmxParser::default();
    parser.parse(xml)?;
    parser.into_game_map()
}

/// Reads a `.tmx` file from `path` and returns a [`GameMap`].
///
/// # Errors
/// Returns [`Error::Io`] if the file cannot be read, or any import error.
pub fn read_tmx(path: &Path) -> Result<GameMap, Error> {
    let xml = std::fs::read_to_string(path)?;
    import_tmx(&xml)
}

// ─── Parser state machine ─────────────────────────────────────────────────────

#[derive(Default)]
struct TmxParser {
    width: Option<u32>,
    height: Option<u32>,
    seed: [u8; 32],
    /// Whether we are currently inside a `<data encoding="csv">` element.
    in_csv_data: bool,
    /// Whether the next property value is the map seed.
    next_property_is_seed: bool,
    /// Accumulated CSV text from `<data>`.
    csv: String,
}

impl TmxParser {
    fn parse(&mut self, xml: &str) -> Result<(), Error> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(false);

        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) => self.handle_start(&e)?,
                Ok(Event::Empty(e)) => self.handle_empty(&e)?,
                Ok(Event::Text(e)) => {
                    if self.in_csv_data {
                        self.csv.push_str(e.unescape()?.as_ref());
                    }
                }
                Ok(Event::End(e)) => {
                    if e.name().as_ref() == b"data" {
                        self.in_csv_data = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(Error::Parse(e)),
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_start(&mut self, e: &quick_xml::events::BytesStart<'_>) -> Result<(), Error> {
        match e.name().as_ref() {
            b"map" => {
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"width" => {
                            self.width = Some(parse_u32_attr("width", &attr.value)?);
                        }
                        b"height" => {
                            self.height = Some(parse_u32_attr("height", &attr.value)?);
                        }
                        _ => {}
                    }
                }
            }
            b"data" => {
                // Only accept CSV encoding
                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == b"encoding"
                        && attr.value.as_ref() == b"csv"
                    {
                        self.in_csv_data = true;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_empty(&mut self, e: &quick_xml::events::BytesStart<'_>) -> Result<(), Error> {
        match e.name().as_ref() {
            b"property" => {
                let mut is_seed = false;
                let mut value: Option<String> = None;

                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"name" if attr.value.as_ref() == b"seed" => is_seed = true,
                        b"value" => {
                            value = Some(
                                std::str::from_utf8(&attr.value)
                                    .unwrap_or("")
                                    .to_owned(),
                            );
                        }
                        _ => {}
                    }
                }

                if is_seed {
                    if let Some(hex) = value {
                        self.seed = hex_decode(&hex)?;
                    }
                }
            }
            b"data" => {
                // Self-closing <data/> — nothing to parse
            }
            _ => {}
        }
        Ok(())
    }

    fn into_game_map(self) -> Result<GameMap, Error> {
        let width = self.width.ok_or_else(|| Error::MissingField("width".into()))?;
        let height = self.height.ok_or_else(|| Error::MissingField("height".into()))?;

        let gids = parse_csv(&self.csv)?;
        let expected = (width * height) as usize;
        if gids.len() != expected {
            return Err(Error::DimensionMismatch {
                expected: format!("{expected} tiles ({width}×{height})"),
                got: format!("{} GIDs in CSV", gids.len()),
            });
        }

        let tiles: Result<Vec<Tile>, Error> = gids
            .into_iter()
            .map(|gid| {
                Tiles::from_gid(gid)
                    .map(|kind| Tile { kind })
                    .map_err(|_| Error::UnknownGid(gid))
            })
            .collect();

        GameMap::new(width, height, tiles?, self.seed).map_err(Error::Engine)
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn parse_u32_attr(field: &str, raw: &[u8]) -> Result<u32, Error> {
    let s = std::str::from_utf8(raw).unwrap_or("");
    s.parse::<u32>().map_err(|_| Error::InvalidAttribute {
        field: field.to_owned(),
        value: s.to_owned(),
    })
}

/// Parses a comma-separated list of GID strings, ignoring whitespace.
fn parse_csv(csv: &str) -> Result<Vec<u32>, Error> {
    csv.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<u32>().map_err(|_| Error::InvalidAttribute {
                field: "csv gid".into(),
                value: s.to_owned(),
            })
        })
        .collect()
}

/// Decodes a 64-character hex string into a 32-byte seed.
fn hex_decode(hex: &str) -> Result<[u8; 32], Error> {
    if hex.len() != 64 {
        return Err(Error::InvalidAttribute {
            field: "seed".into(),
            value: hex.to_owned(),
        });
    }
    let mut out = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let hi = hex_nibble(chunk[0])?;
        let lo = hex_nibble(chunk[1])?;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Result<u8, Error> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(Error::InvalidAttribute {
            field: "seed hex char".into(),
            value: (b as char).to_string(),
        }),
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exporter::export_tmx;
    use rpg_engine::map::game_map::GameMap;
    use rpg_engine::map::tile::{Tile, Tiles};

    fn make_map(w: u32, h: u32, kind: Tiles, seed: [u8; 32]) -> GameMap {
        let tiles = vec![Tile { kind }; (w * h) as usize];
        GameMap::new(w, h, tiles, seed).unwrap()
    }

    #[test]
    fn round_trip_dimensions() {
        let map = make_map(32, 32, Tiles::Meadow, [0u8; 32]);
        let xml = export_tmx(&map, "t.tsx");
        let imported = import_tmx(&xml).unwrap();
        assert_eq!(imported.tile_width(), 32);
        assert_eq!(imported.tile_height(), 32);
    }

    #[test]
    fn round_trip_tiles() {
        let map = make_map(4, 4, Tiles::Water, [1u8; 32]);
        let xml = export_tmx(&map, "t.tsx");
        let imported = import_tmx(&xml).unwrap();
        for tile in imported.tiles() {
            assert_eq!(tile.kind, Tiles::Water);
        }
    }

    #[test]
    fn round_trip_seed() {
        let seed: [u8; 32] = {
            let mut s = [0u8; 32];
            for (i, b) in s.iter_mut().enumerate() {
                *b = i as u8;
            }
            s
        };
        let map = make_map(2, 2, Tiles::Meadow, seed);
        let xml = export_tmx(&map, "t.tsx");
        let imported = import_tmx(&xml).unwrap();
        assert_eq!(imported.seed, seed);
    }

    #[test]
    fn round_trip_mixed_tiles() {
        let kinds = [
            Tiles::Meadow, Tiles::Water, Tiles::Forest, Tiles::Mountain,
            Tiles::Road, Tiles::River, Tiles::City, Tiles::CityEntrance,
            Tiles::Gold, Tiles::Resource,
        ];
        let tiles: Vec<Tile> = (0..10)
            .map(|i| Tile { kind: kinds[i % kinds.len()] })
            .collect();
        let map = GameMap::new(10, 1, tiles, [0u8; 32]).unwrap();
        let xml = export_tmx(&map, "t.tsx");
        let imported = import_tmx(&xml).unwrap();
        for (orig, imp) in map.tiles().iter().zip(imported.tiles()) {
            assert_eq!(orig.kind, imp.kind);
        }
    }

    #[test]
    fn missing_width_returns_error() {
        let xml = r#"<?xml version="1.0"?>
<map height="4">
  <layer id="1" name="Terrain" width="0" height="4">
    <data encoding="csv">1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1</data>
  </layer>
</map>"#;
        assert!(matches!(import_tmx(xml), Err(Error::MissingField(_))));
    }

    #[test]
    fn unknown_gid_returns_error() {
        let xml = r#"<?xml version="1.0"?>
<map width="2" height="2">
  <layer id="1" name="Terrain" width="2" height="2">
    <data encoding="csv">1,999,1,1</data>
  </layer>
</map>"#;
        assert!(matches!(import_tmx(xml), Err(Error::UnknownGid(999))));
    }
}
