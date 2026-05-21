//! T-Deck host adapter around the shared embedded app controller.

use alloc::{string::String, string::ToString};

use embedded_graphics::prelude::Size;
use game::app::{AppHost, AppLayout, EmbeddedApp, LaunchConfig as SharedLaunchConfig, LoadedGame};
use game::info_overlay::InfoOverlay;
use game::list::ListEntry;
use game::render::{RenderConfig, visible_tiles};
use rpg_engine::game_state::GameState;

use crate::input::InputEvent;
use crate::storage::{self, AppError};
use crate::system_info::SystemInfoReader;

const MAP_RENDER_CONFIG: RenderConfig = RenderConfig {
    tile_width: 16,
    tile_height: 16,
    header_height: 22,
    footer_height: 12,
};

/// Compile-time launch configuration for direct map boot.
pub struct LaunchConfig {
    /// Optional map name requested through `TDECK_START_MAP`.
    pub start_map: Option<&'static str>,
    /// Initial horizontal viewport offset.
    pub start_x: usize,
    /// Initial vertical viewport offset.
    pub start_y: usize,
}

impl LaunchConfig {
    /// Reads launch configuration from compile-time environment variables.
    pub fn from_env() -> Self {
        Self {
            start_map: option_env!("TDECK_START_MAP"),
            start_x: parse_env_usize(option_env!("TDECK_VIEW_X")).unwrap_or(0),
            start_y: parse_env_usize(option_env!("TDECK_VIEW_Y")).unwrap_or(0),
        }
    }
}

struct TdeckHost<'a, 'b, 'c, D>
where
    D: embedded_sdmmc::BlockDevice,
{
    volume_mgr: &'a embedded_sdmmc::VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    system_info: &'b mut SystemInfoReader<'c>,
}

impl<'a, 'b, 'c, D> AppHost for TdeckHost<'a, 'b, 'c, D>
where
    D: embedded_sdmmc::BlockDevice,
{
    type Error = AppError;

    fn discover_maps(&mut self) -> Result<alloc::vec::Vec<ListEntry>, Self::Error> {
        let maps = storage::discover_maps(self.volume_mgr)?;
        Ok(maps
            .into_iter()
            .map(|entry| ListEntry {
                id: entry.short_name,
                label: entry.display_name,
                meta: entry.size_bytes,
            })
            .collect())
    }

    fn discover_saves(&mut self) -> Result<alloc::vec::Vec<ListEntry>, Self::Error> {
        let saves = storage::discover_saves(self.volume_mgr)?;
        Ok(saves
            .into_iter()
            .map(|entry| ListEntry {
                id: entry.short_name,
                label: entry.display_name,
                meta: entry.size_bytes,
            })
            .collect())
    }

    fn load_map(&mut self, entry: &ListEntry) -> Result<LoadedGame, Self::Error> {
        let map_entry = storage::MapEntry {
            short_name: entry.id.clone(),
            display_name: entry.label.clone(),
            size_bytes: entry.meta,
        };
        let loaded = storage::load_map(self.volume_mgr, &map_entry)?;
        Ok(LoadedGame {
            map_name: loaded.name,
            state: loaded.state,
        })
    }

    fn load_save(&mut self, entry: &ListEntry) -> Result<LoadedGame, Self::Error> {
        let map_entry = storage::MapEntry {
            short_name: entry.id.clone(),
            display_name: entry.label.clone(),
            size_bytes: entry.meta,
        };
        let state = storage::load_save(self.volume_mgr, &map_entry)?;
        Ok(LoadedGame {
            map_name: entry.label.clone(),
            state,
        })
    }

    fn save_game(&mut self, name: &str, state: &GameState) -> Result<(), Self::Error> {
        storage::save_game(self.volume_mgr, name, state)
    }

    fn info_overlay(&mut self) -> Option<InfoOverlay> {
        Some(self.system_info.snapshot().to_info_overlay())
    }

    fn error_message(&self, error: Self::Error) -> String {
        storage::error_message(error)
    }
}

/// Creates the shared embedded app for T-Deck.
pub fn initial_screen<D>(
    volume_mgr: &embedded_sdmmc::VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    launch: &LaunchConfig,
    system_info: &mut SystemInfoReader<'_>,
) -> EmbeddedApp
where
    D: embedded_sdmmc::BlockDevice,
{
    let mut host = TdeckHost {
        volume_mgr,
        system_info,
    };
    EmbeddedApp::new(
        &mut host,
        SharedLaunchConfig {
            start_map: launch.start_map.map(ToString::to_string),
            start_x: launch.start_x,
            start_y: launch.start_y,
        },
    )
}

/// Applies a single input event to the current shared app.
pub fn handle_event<D>(
    app: &mut EmbeddedApp,
    event: InputEvent,
    volume_mgr: &embedded_sdmmc::VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    system_info: &mut SystemInfoReader<'_>,
    screen_size: Size,
) -> bool
where
    D: embedded_sdmmc::BlockDevice,
{
    let layout = app_layout(screen_size);
    let mut host = TdeckHost {
        volume_mgr,
        system_info,
    };
    app.handle_input(&mut host, event, layout)
}

/// Clamps the shared current screen to the visible map viewport when needed.
pub fn clamp_to_screen(app: &mut EmbeddedApp, screen_size: Size) {
    app.clamp_view_to_layout(app_layout(screen_size));
}

/// Returns the shared list row count for the current T-Deck screen size.
pub fn selectable_rows(screen_size: Size) -> usize {
    screen_size.height.saturating_sub(32) as usize / 14
}

/// Returns the shared T-Deck render config.
pub fn render_config() -> RenderConfig {
    MAP_RENDER_CONFIG
}

fn app_layout(screen_size: Size) -> AppLayout {
    let (map_visible_cols, map_visible_rows) = visible_tiles(screen_size, MAP_RENDER_CONFIG);
    AppLayout {
        list_rows: selectable_rows(screen_size),
        save_rows: 4,
        map_visible_cols,
        map_visible_rows,
    }
}

fn parse_env_usize(value: Option<&'static str>) -> Option<usize> {
    value.and_then(|item| item.parse::<usize>().ok())
}
