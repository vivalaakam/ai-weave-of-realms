//! Shared top-level application state machine for embedded frontends.

use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec::Vec,
};

use rpg_engine::game_state::GameState;

use crate::info_overlay::{InfoOverlay, InfoOverlayOutcome};
use crate::input::InputEvent;
use crate::list::{ListEntry, ListOutcome, ListScreen};
use crate::map_view::{MapViewApp, MapViewOutcome};
use crate::save_overlay::{SaveOverlay, SaveOverlayOutcome};
use crate::session::GameSession;
use crate::splash::{SplashOutcome, SplashScreen};

/// Shared app title rendered on the splash screen.
pub const APP_TITLE: &str = "weave of realms";
/// Shared splash menu labels.
pub const SPLASH_OPTIONS: [&str; 2] = ["New Game", "Load Game"];
/// Shared splash footer hint.
pub const SPLASH_FOOTER: &str = "Enter: select  W/S: move";
/// Shared map list title.
pub const MAP_LIST_TITLE: &str = "Maps";
/// Shared map list footer hint.
pub const MAP_LIST_FOOTER: &str = "Up/Down: select  Enter: load  Back: splash";
/// Shared save list title.
pub const SAVE_LIST_TITLE: &str = "Saves";
/// Shared save list footer hint.
pub const SAVE_LIST_FOOTER: &str = "Up/Down: select  Enter: load  Back: menu";
const MAP_SELECT_STATUS: &str = "Select a map and press Enter";
const SAVE_SELECT_STATUS: &str = "Select a save and press Enter";
const MAP_LOADED_STATUS: &str = "Map loaded. Press Enter to switch between pan and hero mode";
const SAVE_LOADED_STATUS: &str = "Save loaded";

/// Layout numbers derived from the current target device size.
#[derive(Clone, Copy)]
pub struct AppLayout {
    /// Number of visible rows in list selectors.
    pub list_rows: usize,
    /// Number of visible rows in the save overlay load-list.
    pub save_rows: usize,
    /// Number of visible map columns.
    pub map_visible_cols: usize,
    /// Number of visible map rows.
    pub map_visible_rows: usize,
}

/// Initial boot options used by the shared app controller.
pub struct LaunchConfig {
    /// Optional requested map id or display name for direct boot.
    pub start_map: Option<String>,
    /// Initial viewport x offset when opening a map.
    pub start_x: usize,
    /// Initial viewport y offset when opening a map.
    pub start_y: usize,
}

/// Loaded game data returned by host storage implementations.
pub struct LoadedGame {
    /// Human-readable map or save name.
    pub map_name: String,
    /// Full engine state to attach to the session.
    pub state: GameState,
}

/// Shared gameplay screen state with optional overlays.
pub struct MapViewScreen {
    /// Shared gameplay map-view application model.
    pub app: MapViewApp,
    /// Optional status line mirrored for host convenience.
    pub status: Option<String>,
    /// Optional information overlay.
    pub info_overlay: Option<InfoOverlay>,
    /// Optional save/load overlay.
    pub save_overlay: Option<SaveOverlay>,
}

/// Top-level shared screen state.
pub enum AppScreen {
    /// Shared splash screen.
    Splash(SplashScreen),
    /// Shared map selection list.
    MapSelect(ListScreen),
    /// Shared save selection list.
    SaveSelect(ListScreen),
    /// Shared gameplay map view with overlays.
    MapView(Box<MapViewScreen>),
}

/// Host-side storage and platform hooks required by the shared controller.
pub trait AppHost {
    /// Host-specific error type for map/save access.
    type Error;

