//! App state machine and screen transitions.

use alloc::{boxed::Box, format, string::{String, ToString}};

use embedded_graphics::prelude::Size;
use rpg_embedded::list::{ListEntry, ListOutcome, ListScreen};
use rpg_embedded::map_view::{MapViewApp, MapViewOutcome};
use rpg_embedded::render::{RenderConfig, visible_tiles};
use rpg_embedded::session::GameSession;
use rpg_embedded::splash::{SplashOutcome, SplashScreen};

use crate::input::InputEvent;
use crate::render::selectable_rows;
use crate::screens::{MapViewScreen, SaveOverlay, Screen};
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
                        Ok(screen) => Screen::MapView(Box::new(screen)),
                        Err(err) => Screen::MapSelect(make_list_screen(
                            &maps,
                            Some(storage::error_message(err)),
                        )),
                    };
                }

                return Screen::MapSelect(make_list_screen(
                    &maps,
                    Some(storage::error_message(AppError::InvalidConfiguredMap)),
                ));
            }
            Err(err) => {
                return Screen::Splash(SplashScreen::new(
                    0,
                    Some(storage::error_message(err)),
                ));
            }
        }
    }

    Screen::Splash(SplashScreen::new(0, None))
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
            let outcome =
                handle_map_view(map_view.as_mut(), event, volume_mgr, system_info, screen_size);
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
    match splash.handle_input(event, 2) {
        SplashOutcome::Changed => ScreenOutcome {
            changed: true,
            next_screen: None,
        },
        SplashOutcome::Selected(selected) => {
            let next_screen = match selected {
                0 => match storage::discover_maps(volume_mgr) {
                    Ok(maps) => Screen::MapSelect(make_list_screen(
                        &maps,
                        Some("Select a map and press Enter".to_string()),
                    )),
                    Err(err) => {
                        splash.status = Some(storage::error_message(err));
                        Screen::Splash(SplashScreen::new(
                            splash.selected,
                            splash.status.clone(),
                        ))
                    }
                },
                _ => match storage::discover_saves(volume_mgr) {
                    Ok(saves) => Screen::SaveSelect(make_list_screen(
                        &saves,
                        Some("Select a save and press Enter".to_string()),
                    )),
                    Err(err) => {
                        splash.status = Some(storage::error_message(err));
                        Screen::Splash(SplashScreen::new(
                            splash.selected,
                            splash.status.clone(),
                        ))
                    }
                },
            };
            ScreenOutcome {
                changed: true,
                next_screen: Some(next_screen),
            }
        }
        SplashOutcome::BackRequested | SplashOutcome::NoChange => ScreenOutcome {
            changed: false,
            next_screen: None,
        },
    }
}

fn handle_map_select<D>(
    map_select: &mut ListScreen,
    event: InputEvent,
    volume_mgr: &embedded_sdmmc::VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    launch: &LaunchConfig,
    screen_size: Size,
) -> ScreenOutcome
where
    D: embedded_sdmmc::BlockDevice,
{
    if map_select.entries.is_empty() {
        if matches!(event, InputEvent::Enter) {
            return ScreenOutcome {
                changed: true,
                next_screen: Some(Screen::Splash(SplashScreen::new(
                    0,
                    Some("No .rpgs maps found in /maps".to_string()),
                ))),
            };
        }
        return ScreenOutcome {
            changed: false,
            next_screen: None,
        };
    }

    match map_select.handle_input(event, selectable_rows(screen_size)) {
        ListOutcome::NoChange => ScreenOutcome {
            changed: false,
            next_screen: None,
        },
        ListOutcome::Changed => ScreenOutcome {
            changed: true,
            next_screen: None,
        },
        ListOutcome::BackRequested => ScreenOutcome {
            changed: true,
            next_screen: Some(Screen::Splash(SplashScreen::new(0, None))),
        },
        ListOutcome::Selected(selected) => match storage::discover_maps(volume_mgr) {
            Ok(maps) => match build_map_view(volume_mgr, &maps[selected], launch) {
                Ok(map_view) => ScreenOutcome {
                    changed: true,
                    next_screen: Some(Screen::MapView(Box::new(map_view))),
                },
                Err(err) => {
                    map_select.status = Some(storage::error_message(err));
                    ScreenOutcome {
                        changed: true,
                        next_screen: None,
                    }
                }
            },
            Err(err) => {
                map_select.status = Some(storage::error_message(err));
                ScreenOutcome {
                    changed: true,
                    next_screen: None,
                }
            }
        },
    }
}

