use std::collections::{BTreeSet, VecDeque};

use engine::Direction;
use engine::game_state::ROD_COST;
use engine::hero::HeroId;
use engine::map::game_map::Direction as MapDir;
use engine::map::game_map::{GameMap, MapCoord};
use engine::map::tile::Tiles;

use super::input::InputEvent;
use super::session::GameSession;

/// Flood-fill a connected city starting from `start`.
fn flood_city(map: &GameMap, start: MapCoord) -> Vec<MapCoord> {
    let is_city = map.get_tile(start).map(|t| matches!(t.kind, Tiles::City)).unwrap_or(false);

    if !is_city {
        return vec![start];
    }

    let w = map.tile_width();
    let h = map.tile_height();
    let mut visited: BTreeSet<MapCoord> = BTreeSet::new();
    let mut queue: VecDeque<MapCoord> = VecDeque::new();
    let mut result: Vec<MapCoord> = Vec::new();

    visited.insert(start);
    queue.push_back(start);

    while let Some(coord) = queue.pop_front() {
        result.push(coord);

        for dir in [MapDir::North, MapDir::East, MapDir::South, MapDir::West] {
            if let Some(neighbor) = dir.apply(coord, w, h)
                && !visited.contains(&neighbor)
                && map.get_tile(neighbor).map(|t| matches!(t.kind, Tiles::City)).unwrap_or(false)
            {
                visited.insert(neighbor);
                queue.push_back(neighbor);
            }
        }
    }

    result
}

/// Description of a structure under the cursor.
pub struct StructureInfo {
    /// Display name ("City", "Ruins", etc.).
    pub name: String,
    /// Min x tile.
    pub min_x: u32,
    /// Min y tile.
    pub min_y: u32,
    /// Max x tile.
    pub max_x: u32,
    /// Max y tile.
    pub max_y: u32,
}

impl StructureInfo {
    /// Width in tiles.
    pub fn width(&self) -> u32 {
        self.max_x - self.min_x + 1
    }

    /// Height in tiles.
    pub fn height(&self) -> u32 {
        self.max_y - self.min_y + 1
    }
}

/// Result of applying one shared input event to the map view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MapViewOutcome {
    /// State did not change.
    NoChange,
    /// State changed and should be redrawn.
    Changed,
    /// Cursor moved (triggers redraw but not status update).
    CursorChanged,
    /// User requested ending current turn.
    RequestEndTurn,
    /// User pressed Enter on a structure under the cursor.
    OpenStructureOverlay { name: String },
    /// User pressed Enter on a hero standing outside any structure.
    OpenHeroInfo,
}

/// Shared gameplay screen model for frontends.
pub struct MapViewApp {
    session: GameSession,
    view_x: usize,
    view_y: usize,
    cursor_x: isize,
    cursor_y: isize,
    status: Option<String>,
}

impl MapViewApp {
    /// Creates a new shared map-view state.
    pub fn new(session: GameSession, view_x: usize, view_y: usize, status: Option<String>) -> Self {
        let mut app = Self {
            session,
            view_x,
            view_y,
            cursor_x: view_x as isize,
            cursor_y: view_y as isize,
            status,
        };
        app.sync_cursor_to_hero();
        app
    }

    /// Returns the shared gameplay session.
    pub fn session(&self) -> &GameSession {
        &self.session
    }

    /// Returns the mutable gameplay session.
    pub fn session_mut(&mut self) -> &mut GameSession {
        &mut self.session
    }

    /// Returns the current leftmost visible tile.
    pub fn view_x(&self) -> usize {
        self.view_x
    }

    pub fn view_y(&self) -> usize {
        self.view_y
    }

    pub fn cursor_x(&self) -> isize {
        self.cursor_x
    }

    pub fn cursor_y(&self) -> isize {
        self.cursor_y
    }

    /// Sync cursor position to the currently selected hero.
    pub fn sync_cursor_to_hero(&mut self) {
        if let Some(id) = self.session.selected_hero_id()
            && let Some(hero) = self.session.state().hero(id)
        {
            self.cursor_x = hero.position.x as isize;
            self.cursor_y = hero.position.y as isize;
            return;
        }
        // No hero — center on the team's city instead.
        if let Ok(team_id) = self.session.state().get_active_team_id() {
            // Prefer city entrance (where heroes can be hired).
            if let Some(coord) = self.session.state().city_entrance_for_team(*team_id) {
                self.cursor_x = coord.x as isize;
                self.cursor_y = coord.y as isize;
                return;
            }
            // Fall back to any owned city tile.
            if let Some(coord) = self.session.state().city_owner_for_team(*team_id) {
                self.cursor_x = coord.x as isize;
                self.cursor_y = coord.y as isize;
                return;
            }
        }
        // Absolute fallback — (0, 0).
        self.cursor_x = 0;
        self.cursor_y = 0;
    }