    /// Returns all loadable maps shown under "New Game".
    fn discover_maps(&mut self) -> Result<Vec<ListEntry>, Self::Error>;
    /// Returns all loadable saves shown under "Load Game".
    fn discover_saves(&mut self) -> Result<Vec<ListEntry>, Self::Error>;
    /// Loads a map entry into a full engine state.
    fn load_map(&mut self, entry: &ListEntry) -> Result<LoadedGame, Self::Error>;
    /// Loads a save entry into a full engine state.
    fn load_save(&mut self, entry: &ListEntry) -> Result<LoadedGame, Self::Error>;
    /// Persists the current engine state as a save file.
    fn save_game(&mut self, name: &str, state: &GameState) -> Result<(), Self::Error>;
    /// Builds an optional platform information overlay.
    fn info_overlay(&mut self) -> Option<InfoOverlay>;
    /// Converts a host-specific error into a user-visible message.
    fn error_message(&self, error: Self::Error) -> String;
}

/// Shared top-level app controller reused by all embedded frontends.
pub struct EmbeddedApp {
    screen: AppScreen,
    launch: LaunchConfig,
}

impl EmbeddedApp {
    /// Builds the first shared screen and stores the launch options.
    ///
    /// # Arguments
    /// * `host` - Platform host implementation used for initial discovery.
    /// * `launch` - Initial boot configuration.
    pub fn new<H>(host: &mut H, launch: LaunchConfig) -> Self
    where
        H: AppHost,
    {
        let screen = initial_screen(host, &launch);
        Self { screen, launch }
    }

    /// Returns the current top-level screen.
    pub fn screen(&self) -> &AppScreen {
        &self.screen
    }

    /// Returns the current top-level screen mutably.
    pub fn screen_mut(&mut self) -> &mut AppScreen {
        &mut self.screen
    }

    /// Clamps the visible viewport if the current screen is a map view.
    ///
    /// # Arguments
    /// * `layout` - Current host layout metrics.
    pub fn clamp_view_to_layout(&mut self, layout: AppLayout) {
        if let AppScreen::MapView(map_view) = &mut self.screen {
            map_view.app.clamp_view_to_map(layout.map_visible_cols, layout.map_visible_rows);
        }
    }

    /// Applies one platform-neutral input event to the shared app controller.
    ///
    /// # Arguments
    /// * `host` - Platform host callbacks.
    /// * `event` - Input event from the host.
    /// * `layout` - Current host layout metrics.
    pub fn handle_input<H>(&mut self, host: &mut H, event: InputEvent, layout: AppLayout) -> bool
    where
        H: AppHost,
    {
        match &mut self.screen {
            AppScreen::Splash(splash) => {
                let outcome = handle_splash(host, splash, event);
                if let Some(next) = outcome.next_screen {
                    self.screen = next;
                }
                outcome.changed
            }
            AppScreen::MapSelect(map_select) => {
                let outcome = handle_map_select(host, map_select, event, &self.launch, layout);
                if let Some(next) = outcome.next_screen {
                    self.screen = next;
                }
                outcome.changed
            }
            AppScreen::SaveSelect(save_select) => {
                let outcome = handle_save_select(host, save_select, event, layout);
                if let Some(next) = outcome.next_screen {
                    self.screen = next;
                }
                outcome.changed
            }
            AppScreen::MapView(map_view) => {
                let outcome = handle_map_view(host, map_view.as_mut(), event, layout);
                if let Some(next) = outcome.next_screen {
                    self.screen = next;
                }
                outcome.changed
            }
        }
    }
}

struct ScreenOutcome {
    changed: bool,
    next_screen: Option<AppScreen>,
}

fn initial_screen<H>(host: &mut H, launch: &LaunchConfig) -> AppScreen
where
    H: AppHost,
{
    if let Some(requested_map) = launch.start_map.as_deref() {
        match host.discover_maps() {
            Ok(maps) => {
                if let Some(entry) = maps.iter().find(|entry| {
                    names_match(&entry.id, requested_map)
                        || names_match(&entry.label, requested_map)
                }) {
                    return match build_map_view(
                        host,
                        entry,
                        launch,
                        Some(MAP_LOADED_STATUS.to_string()),
                    ) {
                        Ok(screen) => AppScreen::MapView(Box::new(screen)),
                        Err(message) => AppScreen::MapSelect(ListScreen::new(maps, Some(message))),
                    };
                }
                return AppScreen::MapSelect(ListScreen::new(
                    maps,
                    Some("Configured map does not match any discovered map".to_string()),
                ));
            }
            Err(error) => {
                return AppScreen::Splash(SplashScreen::new(0, Some(host.error_message(error))));
            }
        }
    }

    AppScreen::Splash(SplashScreen::new(0, None))
}

