//! App state machine and screen transitions.

use alloc::string::ToString;

use embedded_graphics::prelude::Size;
use rpg_engine::Direction;

use crate::input::InputEvent;
use crate::render::{map_view_tiles, selectable_rows};
use crate::screens::{InteractionMode, MapSelectScreen, MapViewScreen, Screen, SplashScreen};
use crate::session::GameSession;
use crate::storage::{self, AppError};
use crate::system_info::SystemInfoReader;

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

/// Creates the first screen shown after boot.
pub fn initial_screen<D>(
    volume_mgr: &embedded_sdmmc::VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    launch: &LaunchConfig,
) -> Screen
where
    D: embedded_sdmmc::BlockDevice,
{
    if let Some(requested_map) = launch.start_map {
        match storage::discover_maps(volume_mgr) {
            Ok(maps) => {
                if let Some(entry) = maps.iter().find(|entry| {
                    storage::names_match(&entry.display_name, requested_map)
                        || storage::names_match(&entry.short_name, requested_map)
                }) {
                    return match build_map_view(volume_mgr, entry, launch) {
                        Ok(screen) => Screen::MapView(screen),
                        Err(err) => Screen::MapSelect(MapSelectScreen {
                            maps,
                            selected: 0,
                            scroll: 0,
                            status: Some(storage::error_message(err)),
                        }),
                    };
                }

                return Screen::MapSelect(MapSelectScreen {
                    maps,
                    selected: 0,
                    scroll: 0,
                    status: Some(storage::error_message(AppError::InvalidConfiguredMap)),
                });
            }
            Err(err) => {
                return Screen::Splash(SplashScreen {
                    status: Some(storage::error_message(err)),
                });
            }
        }
    }

    Screen::Splash(SplashScreen { status: None })
}

/// Applies a single input event to the current screen.
pub fn handle_event<D>(
    screen: &mut Screen,
    event: InputEvent,
    volume_mgr: &embedded_sdmmc::VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    launch: &LaunchConfig,
    system_info: &mut SystemInfoReader<'_>,
    screen_size: Size,
) -> bool
where
    D: embedded_sdmmc::BlockDevice,
{
    match screen {
        Screen::Splash(_) => {
            let outcome = handle_splash(event, volume_mgr);
            if let Some(next_screen) = outcome.next_screen {
                *screen = next_screen;
            }
            outcome.changed
        }
        Screen::MapSelect(map_select) => {
            let outcome = handle_map_select(map_select, event, volume_mgr, launch, screen_size);
            if let Some(next_screen) = outcome.next_screen {
                *screen = next_screen;
            }
            outcome.changed
        }
        Screen::MapView(map_view) => {
            let outcome = handle_map_view(map_view, event, volume_mgr, system_info, screen_size);
            if let Some(next_screen) = outcome.next_screen {
                *screen = next_screen;
            }
            outcome.changed
        }
    }
}

struct ScreenOutcome {
    changed: bool,
    next_screen: Option<Screen>,
}

fn handle_splash<D>(
    event: InputEvent,
    volume_mgr: &embedded_sdmmc::VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
) -> ScreenOutcome
where
    D: embedded_sdmmc::BlockDevice,
{
    if matches!(event, InputEvent::Enter) {
        let next_screen = match storage::discover_maps(volume_mgr) {
            Ok(maps) => Screen::MapSelect(MapSelectScreen {
                maps,
                selected: 0,
                scroll: 0,
                status: Some("Select a map and press Enter".to_string()),
            }),
            Err(err) => Screen::Splash(SplashScreen {
                status: Some(storage::error_message(err)),
            }),
        };
        return ScreenOutcome {
            changed: true,
            next_screen: Some(next_screen),
        };
    }

    ScreenOutcome {
        changed: false,
        next_screen: None,
    }
}