    /// Returns the current footer status line.
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// Replaces the current footer status line.
    pub fn set_status(&mut self, status: Option<String>) {
        self.status = status;
    }

    pub fn clamp_cursor(&mut self) {
        let w = self.session.state().map.tile_width() as isize;
        let h = self.session.state().map.tile_height() as isize;
        self.cursor_x = self.cursor_x.clamp(0, w - 1);
        self.cursor_y = self.cursor_y.clamp(0, h - 1);
    }

    /// Places the cursor at an absolute map coordinate and applies city snapping.
    ///
    /// Returns `true` when the resulting cursor position changed.
    /// Sets the cursor position to the given map coordinates.
    pub fn set_cursor_pos(&mut self, x: u32, y: u32) {
        self.cursor_x = x as isize;
        self.cursor_y = y as isize;
    }

    pub fn set_cursor_from_pointer(
        &mut self,
        x: usize,
        y: usize,
        visible_cols: usize,
        visible_rows: usize,
    ) -> bool {
        let previous_x = self.cursor_x;
        let previous_y = self.cursor_y;
        self.cursor_x = x as isize;
        self.cursor_y = y as isize;
        self.clamp_cursor();
        self.snap_cursor_to_city_center();
        self.scroll_cursor_into_view(visible_cols, visible_rows);
        self.cursor_x != previous_x || self.cursor_y != previous_y
    }

    /// Moves the cursor so the target tile is visible within the viewport.
    pub fn scroll_cursor_into_view(&mut self, visible_cols: usize, visible_rows: usize) -> bool {
        let mut changed = false;
        while self.cursor_x < self.view_x as isize {
            self.view_x = self.view_x.saturating_sub(1);
            changed = true;
        }
        while self.cursor_x >= (self.view_x + visible_cols) as isize {
            self.view_x += 1;
            changed = true;
        }
        while self.cursor_y < self.view_y as isize {
            self.view_y = self.view_y.saturating_sub(1);
            changed = true;
        }
        while self.cursor_y >= (self.view_y + visible_rows) as isize {
            self.view_y += 1;
            changed = true;
        }
        changed
    }

    /// Finds the nearest `CityEntrance` tile within the city structure under the cursor.
    ///
    /// Uses flood fill from the cursor position to discover all connected city/entrance
    /// tiles, then returns the `CityEntrance` closest to the cursor (Manhattan distance).
    /// Returns `None` if the cursor is not on a city tile or no entrance exists.
    pub fn find_city_entrance_at_cursor(&self) -> Option<MapCoord> {
        let coord = MapCoord::new(self.cursor_x.max(0) as u32, self.cursor_y.max(0) as u32);
        let city_tiles = engine::state_flood::flood_city(&self.session.state().map, coord);
        let cursor = MapCoord::new(coord.x, coord.y);
        city_tiles
            .iter()
            .filter(|c| {
                self.session
                    .state()
                    .map
                    .get_tile(**c)
                    .map(|t| t.kind == Tiles::CityEntrance)
                    .unwrap_or(false)
            })
            .min_by_key(|c| {
                (c.x as i32).abs_diff(cursor.x as i32) + (c.y as i32).abs_diff(cursor.y as i32)
            })
            .copied()
    }

    /// Detects what structure (if any) is under the cursor.
    ///
    /// For a city tile, floods the full city complex and returns the bounding box.
    /// For Ruins, Merchant, Village, Gold, Resource — returns a single tile.
    pub fn cursor_structure(&self) -> Option<StructureInfo> {
        let map = &self.session.state().map;
        let x = self.cursor_x as u32;
        let y = self.cursor_y as u32;
        let coord = MapCoord::new(x, y);
        let Ok(tile) = map.get_tile(coord) else {
            return None;
        };
        match tile.kind {
            Tiles::City => {
                let tiles = flood_city(map, coord);
                if tiles.is_empty() {
                    return None;
                }
                let mut min_x = u32::MAX;
                let mut min_y = u32::MAX;
                let mut max_x = 0u32;
                let mut max_y = 0u32;
                for c in &tiles {
                    min_x = min_x.min(c.x);
                    min_y = min_y.min(c.y);
                    max_x = max_x.max(c.x);
                    max_y = max_y.max(c.y);
                }
                Some(StructureInfo { name: "City".to_string(), min_x, min_y, max_x, max_y })
            }
            Tiles::CityEntrance => Some(StructureInfo {
                name: "City Entrance".to_string(),
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            }),
            Tiles::Ruins => Some(StructureInfo {
                name: "Ruins".to_string(),
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            }),
            Tiles::Merchant => Some(StructureInfo {
                name: "Merchant".to_string(),
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            }),
            Tiles::Village => Some(StructureInfo {
                name: "Village".to_string(),
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            }),
            Tiles::Gold => Some(StructureInfo {
                name: "Gold Deposit".to_string(),
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            }),
            Tiles::Resource => Some(StructureInfo {
                name: "Resource Node".to_string(),
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            }),
            _ => None,
        }
    }