fn handle_splash<H>(host: &mut H, splash: &mut SplashScreen, event: InputEvent) -> ScreenOutcome
where
    H: AppHost,
{
    splash.status = None;
    match splash.handle_input(event, SPLASH_OPTIONS.len()) {
        SplashOutcome::Changed => ScreenOutcome { changed: true, next_screen: None },
        SplashOutcome::Selected(selected) => {
            let next_screen = match selected {
                0 => match host.discover_maps() {
                    Ok(maps) => AppScreen::MapSelect(ListScreen::new(
                        maps,
                        Some(MAP_SELECT_STATUS.to_string()),
                    )),
                    Err(error) => AppScreen::Splash(SplashScreen::new(
                        splash.selected,
                        Some(host.error_message(error)),
                    )),
                },
                _ => match host.discover_saves() {
                    Ok(saves) => AppScreen::SaveSelect(ListScreen::new(
                        saves,
                        Some(SAVE_SELECT_STATUS.to_string()),
                    )),
                    Err(error) => AppScreen::Splash(SplashScreen::new(
                        splash.selected,
                        Some(host.error_message(error)),
                    )),
                },
            };
            ScreenOutcome { changed: true, next_screen: Some(next_screen) }
        }
        SplashOutcome::BackRequested | SplashOutcome::NoChange => {
            ScreenOutcome { changed: false, next_screen: None }
        }
    }
}

fn handle_map_select<H>(
    host: &mut H,
    map_select: &mut ListScreen,
    event: InputEvent,
    launch: &LaunchConfig,
    layout: AppLayout,
) -> ScreenOutcome
where
    H: AppHost,
{
    if map_select.entries.is_empty() {
        if matches!(event, InputEvent::Enter) {
            return ScreenOutcome {
                changed: true,
                next_screen: Some(AppScreen::Splash(SplashScreen::new(
                    0,
                    Some("No maps found".to_string()),
                ))),
            };
        }
        return ScreenOutcome { changed: false, next_screen: None };
    }

    match map_select.handle_input(event, layout.list_rows) {
        ListOutcome::NoChange => ScreenOutcome { changed: false, next_screen: None },
        ListOutcome::Changed => ScreenOutcome { changed: true, next_screen: None },
        ListOutcome::BackRequested => ScreenOutcome {
            changed: true,
            next_screen: Some(AppScreen::Splash(SplashScreen::new(0, None))),
        },
        ListOutcome::Selected(selected) => match map_select.entries.get(selected) {
            Some(entry) => {
                match build_map_view(host, entry, launch, Some(MAP_LOADED_STATUS.to_string())) {
                    Ok(map_view) => ScreenOutcome {
                        changed: true,
                        next_screen: Some(AppScreen::MapView(Box::new(clamped_map_view(
                            map_view, layout,
                        )))),
                    },
                    Err(message) => {
                        map_select.status = Some(message);
                        ScreenOutcome { changed: true, next_screen: None }
                    }
                }
            }
            None => ScreenOutcome { changed: false, next_screen: None },
        },
    }
}

