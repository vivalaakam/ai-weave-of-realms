//! App state machine and screen transitions.

use alloc::{format, string::{String, ToString}};

use embedded_graphics::prelude::Size;
use rpg_engine::Direction;

use crate::input::InputEvent;
use crate::render::{map_view_tiles, selectable_rows};
use crate::screens::{InteractionMode, MapSelectScreen, MapViewScreen, SaveOverlay, Screen, SplashScreen};
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
                    selected: 0,
                    status: Some(storage::error_message(err)),
                });
            }
        }
    }

    Screen::Splash(SplashScreen {
        selected: 0,
        status: None,
    })
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
        Screen::Splash(splash) => {
            let outcome = handle_splash(splash, event, volume_mgr);
            if let Some(next_screen) = outcome.next_screen {
                *screen = next_screen;
            }
            outcome.changed
        }
        Screen::SaveSelect(save_select) => {
            let outcome = handle_save_select(save_select, event, volume_mgr, screen_size);
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
    splash: &mut SplashScreen,
    event: InputEvent,
    volume_mgr: &embedded_sdmmc::VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
) -> ScreenOutcome
where
    D: embedded_sdmmc::BlockDevice,
{
    splash.status = None;
    let event = map_select_event(event);
    match event {
        InputEvent::Up => {
            splash.selected = splash.selected.saturating_sub(1);
            return ScreenOutcome {
                changed: true,
                next_screen: None,
            };
        }
        InputEvent::Down => {
            splash.selected = (splash.selected + 1).min(1);
            return ScreenOutcome {
                changed: true,
                next_screen: None,
            };
        }
        InputEvent::Enter => {
            let next_screen = match splash.selected {
                0 => match storage::discover_maps(volume_mgr) {
                    Ok(maps) => Screen::MapSelect(MapSelectScreen {
                        maps,
                        selected: 0,
                        scroll: 0,
                        status: Some("Select a map and press Enter".to_string()),
                    }),
                    Err(err) => {
                        splash.status = Some(storage::error_message(err));
                        Screen::Splash(SplashScreen {
                            selected: splash.selected,
                            status: splash.status.clone(),
                        })
                    }
                },
                _ => match storage::discover_saves(volume_mgr) {
                    Ok(saves) => Screen::SaveSelect(MapSelectScreen {
                        maps: saves,
                        selected: 0,
                        scroll: 0,
                        status: Some("Select a save and press Enter".to_string()),
                    }),
                    Err(err) => {
                        splash.status = Some(storage::error_message(err));
                        Screen::Splash(SplashScreen {
                            selected: splash.selected,
                            status: splash.status.clone(),
                        })
                    }
                },
            };
            return ScreenOutcome {
                changed: true,
                next_screen: Some(next_screen),
            };
        }
        _ => {}
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
                    selected: 0,
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
    let event = map_select_event(event);
    match event {
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
                next_screen: Some(Screen::Splash(SplashScreen {
                    selected: 0,
                    status: None,
                })),
            };
        }
        InputEvent::Left | InputEvent::Right | InputEvent::None | InputEvent::Key(_) => {}
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

fn handle_save_select<D>(
    save_select: &mut MapSelectScreen,
    event: InputEvent,
    volume_mgr: &embedded_sdmmc::VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    screen_size: Size,
) -> ScreenOutcome
where
    D: embedded_sdmmc::BlockDevice,
{
    if save_select.maps.is_empty() {
        if matches!(event, InputEvent::Enter) {
            return ScreenOutcome {
                changed: true,
                next_screen: Some(Screen::Splash(SplashScreen {
                    selected: 1,
                    status: Some("No save files found in /savegame".to_string()),
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
    let event = map_select_event(event);
    match event {
        InputEvent::Up => {
            if save_select.selected > 0 {
                save_select.selected -= 1;
                changed = true;
            }
        }
        InputEvent::Down => {
            if save_select.selected + 1 < save_select.maps.len() {
                save_select.selected += 1;
                changed = true;
            }
        }
        InputEvent::Enter => {
            let entry = &save_select.maps[save_select.selected];
            match storage::load_save(volume_mgr, entry) {
                Ok(state) => match GameSession::from_state(entry.display_name.clone(), state) {
                    Ok(session) => {
                        let mut map_view = MapViewScreen {
                            session,
                            view_x: 0,
                            view_y: 0,
                            mode: InteractionMode::Pan,
                            status: Some("Save loaded".to_string()),
                            info_overlay: None,
                            save_overlay: None,
                        };
                        clamp_view_to_map(&mut map_view, screen_size);
                        next_screen = Some(Screen::MapView(map_view));
                        changed = true;
                    }
                    Err(err) => {
                        save_select.status = Some(err.to_string());
                        changed = true;
                    }
                },
                Err(err) => {
                    save_select.status = Some(storage::error_message(err));
                    changed = true;
                }
            }
        }
        InputEvent::Back => {
            return ScreenOutcome {
                changed: true,
                next_screen: Some(Screen::Splash(SplashScreen {
                    selected: 1,
                    status: None,
                })),
            };
        }
        InputEvent::Left | InputEvent::Right | InputEvent::None | InputEvent::Key(_) => {}
    }

    let previous_scroll = save_select.scroll;
    let visible_rows = selectable_rows(screen_size);
    if save_select.selected < save_select.scroll {
        save_select.scroll = save_select.selected;
    } else if save_select.selected >= save_select.scroll + visible_rows {
        save_select.scroll = save_select
            .selected
            .saturating_sub(visible_rows.saturating_sub(1));
    }

    ScreenOutcome {
        changed: changed || save_select.scroll != previous_scroll,
        next_screen,
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
    if map_view.save_overlay.is_some() {
        return handle_save_overlay(map_view, event, volume_mgr, screen_size);
    }

    if map_view.info_overlay.is_some() {
        if matches!(event, InputEvent::Enter | InputEvent::Back) || is_key(event, 'q') {
            map_view.info_overlay = None;
            return ScreenOutcome {
                changed: true,
                next_screen: None,
            };
        }
        return ScreenOutcome {
            changed: false,
            next_screen: None,
        };
    }

    if is_key(event, 'i') {
        map_view.info_overlay = Some(system_info.snapshot());
        return ScreenOutcome {
            changed: true,
            next_screen: None,
        };
    }

    if is_key(event, 'p') {
        map_view.save_overlay = Some(SaveOverlay::Menu {
            selected: 0,
            status: None,
        });
        return ScreenOutcome {
            changed: true,
            next_screen: None,
        };
    }

    let event = map_view_event(event);
    match event {
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
                    selected: 0,
                    status: Some(storage::error_message(err)),
                }),
            };
            return ScreenOutcome {
                changed: true,
                next_screen: Some(next_screen),
            };
        }
        InputEvent::None | InputEvent::Key(_) => {
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
    let session = match loaded.payload {
        crate::storage::LoadedPayload::Map(map) => {
            GameSession::new(loaded.name, map).map_err(|err| AppError::Engine(err.to_string()))?
        }
        crate::storage::LoadedPayload::Save(state) => GameSession::from_state(loaded.name, state)
            .map_err(|err| AppError::Engine(err.to_string()))?,
    };

    Ok(MapViewScreen {
        session,
        view_x: launch.start_x,
        view_y: launch.start_y,
        mode: InteractionMode::Pan,
        status: Some("Map loaded. Press Enter to switch between pan and hero mode".to_string()),
        info_overlay: None,
        save_overlay: None,
    })
}

fn handle_save_overlay<D>(
    map_view: &mut MapViewScreen,
    event: InputEvent,
    volume_mgr: &embedded_sdmmc::VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    screen_size: Size,
) -> ScreenOutcome
where
    D: embedded_sdmmc::BlockDevice,
{
    let overlay = map_view.save_overlay.take();
    let Some(overlay) = overlay else {
        return ScreenOutcome {
            changed: false,
            next_screen: None,
        };
    };

    let outcome = match overlay {
        SaveOverlay::Menu { mut selected, mut status } => {
            status = None;
            match menu_event(event) {
                InputEvent::Up => {
                    selected = selected.saturating_sub(1);
                    (Some(SaveOverlay::Menu { selected, status }), true)
                }
                InputEvent::Down => {
                    selected = (selected + 1).min(2);
                    (Some(SaveOverlay::Menu { selected, status }), true)
                }
                InputEvent::Enter => match selected {
                    0 => (
                        Some(SaveOverlay::SaveName {
                            name: String::new(),
                            status: None,
                        }),
                        true,
                    ),
                    1 => match storage::discover_saves(volume_mgr) {
                        Ok(saves) => (
                            Some(SaveOverlay::LoadList {
                                saves,
                                selected: 0,
                                scroll: 0,
                                status: None,
                            }),
                            true,
                        ),
                        Err(err) => (
                            Some(SaveOverlay::Menu {
                                selected,
                                status: Some(storage::error_message(err)),
                            }),
                            true,
                        ),
                    },
                    _ => (None, true),
                },
                InputEvent::Back => (None, true),
                _ => (Some(SaveOverlay::Menu { selected, status }), false),
            }
        }
        SaveOverlay::SaveName { mut name, mut status } => {
            const MAX_NAME_LEN: usize = 24;
            status = None;
            match event {
                InputEvent::Key(ch) => {
                    if name.len() < MAX_NAME_LEN {
                        if let Some(mapped) = normalize_save_char(ch) {
                            name.push(mapped);
                        } else {
                            status = Some("Allowed: A-Z 0-9 _ - space".to_string());
                        }
                    } else {
                        status = Some("Name is too long".to_string());
                    }
                    (Some(SaveOverlay::SaveName { name, status }), true)
                }
                InputEvent::Back => {
                    if name.pop().is_some() {
                        (Some(SaveOverlay::SaveName { name, status }), true)
                    } else {
                        (
                            Some(SaveOverlay::Menu {
                                selected: 0,
                                status: None,
                            }),
                            true,
                        )
                    }
                }
                InputEvent::Enter => {
                    let trimmed = name.trim();
                    if trimmed.is_empty() {
                        status = Some("Enter a save name".to_string());
                        (Some(SaveOverlay::SaveName { name, status }), true)
                    } else {
                        match storage::save_game(volume_mgr, trimmed, map_view.session.state()) {
                            Ok(_) => {
                                map_view.status =
                                    Some(format!("Saved game: {trimmed}"));
                                (None, true)
                            }
                            Err(err) => {
                                status = Some(storage::error_message(err));
                                (Some(SaveOverlay::SaveName { name, status }), true)
                            }
                        }
                    }
                }
                _ => (Some(SaveOverlay::SaveName { name, status }), false),
            }
        }
        SaveOverlay::LoadList {
            mut saves,
            mut selected,
            mut scroll,
            mut status,
        } => {
            status = None;
            let visible_rows = save_list_rows(screen_size);
            match menu_event(event) {
                InputEvent::Up => {
                    selected = selected.saturating_sub(1);
                }
                InputEvent::Down => {
                    if selected + 1 < saves.len() {
                        selected += 1;
                    }
                }
                InputEvent::Enter => {
                    if let Some(entry) = saves.get(selected) {
                        match storage::load_save(volume_mgr, entry) {
                            Ok(state) => {
                                let map_name = entry.display_name.clone();
                                match GameSession::from_state(map_name, state) {
                                    Ok(session) => {
                                        map_view.session = session;
                                        map_view.mode = InteractionMode::Pan;
                                        clamp_view_to_map(map_view, screen_size);
                                        map_view.status = Some("Save loaded".to_string());
                                        return ScreenOutcome {
                                            changed: true,
                                            next_screen: None,
                                        };
                                    }
                                    Err(err) => {
                                        status = Some(err.to_string());
                                    }
                                }
                            }
                            Err(err) => status = Some(storage::error_message(err)),
                        }
                    }
                }
                InputEvent::Back => {
                    map_view.save_overlay = Some(SaveOverlay::Menu {
                        selected: 1,
                        status: None,
                    });
                    return ScreenOutcome {
                        changed: true,
                        next_screen: None,
                    };
                }
                _ => {}
            }

            if selected < scroll {
                scroll = selected;
            } else if selected >= scroll + visible_rows {
                scroll = selected.saturating_sub(visible_rows.saturating_sub(1));
            }

            (
                Some(SaveOverlay::LoadList {
                    saves,
                    selected,
                    scroll,
                    status,
                }),
                true,
            )
        }
    };

    map_view.save_overlay = outcome.0;
    ScreenOutcome {
        changed: outcome.1,
        next_screen: None,
    }
}

fn save_list_rows(screen_size: Size) -> usize {
    let _ = screen_size;
    4
}

fn clamp_view_to_map(map_view: &mut MapViewScreen, screen_size: Size) {
    let (visible_cols, visible_rows) = map_view_tiles(screen_size);
    let map = &map_view.session.state().map;
    let max_x = map
        .tile_width()
        .saturating_sub(visible_cols as u32) as usize;
    let max_y = map
        .tile_height()
        .saturating_sub(visible_rows as u32) as usize;
    map_view.view_x = map_view.view_x.min(max_x);
    map_view.view_y = map_view.view_y.min(max_y);
}

fn normalize_save_char(ch: char) -> Option<char> {
    if ch.is_ascii_alphanumeric() {
        Some(ch)
    } else if ch == '_' || ch == '-' || ch == ' ' {
        Some(ch)
    } else {
        None
    }
}

fn is_key(event: InputEvent, key: char) -> bool {
    match event {
        InputEvent::Key(ch) => ch.eq_ignore_ascii_case(&key),
        _ => false,
    }
}

fn map_select_event(event: InputEvent) -> InputEvent {
    match event {
        InputEvent::Key(ch) => match ch.to_ascii_lowercase() {
            'w' | 'k' => InputEvent::Up,
            's' | 'j' => InputEvent::Down,
            'q' => InputEvent::Back,
            _ => InputEvent::None,
        },
        other => other,
    }
}

fn map_view_event(event: InputEvent) -> InputEvent {
    match event {
        InputEvent::Key(ch) => match ch.to_ascii_lowercase() {
            'w' | 'k' => InputEvent::Up,
            's' | 'j' => InputEvent::Down,
            'a' | 'h' => InputEvent::Left,
            'd' | 'l' => InputEvent::Right,
            'q' => InputEvent::Back,
            _ => InputEvent::None,
        },
        other => other,
    }
}

fn menu_event(event: InputEvent) -> InputEvent {
    match event {
        InputEvent::Key(ch) => match ch.to_ascii_lowercase() {
            'w' | 'k' => InputEvent::Up,
            's' | 'j' => InputEvent::Down,
            'q' => InputEvent::Back,
            _ => InputEvent::None,
        },
        other => other,
    }
}

fn pan_view(map_view: &mut MapViewScreen, event: InputEvent, screen_size: Size) -> bool {
    let (visible_cols, visible_rows) = map_view_tiles(screen_size);
    let map = &map_view.session.state().map;
    let max_x = map.tile_width().saturating_sub(visible_cols as u32) as usize;
    let max_y = map.tile_height().saturating_sub(visible_rows as u32) as usize;

    let previous_x = map_view.view_x;
    let previous_y = map_view.view_y;
    match event {
        InputEvent::Key(_) | InputEvent::None => {}
        InputEvent::Up => map_view.view_y = map_view.view_y.saturating_sub(1),
        InputEvent::Down => map_view.view_y = (map_view.view_y + 1).min(max_y),
        InputEvent::Left => map_view.view_x = map_view.view_x.saturating_sub(1),
        InputEvent::Right => map_view.view_x = (map_view.view_x + 1).min(max_x),
        InputEvent::Enter | InputEvent::Back => {}
    }

    map_view.view_x != previous_x || map_view.view_y != previous_y
}

fn move_hero_or_report(map_view: &mut MapViewScreen, event: InputEvent, screen_size: Size) -> bool {
    let direction = match event {
        InputEvent::Key(_) | InputEvent::None => None,
        InputEvent::Up => Some(Direction::North),
        InputEvent::Down => Some(Direction::South),
        InputEvent::Left => Some(Direction::West),
        InputEvent::Right => Some(Direction::East),
        InputEvent::Enter | InputEvent::Back => None,
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