    /// Returns the map coordinate currently under the cursor, if it is valid.
    pub fn cursor_coord(&self) -> Option<MapCoord> {
        if self.cursor_x < 0 || self.cursor_y < 0 {
            return None;
        }
        Some(MapCoord::new(self.cursor_x as u32, self.cursor_y as u32))
    }

    /// Returns the id of the living hero standing under the cursor, if any.
    pub fn cursor_hero_id(&self) -> Option<HeroId> {
        let coord = self.cursor_coord()?;
        self.session.state().hero_at(coord).map(|hero| hero.get_id())
    }

    /// Applies a single input event to the shared map view.
    ///
    /// Arrow keys move the selected hero one tile.
    /// WASD pans the viewport.
    /// HJKL moves the cursor.
    /// Tab cycles to the next hero and centers the camera on them.
    /// Enter on a structure opens its overlay.
    pub fn handle_input(
        &mut self,
        event: InputEvent,
        visible_cols: usize,
        visible_rows: usize,
    ) -> MapViewOutcome {
        match event {
            InputEvent::Up | InputEvent::Down | InputEvent::Left | InputEvent::Right => {
                self.move_hero_or_report(event, visible_cols, visible_rows)
            }
            InputEvent::CursorUp
            | InputEvent::CursorDown
            | InputEvent::CursorLeft
            | InputEvent::CursorRight => {
                self.move_cursor_and_snap(event, visible_cols, visible_rows)
            }
            InputEvent::NextHero => {
                self.session_mut().cycle_selected_hero();
                self.sync_cursor_to_hero();
                self.center_on_hero(visible_cols, visible_rows);
                self.status = Some(self.session.summary());
                MapViewOutcome::Changed
            }
            InputEvent::PlaceRod => match self.session_mut().place_resource_rod() {
                Ok(pos) => {
                    self.cursor_x = pos.x as isize;
                    self.cursor_y = pos.y as isize;
                    self.scroll_cursor_into_view(visible_cols, visible_rows);
                    self.status = Some(format!("Resource rod placed (-{ROD_COST} gold)"));
                    MapViewOutcome::Changed
                }
                Err(e) => {
                    self.status = Some(e.to_string());
                    MapViewOutcome::Changed
                }
            },
            InputEvent::PanUp => self.pan_view(InputEvent::Up, visible_cols, visible_rows),
            InputEvent::PanDown => self.pan_view(InputEvent::Down, visible_cols, visible_rows),
            InputEvent::PanLeft => self.pan_view(InputEvent::Left, visible_cols, visible_rows),
            InputEvent::PanRight => self.pan_view(InputEvent::Right, visible_cols, visible_rows),
            InputEvent::Enter => {
                if let Some(info) = self.cursor_structure() {
                    MapViewOutcome::OpenStructureOverlay { name: info.name }
                } else if self.cursor_hero_id().is_some() {
                    MapViewOutcome::OpenHeroInfo
                } else {
                    MapViewOutcome::NoChange
                }
            }
            InputEvent::NextTurn => MapViewOutcome::RequestEndTurn,
        }
    }

    fn pan_view(
        &mut self,
        event: InputEvent,
        visible_cols: usize,
        visible_rows: usize,
    ) -> MapViewOutcome {
        let map = &self.session.state().map;
        let max_x = map.tile_width().saturating_sub(visible_cols as u32) as usize;
        let max_y = map.tile_height().saturating_sub(visible_rows as u32) as usize;

        let previous_x = self.view_x;
        let previous_y = self.view_y;
        match event {
            InputEvent::Up => self.view_y = self.view_y.saturating_sub(1),
            InputEvent::Down => self.view_y = (self.view_y + 1).min(max_y),
            InputEvent::Left => self.view_x = self.view_x.saturating_sub(1),
            InputEvent::Right => self.view_x = (self.view_x + 1).min(max_x),
            _ => {}
        }

        if self.view_x != previous_x || self.view_y != previous_y {
            MapViewOutcome::Changed
        } else {
            MapViewOutcome::NoChange
        }
    }