fn handle_save_select<H>(
    host: &mut H,
    save_select: &mut ListScreen,
    event: InputEvent,
    layout: AppLayout,
) -> ScreenOutcome
where
    H: AppHost,
{
    if save_select.entries.is_empty() {
        if matches!(event, InputEvent::Enter) {
            return ScreenOutcome {
                changed: true,
                next_screen: Some(AppScreen::Splash(SplashScreen::new(
                    1,
                    Some("No save files found".to_string()),
                ))),
            };
        }
        return ScreenOutcome { changed: false, next_screen: None };
    }

    match save_select.handle_input(event, layout.list_rows) {
        ListOutcome::NoChange => ScreenOutcome { changed: false, next_screen: None },
        ListOutcome::Changed => ScreenOutcome { changed: true, next_screen: None },
        ListOutcome::BackRequested => ScreenOutcome {
            changed: true,
            next_screen: Some(AppScreen::Splash(SplashScreen::new(1, None))),
        },
        ListOutcome::Selected(selected) => match save_select.entries.get(selected) {
            Some(entry) => match host.load_save(entry) {
                Ok(loaded) => {
                    match map_view_from_loaded(loaded, 0, 0, Some(SAVE_LOADED_STATUS.to_string())) {
                        Ok(map_view) => ScreenOutcome {
                            changed: true,
                            next_screen: Some(AppScreen::MapView(Box::new(clamped_map_view(
                                map_view, layout,
                            )))),
                        },
                        Err(message) => {
                            save_select.status = Some(message);
                            ScreenOutcome { changed: true, next_screen: None }
                        }
                    }
                }
                Err(error) => {
                    save_select.status = Some(host.error_message(error));
                    ScreenOutcome { changed: true, next_screen: None }
                }
            },
            None => ScreenOutcome { changed: false, next_screen: None },
        },
    }
}

fn handle_map_view<H>(
    host: &mut H,
    map_view: &mut MapViewScreen,
    event: InputEvent,
    layout: AppLayout,
) -> ScreenOutcome
where
    H: AppHost,
{
    if map_view.save_overlay.is_some() {
        return handle_save_overlay(host, map_view, event, layout);
    }

    if let Some(info_overlay) = map_view.info_overlay.as_ref() {
        return match info_overlay.handle_input(event) {
            InfoOverlayOutcome::Close => {
                map_view.info_overlay = None;
                ScreenOutcome { changed: true, next_screen: None }
            }
            InfoOverlayOutcome::NoChange => ScreenOutcome { changed: false, next_screen: None },
        };
    }

    if is_key(event, 'i') {
        if let Some(overlay) = host.info_overlay() {
            map_view.info_overlay = Some(overlay);
            return ScreenOutcome { changed: true, next_screen: None };
        }
    }

    if is_key(event, 'p') {
        map_view.save_overlay = Some(SaveOverlay::menu());
        return ScreenOutcome { changed: true, next_screen: None };
    }

    match map_view.app.handle_input(event, layout.map_visible_cols, layout.map_visible_rows) {
        MapViewOutcome::NoChange => ScreenOutcome { changed: false, next_screen: None },
        MapViewOutcome::Changed => {
            map_view.status = map_view.app.status().map(ToString::to_string);
            ScreenOutcome { changed: true, next_screen: None }
        }
        MapViewOutcome::BackRequested => match host.discover_maps() {
            Ok(maps) => {
                let mut screen =
                    ListScreen::new(maps, Some("Returned to map selection".to_string()));
                screen.selected = screen
                    .entries
                    .iter()
                    .position(|entry| entry.label == map_view.app.session().map_name())
                    .unwrap_or(0);
                screen.scroll = screen.selected;
                ScreenOutcome { changed: true, next_screen: Some(AppScreen::MapSelect(screen)) }
            }
            Err(error) => ScreenOutcome {
                changed: true,
                next_screen: Some(AppScreen::Splash(SplashScreen::new(
                    0,
                    Some(host.error_message(error)),
                ))),
            },
        },
    }
}