fn handle_map_select<D>(
    map_select: &mut MapSelectScreen,
    event: InputEvent,
    volume_mgr: &embedded_sdmmc::VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    launch: &LaunchConfig,
    screen_size: Size,
) -> ScreenOutcome
where
    D: embedded_sdmmc::BlockDevice,
{
    if map_select.maps.is_empty() {
        if matches!(event, InputEvent::Enter) {
            return ScreenOutcome {
                changed: true,
                next_screen: Some(Screen::Splash(SplashScreen {
                    status: Some("No .tmx maps found in /maps".to_string()),
                })),
            };
        }
        return ScreenOutcome {
            changed: false,
            next_screen: None,
        };
    }

    let mut changed = false;
    let mut next_screen: Option<Screen> = None;
    match event {
        InputEvent::Info | InputEvent::Close => {}
        InputEvent::Up => {
            if map_select.selected > 0 {
                map_select.selected -= 1;
                changed = true;
            }
        }
        InputEvent::Down => {
            if map_select.selected + 1 < map_select.maps.len() {
                map_select.selected += 1;
                changed = true;
            }
        }
        InputEvent::Enter => match build_map_view(volume_mgr, &map_select.maps[map_select.selected], launch) {
            Ok(map_view) => {
                next_screen = Some(Screen::MapView(map_view));
                changed = true;
            }
            Err(err) => {
                map_select.status = Some(storage::error_message(err));
                changed = true;
            }
        },
        InputEvent::Back => {
            return ScreenOutcome {
                changed: true,
                next_screen: Some(Screen::Splash(SplashScreen { status: None })),
            };
        }
        InputEvent::Left | InputEvent::Right | InputEvent::None => {}
    }

    if next_screen.is_some() {
        return ScreenOutcome {
            changed,
            next_screen,
        };
    }

    let previous_scroll = map_select.scroll;
    let visible_rows = selectable_rows(screen_size);
    if map_select.selected < map_select.scroll {
        map_select.scroll = map_select.selected;
    } else if map_select.selected >= map_select.scroll + visible_rows {
        map_select.scroll = map_select
            .selected
            .saturating_sub(visible_rows.saturating_sub(1));
    }

    ScreenOutcome {
        changed: changed || map_select.scroll != previous_scroll,
        next_screen: None,
    }
}

fn handle_map_view<D>(
    map_view: &mut MapViewScreen,
    event: InputEvent,
    volume_mgr: &embedded_sdmmc::VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    system_info: &mut SystemInfoReader<'_>,
    screen_size: Size,
) -> ScreenOutcome
where
    D: embedded_sdmmc::BlockDevice,
{
    if map_view.info_overlay.is_some() {
        return match event {
            InputEvent::Enter | InputEvent::Close => {
                map_view.info_overlay = None;
                ScreenOutcome {
                    changed: true,
                    next_screen: None,
                }
            }
            InputEvent::None => ScreenOutcome {
                changed: false,
                next_screen: None,
            },
            _ => ScreenOutcome {
                changed: false,
                next_screen: None,
            },
        };
    }

    match event {
        InputEvent::Info => {
            map_view.info_overlay = Some(system_info.snapshot());
            return ScreenOutcome {
                changed: true,
                next_screen: None,
            };
        }
        InputEvent::Close => {
            return ScreenOutcome {
                changed: false,
                next_screen: None,
            };
        }
        InputEvent::Enter => {
            map_view.mode = match map_view.mode {
                InteractionMode::Pan => InteractionMode::Hero,
                InteractionMode::Hero => InteractionMode::Pan,
            };
            map_view.status = Some(match map_view.mode {
                InteractionMode::Pan => "Pan mode: arrows move the viewport".to_string(),
                InteractionMode::Hero => "Hero mode: arrows move the selected hero".to_string(),
            });
            return ScreenOutcome {
                changed: true,
                next_screen: None,
            };
        }
        InputEvent::Back => {
            let next_screen = match storage::discover_maps(volume_mgr) {
                Ok(maps) => Screen::MapSelect(MapSelectScreen {
                    selected: maps
                        .iter()
                        .position(|entry| entry.display_name == map_view.session.map_name())
                        .unwrap_or(0),
                    scroll: 0,
                    maps,
                    status: Some("Returned to map selection".to_string()),
                }),
                Err(err) => Screen::Splash(SplashScreen {
                    status: Some(storage::error_message(err)),
                }),
            };
            return ScreenOutcome {
                changed: true,
                next_screen: Some(next_screen),
            };
        }
        InputEvent::None => {
            return ScreenOutcome {
                changed: false,
                next_screen: None,
            };
        }
        InputEvent::Up | InputEvent::Down | InputEvent::Left | InputEvent::Right => {}
    }

    let changed = match map_view.mode {
        InteractionMode::Pan => pan_view(map_view, event, screen_size),
        InteractionMode::Hero => move_hero_or_report(map_view, event, screen_size),
    };

    ScreenOutcome {
        changed,
        next_screen: None,
    }
}