    fn move_cursor(&mut self, event: InputEvent) -> bool {
        let w = self.session.state().map.tile_width() as isize;
        let h = self.session.state().map.tile_height() as isize;
        let previous_x = self.cursor_x;
        let previous_y = self.cursor_y;
        match event {
            InputEvent::CursorUp => self.cursor_y = (self.cursor_y - 1).clamp(0, h - 1),
            InputEvent::CursorDown => self.cursor_y = (self.cursor_y + 1).clamp(0, h - 1),
            InputEvent::CursorLeft => self.cursor_x = (self.cursor_x - 1).clamp(0, w - 1),
            InputEvent::CursorRight => self.cursor_x = (self.cursor_x + 1).clamp(0, w - 1),
            _ => {}
        }
        self.cursor_x != previous_x || self.cursor_y != previous_y
    }

    fn move_cursor_and_snap(
        &mut self,
        event: InputEvent,
        visible_cols: usize,
        visible_rows: usize,
    ) -> MapViewOutcome {
        let moved = if self.cursor_is_city_tile() {
            self.move_cursor_out_of_city(event)
        } else {
            self.move_cursor(event)
        };

        if !moved {
            return MapViewOutcome::NoChange;
        }

        self.snap_cursor_to_city_center();
        self.scroll_cursor_into_view(visible_cols, visible_rows);
        MapViewOutcome::CursorChanged
    }

    fn cursor_is_city_tile(&self) -> bool {
        if self.cursor_x < 0 || self.cursor_y < 0 {
            return false;
        }
        let coord = MapCoord::new(self.cursor_x as u32, self.cursor_y as u32);
        self.session
            .state()
            .map
            .get_tile(coord)
            .map(|tile| matches!(tile.kind, Tiles::City))
            .unwrap_or(false)
    }

    fn move_cursor_out_of_city(&mut self, event: InputEvent) -> bool {
        let (dx, dy) = match event {
            InputEvent::CursorUp => (0, -1),
            InputEvent::CursorDown => (0, 1),
            InputEvent::CursorLeft => (-1, 0),
            InputEvent::CursorRight => (1, 0),
            _ => return false,
        };
        let map = &self.session.state().map;
        let w = map.tile_width() as isize;
        let h = map.tile_height() as isize;
        let mut next_x = self.cursor_x + dx;
        let mut next_y = self.cursor_y + dy;

        while next_x >= 0 && next_y >= 0 && next_x < w && next_y < h {
            let coord = MapCoord::new(next_x as u32, next_y as u32);
            let is_city =
                map.get_tile(coord).map(|tile| matches!(tile.kind, Tiles::City)).unwrap_or(false);
            if !is_city {
                self.cursor_x = next_x;
                self.cursor_y = next_y;
                return true;
            }
            next_x += dx;
            next_y += dy;
        }

        false
    }

    fn snap_cursor_to_city_center(&mut self) -> bool {
        let Some(info) = self.cursor_structure() else {
            return false;
        };
        if info.name != "City" {
            return false;
        }

        let center_x = ((info.min_x + info.max_x) / 2) as isize;
        let center_y = ((info.min_y + info.max_y) / 2) as isize;
        let changed = self.cursor_x != center_x || self.cursor_y != center_y;
        self.cursor_x = center_x;
        self.cursor_y = center_y;
        changed
    }

    fn move_hero_or_report(
        &mut self,
        event: InputEvent,
        visible_cols: usize,
        visible_rows: usize,
    ) -> MapViewOutcome {
        let direction = match event {
            InputEvent::Up => Some(Direction::North),
            InputEvent::Down => Some(Direction::South),
            InputEvent::Left => Some(Direction::West),
            InputEvent::Right => Some(Direction::East),
            _ => None,
        };

        let Some(direction) = direction else {
            return MapViewOutcome::NoChange;
        };

        match self.session.move_selected_hero(direction) {
            Ok(_position) => {
                self.status = Some(self.session.summary());
                self.sync_cursor_to_hero();
                self.center_on_hero(visible_cols, visible_rows);
                MapViewOutcome::Changed
            }
            Err(err) => {
                self.status = Some(err.to_string());
                MapViewOutcome::Changed
            }
        }
    }

    /// Centers the view on the cursor position (which tracks the selected hero
    /// or the team's city when no hero exists).
    pub fn center_on_hero(&mut self, visible_cols: usize, visible_rows: usize) -> bool {
        let cx = self.cursor_x.max(0) as usize;
        let cy = self.cursor_y.max(0) as usize;

        let map = &self.session.state().map;
        let max_x = map.tile_width().saturating_sub(visible_cols as u32) as usize;
        let max_y = map.tile_height().saturating_sub(visible_rows as u32) as usize;

        let target_x = if cx >= visible_cols / 2 { (cx - visible_cols / 2).min(max_x) } else { 0 };
        let target_y = if cy >= visible_rows / 2 { (cy - visible_rows / 2).min(max_y) } else { 0 };

        let changed = self.view_x != target_x || self.view_y != target_y;
        self.view_x = target_x;
        self.view_y = target_y;
        changed
    }
}