fn handle_save_select<D>(
    save_select: &mut ListScreen,
    event: InputEvent,
    volume_mgr: &embedded_sdmmc::VolumeManager<D, crate::DummyTimesource, 4, 4, 1>,
    screen_size: Size,
) -> ScreenOutcome
where
    D: embedded_sdmmc::BlockDevice,
{
    if save_select.entries.is_empty() {
        if matches!(event, InputEvent::Enter) {
            return ScreenOutcome {
                changed: true,
                next_screen: Some(Screen::Splash(SplashScreen::new(
                    1,
                    Some("No save files found in /savegame".to_string()),
                ))),
            };
        }
        return ScreenOutcome {
            changed: false,
            next_screen: None,
        };
    }

    match save_select.handle_input(event, selectable_rows(screen_size)) {
        ListOutcome::NoChange => ScreenOutcome {
            changed: false,
            next_screen: None,
        },
        ListOutcome::Changed => ScreenOutcome {
            changed: true,
            next_screen: None,
        },
        ListOutcome::BackRequested => ScreenOutcome {
            changed: true,
            next_screen: Some(Screen::Splash(SplashScreen::new(1, None))),
        },
        ListOutcome::Selected(selected) => match storage::discover_saves(volume_mgr) {
            Ok(saves) => {
                let entry = &saves[selected];
                match storage::load_save(volume_mgr, entry) {
                    Ok(state) => match GameSession::from_state(entry.display_name.clone(), state) {
                        Ok(session) => {
                            let mut map_view = MapViewScreen {
                                app: MapViewApp::new(
                                    session,
                                    0,
                                    0,
                                    Some("Save loaded".to_string()),
                                ),
                                status: Some("Save loaded".to_string()),
                                info_overlay: None,
                                save_overlay: None,
                            };
                            clamp_view_to_map(&mut map_view, screen_size);
                            ScreenOutcome {
                                changed: true,
                                next_screen: Some(Screen::MapView(Box::new(map_view))),
                            }
                        }
                        Err(err) => {
                            save_select.status = Some(err.to_string());
                            ScreenOutcome {
                                changed: true,
                                next_screen: None,
                            }
                        }
                    },
                    Err(err) => {
                        save_select.status = Some(storage::error_message(err));
                        ScreenOutcome {
                            changed: true,
                            next_screen: None,
                        }
                    }
                }
            }
            Err(err) => {
                save_select.status = Some(storage::error_message(err));
                ScreenOutcome {
                    changed: true,
                    next_screen: None,
                }
            }
        },
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

    let (visible_cols, visible_rows) = visible_tiles(screen_size, MAP_RENDER_CONFIG);
    match map_view.app.handle_input(event, visible_cols, visible_rows) {
        MapViewOutcome::NoChange => ScreenOutcome {
            changed: false,
            next_screen: None,
        },
        MapViewOutcome::Changed => {
            map_view.status = map_view.app.status().map(ToString::to_string);
            ScreenOutcome {
                changed: true,
                next_screen: None,
            }
        }
        MapViewOutcome::BackRequested => {
            let next_screen = match storage::discover_maps(volume_mgr) {
                Ok(maps) => {
                    let mut screen =
                        make_list_screen(&maps, Some("Returned to map selection".to_string()));
                    screen.selected = maps
                        .iter()
                        .position(|entry| entry.display_name == map_view.app.session().map_name())
                        .unwrap_or(0);
                    screen.scroll = screen.selected;
                    Screen::MapSelect(screen)
                }
                Err(err) => {
                    Screen::Splash(SplashScreen::new(0, Some(storage::error_message(err))))
                }
            };
            ScreenOutcome {
                changed: true,
                next_screen: Some(next_screen),
            }
        }
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
    let session = GameSession::from_state(loaded.name, loaded.state)
        .map_err(|err| AppError::Engine(err.to_string()))?;

    Ok(MapViewScreen {
        app: MapViewApp::new(
            session,
            launch.start_x,
            launch.start_y,
            Some("Map loaded. Press Enter to switch between pan and hero mode".to_string()),
        ),
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
        SaveOverlay::Menu { mut selected, status } => {
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
        SaveOverlay::SaveName { mut name, status } => {
            const MAX_NAME_LEN: usize = 24;
            let mut status = status;
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
                        match storage::save_game(volume_mgr, trimmed, map_view.app.session().state()) {
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
            saves,
            mut selected,
            mut scroll,
            status,
        } => {
            let mut status = status;
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
                                        map_view.app = MapViewApp::new(
                                            session,
                                            map_view.app.view_x(),
                                            map_view.app.view_y(),
                                            Some("Save loaded".to_string()),
                                        );
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
    let (visible_cols, visible_rows) = visible_tiles(screen_size, MAP_RENDER_CONFIG);
    map_view.app.clamp_view_to_map(visible_cols, visible_rows);
}

fn normalize_save_char(ch: char) -> Option<char> {
    if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ' ' {
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

fn parse_env_usize(value: Option<&'static str>) -> Option<usize> {
    value.and_then(|item| item.parse::<usize>().ok())
}

fn make_list_screen(
    entries: &[crate::storage::MapEntry],
    status: Option<String>,
) -> ListScreen {
    let list_entries = entries
        .iter()
        .map(|entry| ListEntry {
            label: entry.display_name.clone(),
            meta: entry.size_bytes,
        })
        .collect();
    ListScreen::new(list_entries, status)
}
