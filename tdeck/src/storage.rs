//! SD-card map discovery and TMX loading.

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use embedded_sdmmc::{LfnBuffer, Mode, VolumeIdx, VolumeManager};
use rpg_engine::map::game_map::{GameMap, MapCoord};
use rpg_engine::map::tile::{Tile, Tiles};

const MAPS_DIR: &str = "MAPS";

/// Map list entry shown in the selector UI.
#[derive(Clone)]
pub struct MapEntry {
    /// Short 8.3 filename on the SD card.
    pub short_name: String,
    /// Human-readable filename.
    pub display_name: String,
    /// File size in bytes.
    pub size_bytes: u32,
}

/// Parsed map data returned by TMX loading.
pub struct LoadedMap {
    /// Human-readable map name.
    pub name: String,
    /// Parsed engine map.
    pub map: GameMap,
}

/// App-level error type for storage, TMX parsing, and engine session setup.
pub enum AppError {
    /// SD card could not be opened or read.
    StorageUnavailable,
    /// `/maps` or `/MAPS` directory was not found.
    MapsDirMissing,
    /// No `.tmx` files were found.
    NoMapsFound,
    /// The TMX payload is malformed.
    InvalidTmx(&'static str),
    /// Compile-time requested map is not present on the SD card.
    InvalidConfiguredMap,
    /// The parsed map could not be accepted by `rpg-engine`.
    Engine(String),
}

/// Discovers all `.tmx` files under `/maps`.
pub fn discover_maps<D>(
    volume_mgr: &VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
) -> Result<Vec<MapEntry>, AppError>
where
    D: embedded_sdmmc::BlockDevice,
{
    let volume = volume_mgr
        .open_volume(VolumeIdx(0))
        .map_err(|_| AppError::StorageUnavailable)?;
    let root_dir = volume
        .open_root_dir()
        .map_err(|_| AppError::StorageUnavailable)?;

    let maps_dir = root_dir
        .open_dir(MAPS_DIR)
        .or_else(|_| root_dir.open_dir("maps"))
        .map_err(|_| AppError::MapsDirMissing)?;

    let mut entries: Vec<MapEntry> = Vec::new();
    let mut lfn_storage: [u8; 128] = [0; 128];
    let mut lfn_buffer = LfnBuffer::new(&mut lfn_storage);

    maps_dir
        .iterate_dir_lfn(&mut lfn_buffer, |dir_entry, long_name| {
            if dir_entry.attributes.is_directory() {
                return;
            }

            let short_name = dir_entry.name.to_string();
            let display_name = long_name.unwrap_or(short_name.as_str()).to_string();

            if has_tmx_extension(&display_name) || has_tmx_extension(&short_name) {
                entries.push(MapEntry {
                    short_name,
                    display_name,
                    size_bytes: dir_entry.size,
                });
            }
        })
        .map_err(|_| AppError::StorageUnavailable)?;

    entries.sort_unstable_by(|left, right| left.display_name.cmp(&right.display_name));

    if entries.is_empty() {
        return Err(AppError::NoMapsFound);
    }

    Ok(entries)
}

/// Loads a TMX file from SD card and converts it to `rpg-engine::GameMap`.
pub fn load_map<D>(
    volume_mgr: &VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    entry: &MapEntry,
) -> Result<LoadedMap, AppError>
where
    D: embedded_sdmmc::BlockDevice,
{
    let volume = volume_mgr
        .open_volume(VolumeIdx(0))
        .map_err(|_| AppError::StorageUnavailable)?;
    let root_dir = volume
        .open_root_dir()
        .map_err(|_| AppError::StorageUnavailable)?;
    let maps_dir = root_dir
        .open_dir(MAPS_DIR)
        .or_else(|_| root_dir.open_dir("maps"))
        .map_err(|_| AppError::MapsDirMissing)?;
    let file = maps_dir
        .open_file_in_dir(entry.short_name.as_str(), Mode::ReadOnly)
        .map_err(|_| AppError::StorageUnavailable)?;

    let file_len = file.length() as usize;
    let mut bytes: Vec<u8> = alloc::vec![0; file_len];
    let mut offset = 0usize;

    while !file.is_eof() && offset < bytes.len() {
        let read = file
            .read(&mut bytes[offset..])
            .map_err(|_| AppError::StorageUnavailable)?;
        if read == 0 {
            break;
        }
        offset += read;
    }

    bytes.truncate(offset);
    let xml = core::str::from_utf8(&bytes).map_err(|_| AppError::InvalidTmx("TMX is not UTF-8"))?;
    parse_tmx(entry.display_name.as_str(), xml)
}

/// Returns a user-facing error string.
pub fn error_message(error: AppError) -> String {
    match error {
        AppError::StorageUnavailable => "SD card is unavailable or unreadable".to_string(),
        AppError::MapsDirMissing => "Folder /maps was not found on the SD card".to_string(),
        AppError::NoMapsFound => "No .tmx maps were found in /maps".to_string(),
        AppError::InvalidTmx(message) => format!("TMX parse error: {message}"),
        AppError::InvalidConfiguredMap => {
            "TDECK_START_MAP does not match any map in /maps".to_string()
        }
        AppError::Engine(message) => format!("Engine error: {message}"),
    }
}

/// Returns `true` when both names match case-insensitively.
pub fn names_match(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn parse_tmx(name: &str, xml: &str) -> Result<LoadedMap, AppError> {
    let map_tag_start = xml.find("<map").ok_or(AppError::InvalidTmx("Missing <map> tag"))?;
    let map_tag_end = xml[map_tag_start..]
        .find('>')
        .ok_or(AppError::InvalidTmx("Malformed <map> tag"))?
        + map_tag_start;
    let map_tag = &xml[map_tag_start..=map_tag_end];

    let width = parse_attribute_u32(map_tag, "width")?;
    let height = parse_attribute_u32(map_tag, "height")?;

    let data_start_tag = "<data encoding=\"csv\">";
    let data_start = xml
        .find(data_start_tag)
        .ok_or(AppError::InvalidTmx("Missing CSV layer data"))?
        + data_start_tag.len();
    let data_end = xml[data_start..]
        .find("</data>")
        .ok_or(AppError::InvalidTmx("Unclosed CSV layer data"))?
        + data_start;

    let csv = &xml[data_start..data_end];
    let mut tiles: Vec<Tile> = Vec::with_capacity(width as usize * height as usize);
    for value in csv.split(',') {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }

        let gid = trimmed
            .parse::<u32>()
            .map_err(|_| AppError::InvalidTmx("Invalid tile gid"))?;
        if gid == 0 {
            return Err(AppError::InvalidTmx("GID 0 is not supported by T-Deck maps"));
        }

        let kind = Tiles::from_gid(gid).map_err(|err| AppError::Engine(err.to_string()))?;
        tiles.push(Tile::new(kind));
    }

    if tiles.len() != width as usize * height as usize {
        return Err(AppError::InvalidTmx("Tile count does not match map size"));
    }

    let (enemy_spawns, chest_spawns) = parse_spawn_points(xml)?;

    let mut map = GameMap::new(width, height, tiles, [0u8; 32])
        .map_err(|err| AppError::Engine(err.to_string()))?;
    map.set_spawn_points(enemy_spawns, chest_spawns)
        .map_err(|err| AppError::Engine(err.to_string()))?;

    Ok(LoadedMap {
        name: name.to_string(),
        map,
    })
}

fn parse_spawn_points(xml: &str) -> Result<(Vec<MapCoord>, Vec<MapCoord>), AppError> {
    let mut enemy_spawns = Vec::new();
    let mut chest_spawns = Vec::new();
    let mut search_start = 0usize;

    while let Some(group_start) = xml[search_start..].find("<objectgroup") {
        let group_start = search_start + group_start;
        let group_tag_end = xml[group_start..]
            .find('>')
            .ok_or(AppError::InvalidTmx("Malformed <objectgroup> tag"))?
            + group_start;
        let group_tag = &xml[group_start..=group_tag_end];

        let group_end = xml[group_tag_end..]
            .find("</objectgroup>")
            .ok_or(AppError::InvalidTmx("Unclosed <objectgroup> tag"))?
            + group_tag_end;
        let group_name = match parse_attribute_str(group_tag, "name") {
            Ok(name) => name,
            Err(_) => {
                search_start = group_end + "</objectgroup>".len();
                continue;
            }
        };

        if group_name == "Spawns" {
            let group_body = &xml[group_tag_end..group_end];
            parse_spawn_objects(group_body, &mut enemy_spawns, &mut chest_spawns)?;
            break;
        }

        search_start = group_end + "</objectgroup>".len();
    }

    Ok((enemy_spawns, chest_spawns))
}

fn parse_spawn_objects(
    body: &str,
    enemy_spawns: &mut Vec<MapCoord>,
    chest_spawns: &mut Vec<MapCoord>,
) -> Result<(), AppError> {
    let mut search_start = 0usize;
    while let Some(obj_start) = body[search_start..].find("<object") {
        let obj_start = search_start + obj_start;
        let tag_end = body[obj_start..]
            .find('>')
            .ok_or(AppError::InvalidTmx("Malformed <object> tag"))?
            + obj_start;
        let obj_tag = &body[obj_start..=tag_end];
        let obj_end = body[tag_end..]
            .find("</object>")
            .ok_or(AppError::InvalidTmx("Unclosed <object> tag"))?
            + tag_end;
        let obj_body = &body[tag_end..obj_end];

        let kind = parse_attribute_str(obj_tag, "type")
            .or_else(|_| parse_attribute_str(obj_tag, "name"))
            .unwrap_or_default();

        let tile_x = parse_property_u32(obj_body, "tile_x")?;
        let tile_y = parse_property_u32(obj_body, "tile_y")?;

        if let (Some(x), Some(y)) = (tile_x, tile_y) {
            let coord = MapCoord::new(x, y);
            if kind == "enemy" || kind == "EnemySpawn" {
                enemy_spawns.push(coord);
            } else if kind == "chest" || kind == "ChestSpawn" {
                chest_spawns.push(coord);
            }
        }

        search_start = obj_end + "</object>".len();
    }
    Ok(())
}

fn parse_property_u32(body: &str, property_name: &str) -> Result<Option<u32>, AppError> {
    let needle = format!("name=\"{property_name}\"");
    let Some(start) = body.find(needle.as_str()) else {
        return Ok(None);
    };
    let prop_start = body[..start]
        .rfind("<property")
        .ok_or(AppError::InvalidTmx("Malformed <property> tag"))?;
    let prop_end = body[prop_start..]
        .find('>')
        .ok_or(AppError::InvalidTmx("Malformed <property> tag"))?
        + prop_start;
    let prop_tag = &body[prop_start..=prop_end];

    let value = parse_attribute_u32(prop_tag, "value")?;
    Ok(Some(value))
}

fn parse_attribute_str(tag: &str, attribute: &str) -> Result<String, AppError> {
    let needle = format!("{attribute}=\"");
    let start = tag
        .find(needle.as_str())
        .ok_or(AppError::InvalidTmx("Missing attribute"))?
        + needle.len();
    let end = tag[start..]
        .find('"')
        .ok_or(AppError::InvalidTmx("Malformed attribute"))?
        + start;
    Ok(tag[start..end].to_string())
}

fn parse_attribute_u32(tag: &str, attribute: &str) -> Result<u32, AppError> {
    let needle = format!("{attribute}=\"");
    let start = tag
        .find(needle.as_str())
        .ok_or(AppError::InvalidTmx("Missing map dimension"))?
        + needle.len();
    let end = tag[start..]
        .find('"')
        .ok_or(AppError::InvalidTmx("Malformed map attribute"))?
        + start;

    tag[start..end]
        .parse::<u32>()
        .map_err(|_| AppError::InvalidTmx("Invalid numeric map attribute"))
}

fn has_tmx_extension(name: &str) -> bool {
    let lower = name.as_bytes();
    lower.len() >= 4
        && lower[lower.len() - 4].eq_ignore_ascii_case(&b'.')
        && lower[lower.len() - 3].eq_ignore_ascii_case(&b't')
        && lower[lower.len() - 2].eq_ignore_ascii_case(&b'm')
        && lower[lower.len() - 1].eq_ignore_ascii_case(&b'x')
}
