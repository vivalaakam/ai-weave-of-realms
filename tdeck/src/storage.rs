//! SD-card map discovery and save loading.

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use embedded_sdmmc::{LfnBuffer, Mode, VolumeIdx, VolumeManager};
use rpg_engine::game_state::GameState;

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
    /// Full engine save state.
    pub state: GameState,
}

/// App-level error type for storage and engine session setup.
pub enum AppError {
    /// SD card could not be opened or read.
    StorageUnavailable,
    /// `/maps` or `/MAPS` directory was not found.
    MapsDirMissing,
    /// `/savegame` directory was not found.
    SaveDirMissing,
    /// No `.rpgs` files were found in `/maps`.
    NoMapsFound,
    /// No save files were found.
    NoSavesFound,
    /// Compile-time requested map is not present on the SD card.
    InvalidConfiguredMap,
    /// The parsed map could not be accepted by `rpg-engine`.
    Engine(String),
}

/// Discovers all `.rpgs` files under `/maps`.
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

            if has_rpgs_extension(&display_name) || has_rpgs_extension(&short_name) {
                entries.push(MapEntry {
                    short_name,
                    display_name,
                    size_bytes: dir_entry.size,
                });
            }
        })
        .map_err(|_| AppError::StorageUnavailable)?;

    if entries.is_empty() {
        return Err(AppError::NoMapsFound);
    }

    for entry in entries.iter_mut() {
        if let Ok(name) = read_save_name_in_dir(volume_mgr, MAPS_DIR, entry) {
            if !name.is_empty() {
                entry.display_name = name;
            }
        }
    }

    entries.sort_unstable_by(|left, right| left.display_name.cmp(&right.display_name));

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

            if has_rpgs_extension(&display_name) || has_rpgs_extension(&short_name) {
                entries.push(MapEntry {
                    short_name,
                    display_name,
                    size_bytes: dir_entry.size,
                });
            }
        })
        .map_err(|_| AppError::StorageUnavailable)?;

    if entries.is_empty() {
        return Err(AppError::NoSavesFound);
    }

    for entry in entries.iter_mut() {
        if let Ok(name) = read_save_name_in_dir(volume_mgr, SAVE_DIR, entry) {
            if !name.is_empty() {
                entry.display_name = name;
            }
        }
    }

    entries.sort_unstable_by(|left, right| left.display_name.cmp(&right.display_name));

    Ok(entries)
}

/// Loads a `.rpgs` map save file from SD card.
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
    let state =
        GameState::from_save_bytes(&bytes).map_err(|err| AppError::Engine(err.to_string()))?;
    Ok(LoadedMap {
        name: entry.display_name.to_string(),
        state,
    })
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
    let file = save_dir
        .open_file_in_dir(filename.as_str(), Mode::ReadWriteCreateOrTruncate)
        .map_err(|_| AppError::StorageUnavailable)?;

    file.write(&bytes)
        .map_err(|_| AppError::StorageUnavailable)?;

    verify_saved_file(volume_mgr, SAVE_DIR, filename.as_str(), &bytes)?;

    Ok(())
}

/// Returns a user-facing error string.
pub fn error_message(error: AppError) -> String {
    match error {
        AppError::StorageUnavailable => "SD card is unavailable or unreadable".to_string(),
        AppError::MapsDirMissing => "Folder /maps was not found on the SD card".to_string(),
        AppError::SaveDirMissing => "Folder /savegame was not found on the SD card".to_string(),
        AppError::NoMapsFound => "No .rpgs maps were found in /maps".to_string(),
        AppError::NoSavesFound => "No save files were found in /savegame".to_string(),
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

fn read_save_name_in_dir<D>(
    volume_mgr: &VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    dir_name: &str,
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
    let dir = root_dir
        .open_dir(dir_name)
        .or_else(|_| root_dir.open_dir(dir_name.to_ascii_lowercase().as_str()))
        .map_err(|_| AppError::StorageUnavailable)?;
    let file = dir
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

    if cleaned.len() > 7 {
        cleaned.truncate(7);
    }

    let base = core::str::from_utf8(&cleaned).unwrap_or("SAVE");
    format!("{base}.RPGS")
}

fn has_rpgs_extension(name: &str) -> bool {
    let lower = name.as_bytes();
    lower.len() >= 5
        && lower[lower.len() - 5].eq_ignore_ascii_case(&b'.')
        && lower[lower.len() - 4].eq_ignore_ascii_case(&b'r')
        && lower[lower.len() - 3].eq_ignore_ascii_case(&b'p')
        && lower[lower.len() - 2].eq_ignore_ascii_case(&b'g')
        && lower[lower.len() - 1].eq_ignore_ascii_case(&b's')
}

fn verify_saved_file<D>(
    volume_mgr: &VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    dir_name: &str,
    file_name: &str,
    expected: &[u8],
) -> Result<(), AppError>
where
    D: embedded_sdmmc::BlockDevice,
{
    let expected_hash = hash_bytes(expected);

    let volume = volume_mgr
        .open_volume(VolumeIdx(0))
        .map_err(|_| AppError::StorageUnavailable)?;
    let root_dir = volume
        .open_root_dir()
        .map_err(|_| AppError::StorageUnavailable)?;
    let dir = root_dir
        .open_dir(dir_name)
        .or_else(|_| root_dir.open_dir(dir_name.to_ascii_lowercase().as_str()))
        .map_err(|_| AppError::StorageUnavailable)?;
    let file = dir
        .open_file_in_dir(file_name, Mode::ReadOnly)
        .map_err(|_| AppError::StorageUnavailable)?;

    let file_len = file.length() as usize;
    if file_len != expected.len() {
        return Err(AppError::StorageUnavailable);
    }

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

    if bytes.len() != expected.len() || hash_bytes(&bytes) != expected_hash {
        return Err(AppError::StorageUnavailable);
    }

    Ok(())
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001b3;

    let mut hash = FNV_OFFSET;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