fn handle_save_overlay<H>(
    host: &mut H,
    map_view: &mut MapViewScreen,
    event: InputEvent,
    layout: AppLayout,
) -> ScreenOutcome
where
    H: AppHost,
{
    let overlay = map_view.save_overlay.take();
    let Some(mut overlay) = overlay else {
        return ScreenOutcome { changed: false, next_screen: None };
    };

    match overlay.handle_input(event, layout.save_rows) {
        SaveOverlayOutcome::NoChange => {
            map_view.save_overlay = Some(overlay);
            ScreenOutcome { changed: false, next_screen: None }
        }
        SaveOverlayOutcome::Changed => {
            map_view.save_overlay = Some(overlay);
            ScreenOutcome { changed: true, next_screen: None }
        }
        SaveOverlayOutcome::Close => {
            map_view.save_overlay = None;
            ScreenOutcome { changed: true, next_screen: None }
        }
        SaveOverlayOutcome::RequestDiscoverSaves => {
            match host.discover_saves() {
                Ok(saves) => {
                    overlay = SaveOverlay::load_list(ListScreen::new(saves, None));
                }
                Err(error) => overlay.set_status(Some(host.error_message(error))),
            }
            map_view.save_overlay = Some(overlay);
            ScreenOutcome { changed: true, next_screen: None }
        }
        SaveOverlayOutcome::RequestSave(name) => {
            match host.save_game(&name, map_view.app.session().state()) {
                Ok(()) => {
                    map_view.status = Some(format!("Saved game: {name}"));
                    map_view.save_overlay = None;
                }
                Err(error) => {
                    overlay.set_status(Some(host.error_message(error)));
                    map_view.save_overlay = Some(overlay);
                }
            }
            ScreenOutcome { changed: true, next_screen: None }
        }
        SaveOverlayOutcome::RequestLoad(selected) => {
            match host.discover_saves() {
                Ok(saves) => {
                    if let Some(entry) = saves.get(selected) {
                        match host.load_save(entry) {
                            Ok(loaded) => match map_view_from_loaded(
                                loaded,
                                map_view.app.view_x(),
                                map_view.app.view_y(),
                                Some(SAVE_LOADED_STATUS.to_string()),
                            ) {
                                Ok(next_map_view) => {
                                    *map_view = clamped_map_view(next_map_view, layout);
                                }
                                Err(message) => {
                                    overlay.set_status(Some(message));
                                    map_view.save_overlay = Some(overlay);
                                }
                            },
                            Err(error) => {
                                overlay.set_status(Some(host.error_message(error)));
                                map_view.save_overlay = Some(overlay);
                            }
                        }
                    } else {
                        overlay.set_status(Some("Selected save no longer exists".to_string()));
                        map_view.save_overlay = Some(overlay);
                    }
                }
                Err(error) => {
                    overlay.set_status(Some(host.error_message(error)));
                    map_view.save_overlay = Some(overlay);
                }
            }
            ScreenOutcome { changed: true, next_screen: None }
        }
    }
}

fn build_map_view<H>(
    host: &mut H,
    entry: &ListEntry,
    launch: &LaunchConfig,
    status: Option<String>,
) -> Result<MapViewScreen, String>
where
    H: AppHost,
{
    match host.load_map(entry) {
        Ok(loaded) => map_view_from_loaded(loaded, launch.start_x, launch.start_y, status),
        Err(error) => Err(host.error_message(error)),
    }
}

fn map_view_from_loaded(
    loaded: LoadedGame,
    view_x: usize,
    view_y: usize,
    status: Option<String>,
) -> Result<MapViewScreen, String> {
    let session = GameSession::from_state(loaded.map_name, loaded.state)
        .map_err(|error| error.to_string())?;

    Ok(MapViewScreen {
        app: MapViewApp::new(session, view_x, view_y, status.clone()),
        status,
        info_overlay: None,
        save_overlay: None,
    })
}

fn clamped_map_view(mut map_view: MapViewScreen, layout: AppLayout) -> MapViewScreen {
    map_view.app.clamp_view_to_map(layout.map_visible_cols, layout.map_visible_rows);
    map_view
}

fn is_key(event: InputEvent, key: char) -> bool {
    match event {
        InputEvent::Key(ch) => ch.eq_ignore_ascii_case(&key),
        _ => false,
    }
}

fn names_match(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}