fn build_map_view<D>(
    volume_mgr: &embedded_sdmmc::VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    entry: &crate::storage::MapEntry,
    launch: &LaunchConfig,
) -> Result<MapViewScreen, AppError>
where
    D: embedded_sdmmc::BlockDevice,
{
    let loaded = storage::load_map(volume_mgr, entry)?;
    let session =
        GameSession::new(loaded.name, loaded.map).map_err(|err| AppError::Engine(err.to_string()))?;

    Ok(MapViewScreen {
        session,
        view_x: launch.start_x,
        view_y: launch.start_y,
        mode: InteractionMode::Pan,
        status: Some("Map loaded. Press Enter to switch between pan and hero mode".to_string()),
        info_overlay: None,
    })
}

fn pan_view(map_view: &mut MapViewScreen, event: InputEvent, screen_size: Size) -> bool {
    let (visible_cols, visible_rows) = map_view_tiles(screen_size);
    let map = &map_view.session.state().map;
    let max_x = map.tile_width().saturating_sub(visible_cols as u32) as usize;
    let max_y = map.tile_height().saturating_sub(visible_rows as u32) as usize;

    let previous_x = map_view.view_x;
    let previous_y = map_view.view_y;
    match event {
        InputEvent::Info | InputEvent::Close => {}
        InputEvent::Up => map_view.view_y = map_view.view_y.saturating_sub(1),
        InputEvent::Down => map_view.view_y = (map_view.view_y + 1).min(max_y),
        InputEvent::Left => map_view.view_x = map_view.view_x.saturating_sub(1),
        InputEvent::Right => map_view.view_x = (map_view.view_x + 1).min(max_x),
        InputEvent::Enter | InputEvent::Back | InputEvent::None => {}
    }

    map_view.view_x != previous_x || map_view.view_y != previous_y
}

fn move_hero_or_report(map_view: &mut MapViewScreen, event: InputEvent, screen_size: Size) -> bool {
    let direction = match event {
        InputEvent::Info | InputEvent::Close => None,
        InputEvent::Up => Some(Direction::North),
        InputEvent::Down => Some(Direction::South),
        InputEvent::Left => Some(Direction::West),
        InputEvent::Right => Some(Direction::East),
        InputEvent::Enter | InputEvent::Back | InputEvent::None => None,
    };

    let Some(direction) = direction else {
        return false;
    };

    match map_view.session.move_selected_hero(direction) {
        Ok(position) => {
            map_view.status = Some(map_view.session.summary());
            keep_hero_visible(
                map_view,
                position.x as usize,
                position.y as usize,
                screen_size,
            )
        }
        Err(err) => {
            map_view.status = Some(err.to_string());
            true
        }
    }
}

fn keep_hero_visible(
    map_view: &mut MapViewScreen,
    hero_x: usize,
    hero_y: usize,
    screen_size: Size,
) -> bool {
    let (visible_cols, visible_rows) = map_view_tiles(screen_size);
    let old_x = map_view.view_x;
    let old_y = map_view.view_y;

    if hero_x < map_view.view_x {
        map_view.view_x = hero_x;
    } else if hero_x >= map_view.view_x + visible_cols {
        map_view.view_x = hero_x.saturating_sub(visible_cols.saturating_sub(1));
    }

    if hero_y < map_view.view_y {
        map_view.view_y = hero_y;
    } else if hero_y >= map_view.view_y + visible_rows {
        map_view.view_y = hero_y.saturating_sub(visible_rows.saturating_sub(1));
    }

    let _ = (old_x, old_y);
    true
}

fn parse_env_usize(value: Option<&'static str>) -> Option<usize> {
    value.and_then(|item| item.parse::<usize>().ok())
}
