//! SD-card map discovery and TMX loading.

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use embedded_sdmmc::{LfnBuffer, Mode, VolumeIdx, VolumeManager};
use rpg_engine::game_state::GameState;
use rpg_engine::map::game_map::{GameMap, MapCoord};
use rpg_engine::map::tile::{Tile, Tiles};

const MAPS_DIR: &str = "MAPS";
const SAVE_DIR: &str = "SAVEGAME";

/// Map/save list entry shown in the selector UI.
#[derive(Clone)]
pub struct MapEntry {
    /// Short 8.3 filename on the SD card.
    pub short_name: String,
    /// Human-readable filename.
    pub display_name: String,
    /// File size in bytes.
    pub size_bytes: u32,
}

/// Parsed data returned by map/save loading.
pub struct LoadedMap {
    /// Human-readable map name.
    pub name: String,
    /// Loaded session payload.
    pub payload: LoadedPayload,
}

/// Loaded map or save payload.
pub enum LoadedPayload {
    /// Raw engine map payload (TMX).
    Map(GameMap),
    /// Full engine save state.
    Save(GameState),
}

/// App-level error type for storage, TMX parsing, and engine session setup.
pub enum AppError {
    /// SD card could not be opened or read.
    StorageUnavailable,
    /// `/maps` or `/MAPS` directory was not found.
    MapsDirMissing,
    /// `/savegame` directory was not found.
    SaveDirMissing,
    /// No `.tmx` files were found.
    NoMapsFound,
    /// No save files were found.
    NoSavesFound,
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

/// Discovers all save files under `/savegame`.
pub fn discover_saves<D>(
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

    let save_dir = root_dir
        .open_dir(SAVE_DIR)
        .or_else(|_| root_dir.open_dir("savegame"))
        .map_err(|_| AppError::SaveDirMissing)?;

    let mut entries: Vec<MapEntry> = Vec::new();
    let mut lfn_storage: [u8; 128] = [0; 128];
    let mut lfn_buffer = LfnBuffer::new(&mut lfn_storage);

    save_dir
        .iterate_dir_lfn(&mut lfn_buffer, |dir_entry, long_name| {
            if dir_entry.attributes.is_directory() {
                return;
            }

            let short_name = dir_entry.name.to_string();
            let display_name = long_name.unwrap_or(short_name.as_str()).to_string();

            entries.push(MapEntry {
                short_name,
                display_name,
                size_bytes: dir_entry.size,
            });
        })
        .map_err(|_| AppError::StorageUnavailable)?;

    if entries.is_empty() {
        return Err(AppError::NoSavesFound);
    }

    for entry in entries.iter_mut() {
        if let Ok(name) = read_save_name(volume_mgr, entry) {
            if !name.is_empty() {
                entry.display_name = name;
            }
        }
    }

    entries.sort_unstable_by(|left, right| left.display_name.cmp(&right.display_name));

    Ok(entries)
}

/// Loads a TMX map file from SD card.
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
    let xml =
        core::str::from_utf8(&bytes).map_err(|_| AppError::InvalidTmx("TMX is not UTF-8"))?;
    parse_tmx(entry.display_name.as_str(), xml)
}

/// Loads a `.rpgs` save file from `/savegame`.
pub fn load_save<D>(
    volume_mgr: &VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    entry: &MapEntry,
) -> Result<GameState, AppError>
where
    D: embedded_sdmmc::BlockDevice,
{
    let volume = volume_mgr
        .open_volume(VolumeIdx(0))
        .map_err(|_| AppError::StorageUnavailable)?;
    let root_dir = volume
        .open_root_dir()
        .map_err(|_| AppError::StorageUnavailable)?;
    let save_dir = root_dir
        .open_dir(SAVE_DIR)
        .or_else(|_| root_dir.open_dir("savegame"))
        .map_err(|_| AppError::SaveDirMissing)?;
    let file = save_dir
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
    GameState::from_save_bytes(&bytes).map_err(|err| AppError::Engine(err.to_string()))
}

/// Saves the current game state into `/savegame`.
pub fn save_game<D>(
    volume_mgr: &VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    name: &str,
    state: &GameState,
) -> Result<(), AppError>
where
    D: embedded_sdmmc::BlockDevice,
{
    let volume = volume_mgr
        .open_volume(VolumeIdx(0))
        .map_err(|_| AppError::StorageUnavailable)?;
    let root_dir = volume
        .open_root_dir()
        .map_err(|_| AppError::StorageUnavailable)?;

    let save_dir = match root_dir.open_dir(SAVE_DIR).or_else(|_| root_dir.open_dir("savegame")) {
        Ok(dir) => dir,
        Err(_) => {
            root_dir
                .make_dir_in_dir(SAVE_DIR)
                .map_err(|_| AppError::StorageUnavailable)?;
            root_dir
                .open_dir(SAVE_DIR)
                .map_err(|_| AppError::StorageUnavailable)?
        }
    };

    let filename = sanitize_save_filename(name);
    let bytes = state
        .to_save_bytes_with_name(name)
        .map_err(|err| AppError::Engine(err.to_string()))?;
    let mut file = save_dir
        .open_file_in_dir(filename.as_str(), Mode::ReadWriteCreateOrTruncate)
        .map_err(|_| AppError::StorageUnavailable)?;

    file.write(&bytes)
        .map_err(|_| AppError::StorageUnavailable)?;

    Ok(())
}

/// Returns a user-facing error string.
pub fn error_message(error: AppError) -> String {
    match error {
        AppError::StorageUnavailable => "SD card is unavailable or unreadable".to_string(),
        AppError::MapsDirMissing => "Folder /maps was not found on the SD card".to_string(),
        AppError::SaveDirMissing => "Folder /savegame was not found on the SD card".to_string(),
        AppError::NoMapsFound => "No .tmx maps were found in /maps".to_string(),
        AppError::NoSavesFound => "No save files were found in /savegame".to_string(),
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
        payload: LoadedPayload::Map(map),
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

fn read_save_name<D>(
    volume_mgr: &VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    entry: &MapEntry,
) -> Result<String, AppError>
where
    D: embedded_sdmmc::BlockDevice,
{
    let volume = volume_mgr
        .open_volume(VolumeIdx(0))
        .map_err(|_| AppError::StorageUnavailable)?;
    let root_dir = volume
        .open_root_dir()
        .map_err(|_| AppError::StorageUnavailable)?;
    let save_dir = root_dir
        .open_dir(SAVE_DIR)
        .or_else(|_| root_dir.open_dir("savegame"))
        .map_err(|_| AppError::SaveDirMissing)?;
    let file = save_dir
        .open_file_in_dir(entry.short_name.as_str(), Mode::ReadOnly)
        .map_err(|_| AppError::StorageUnavailable)?;

    let mut buffer = [0u8; 128];
    let read = file
        .read(&mut buffer)
        .map_err(|_| AppError::StorageUnavailable)?;
    GameState::read_save_name(&buffer[..read]).map_err(|err| AppError::Engine(err.to_string()))
}

fn sanitize_save_filename(name: &str) -> String {
    let mut cleaned: Vec<u8> = Vec::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            cleaned.push(ch.to_ascii_uppercase() as u8);
        }
    }

    if cleaned.is_empty() {
        cleaned.extend_from_slice(b"SAVE");
    }

    if cleaned.len() > 8 {
        cleaned.truncate(8);
    }

    let base = core::str::from_utf8(&cleaned).unwrap_or("SAVE");
    format!("{base}.RPG")
}
