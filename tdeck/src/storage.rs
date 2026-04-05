//! SD-card map discovery and TMX loading.

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use embedded_sdmmc::{LfnBuffer, Mode, VolumeIdx, VolumeManager};
use rpg_engine::map::game_map::GameMap;
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

    let map = GameMap::new(width, height, tiles, [0u8; 32])
        .map_err(|err| AppError::Engine(err.to_string()))?;

    Ok(LoadedMap {
        name: name.to_string(),
        map,
    })
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
