use crate::atlas::{TeamLogoImages, TileAtlas};
use crate::input::{InputCooldown, UiAction};
use crate::input_event::InputEvent;
use crate::screens::team_setup::LoadedSession;
use crate::screens::AppState;
use ai::{AiAction, AiContext, AiFactory, AiStrategyKind};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use engine::config::{TeamLogo, TileConfig};
use engine::error::EngineError;
use engine::game_state::{GameState, TurnIncome, TurnStartReport, ROD_COST};
use engine::hero::{HeroId, TeamId};
use engine::map::game_map::{Direction as MapDir, ResourceKind, RESOURCE_KIND_COUNT};
use engine::map::tile::Tiles;
use engine::MapCoord;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Component)]
pub struct MapViewRoot;

#[derive(Component)]
pub struct EndTurnOverlay;

#[derive(Component)]
pub struct PauseOverlay;

#[derive(Component)]
pub struct TurnStartOverlay;

#[derive(Component)]
pub struct EndTurnConfirmButton;

#[derive(Component)]
pub struct EndTurnCancelButton;

#[derive(Component)]
pub struct TurnStartConfirmButton;

#[derive(Component)]
pub struct PauseResumeButton;

#[derive(Component)]
pub struct PauseQuitButton;

/// Marker for the bottom status-bar text (scopes the status update query).
#[derive(Component)]
pub struct StatusText;

/// Marker for the top resource/turn HUD bar root.
#[derive(Component)]
pub struct TopBarRoot;

/// Identifies which treasury value a top-bar text entity displays.
#[derive(Component, Clone, Copy)]
pub enum TopBarField {
    /// Current team turn number + score.
    Turn,
    /// Gold balance.
    Gold,
    /// Stockpile of the resource at this index (0–3).
    Resource(usize),
}

/// Tint for gold values in the HUD.
const GOLD_COLOR: Color = Color::srgb(0.96, 0.82, 0.30);

/// Atlas index of the gold pictogram, sourced from `tiles.yaml` (`gold` tile).
fn gold_icon_index(tile_config: &TileConfig) -> usize {
    tile_config.atlas_index("gold").unwrap_or(0) as usize
}

/// Atlas indices of the four resource pictograms, sourced from `tiles.yaml`
/// (`resource` tile variants, in declaration order).
fn resource_icon_indices(tile_config: &TileConfig) -> [usize; RESOURCE_KIND_COUNT] {
    let mut icons = [0usize; RESOURCE_KIND_COUNT];
    if let Some(indexes) = tile_config.atlas_indexes("resource") {
        for (slot, index) in icons.iter_mut().zip(indexes) {
            *slot = index as usize;
        }
    }
    icons
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

/// Flood-fill a connected city starting from `start`.
fn flood_city(map: &engine::map::game_map::GameMap, start: MapCoord) -> Vec<MapCoord> {
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

#[derive(Resource)]
pub struct MapViewState {
    pub state: Option<GameState>,
    pub selected_hero_id: Option<HeroId>,
    pub view_x: usize,
    pub view_y: usize,
    pub cursor_x: isize,
    pub cursor_y: isize,
    pub status: Option<String>,
    pub tile_size: f32,
    pub visible_cols: usize,
    pub visible_rows: usize,
    pub needs_initial_draw: bool,
    pub end_turn_overlay: bool,
    pub end_turn_selected: usize,
    pub pause_overlay: bool,
    pub pause_selected: usize,
    pub last_mouse_tile: Option<(usize, usize)>,
    /// Handle to the tile atlas image (1_main.png).
    pub atlas_image: Handle<Image>,
    /// Handle to the tile atlas layout.
    pub atlas_layout: Handle<TextureAtlasLayout>,
    pub ai_turn_state: AiTurnState,
    pub ai_default_strategy: AiStrategyKind,
    pub ai_strategies: BTreeMap<TeamId, AiStrategyKind>,
    pub turn_start_overlay: bool,
    pub pending_defeat_skip: bool,
    pub defeated_teams: BTreeSet<TeamId>,
}

struct AiTurnState {
    running: bool,
    active_team: Option<TeamId>,
    pending_actions: VecDeque<AiAction>,
    timer: Timer,
    strategy_name: Option<&'static str>,
}

impl Default for AiTurnState {
    fn default() -> Self {
        Self {
            running: false,
            active_team: None,
            pending_actions: VecDeque::new(),
            timer: Timer::from_seconds(AI_STEP_SECONDS, TimerMode::Repeating),
            strategy_name: None,
        }
    }
}

impl AiTurnState {
    fn reset(&mut self) {
        self.running = false;
        self.active_team = None;
        self.pending_actions.clear();
        self.timer.reset();
        self.strategy_name = None;
    }
}

impl MapViewState {
    fn strategy_for_team(&self, team_id: TeamId) -> AiStrategyKind {
        self.ai_strategies.get(&team_id).copied().unwrap_or(self.ai_default_strategy)
    }

    pub fn load_state(&mut self, state: GameState) {
        self.state = Some(state);
        self.selected_hero_id = self.state.as_ref().and_then(select_hero);
        self.view_x = 0;
        self.view_y = 0;
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.status = None;
        self.ai_turn_state.reset();
        self.turn_start_overlay = false;
        self.pending_defeat_skip = false;
        self.defeated_teams.clear();
        self.sync_cursor_to_hero();
    }

    pub fn has_state(&self) -> bool {
        self.state.is_some()
    }

    pub fn get_game_state(&self) -> Option<&GameState> {
        self.state.as_ref()
    }

    pub fn get_game_state_mut(&mut self) -> Option<&mut GameState> {
        self.state.as_mut()
    }

    pub fn selected_hero_id(&self) -> Option<HeroId> {
        self.selected_hero_id
    }

    pub fn set_selected_hero_id(&mut self, id: HeroId) {
        self.selected_hero_id = Some(id);
    }

    pub fn selected_hero_position(&self) -> MapCoord {
        self.selected_hero_id
            .and_then(|id| self.state.as_ref()?.hero(id).map(|hero| *hero.get_position()))
            .unwrap_or(MapCoord::new(0, 0))
    }

    pub fn move_selected_hero(
        &mut self,
        direction: engine::Direction,
    ) -> Result<MapCoord, EngineError> {
        {
            let state = self.state.as_mut().ok_or_else(missing_state_error)?;
            let id = self.selected_hero_id.ok_or(EngineError::NoSelectedHero)?;
            state.move_hero(id, direction)?;
        }
        Ok(self.selected_hero_position())
    }

    pub fn place_resource_rod(&mut self) -> Result<MapCoord, EngineError> {
        {
            let state = self.state.as_mut().ok_or_else(missing_state_error)?;
            let id = self.selected_hero_id.ok_or(EngineError::NoSelectedHero)?;
            state.place_resource_rod(id)?;
        }
        Ok(self.selected_hero_position())
    }

    pub fn cycle_selected_hero(&mut self) {
        let Some(id) = self.selected_hero_id else {
            return;
        };
        let Some(state) = self.state.as_mut() else {
            return;
        };
        let player_team = state.hero(id).map(|h| h.get_team_id());
        if let Some(team_id) = player_team
            && let Some(next) = state.get_next_hero(team_id)
        {
            self.selected_hero_id = Some(next);
            state.set_active_hero(team_id, Some(next));
        }
    }

    pub fn end_turn(&mut self) -> Result<TurnStartReport, EngineError> {
        let report = {
            let state = self.state.as_mut().ok_or_else(missing_state_error)?;
            let next_team = state.get_next_active_team()?;
            let report = state.on_turn()?;
            self.selected_hero_id = state.get_next_hero(next_team);
            if let Some(next) = self.selected_hero_id {
                state.set_active_hero(next_team, Some(next));
            }
            report
        };
        Ok(report)
    }

    pub fn summary(&self) -> String {
        let Some(state) = self.state.as_ref() else {
            return "No game loaded".to_string();
        };
        let Some(id) = self.selected_hero_id else {
            return "No hero – hire one at a city entrance".to_string();
        };
        let Some(hero) = state.hero(id) else {
            return "?".to_string();
        };
        let team_heroes = state.get_team_alive_heroes_ids(hero.get_team_id());
        let hero_index =
            team_heroes.iter().position(|&hid| hid == id).unwrap_or(0).saturating_add(1);
        let total_team = team_heroes.len();

        format!(
            "{} ({}/{}) MOV:{}/{} HP:{}/{} @{},{}",
            hero.get_name(),
            hero_index,
            total_team,
            hero.get_mov_remaining(),
            hero.get_mov(),
            hero.get_hp(),
            hero.get_max_hp(),
            hero.get_position().x,
            hero.get_position().y
        )
    }

    pub fn cursor_coord(&self) -> Option<MapCoord> {
        if self.cursor_x < 0 || self.cursor_y < 0 {
            return None;
        }
        Some(MapCoord::new(self.cursor_x as u32, self.cursor_y as u32))
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub fn set_status(&mut self, status: Option<String>) {
        self.status = status;
    }

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

    pub fn sync_cursor_to_hero(&mut self) {
        let Some(state) = self.state.as_ref() else {
            return;
        };
        if let Some(id) = self.selected_hero_id
            && let Some(hero) = state.hero(id)
        {
            self.cursor_x = hero.get_position().x as isize;
            self.cursor_y = hero.get_position().y as isize;
            return;
        }
        if let Ok(team_id) = state.get_active_team_id() {
            if let Some(coord) = state.city_entrance_for_team(*team_id) {
                self.cursor_x = coord.x as isize;
                self.cursor_y = coord.y as isize;
                return;
            }
            if let Some(coord) = state.city_owner_for_team(*team_id) {
                self.cursor_x = coord.x as isize;
                self.cursor_y = coord.y as isize;
                return;
            }
        }
        self.cursor_x = 0;
        self.cursor_y = 0;
    }

    pub fn clamp_cursor(&mut self) {
        let Some(state) = self.state.as_ref() else {
            return;
        };
        let w = state.map.tile_width() as isize;
        let h = state.map.tile_height() as isize;
        self.cursor_x = self.cursor_x.clamp(0, w - 1);
        self.cursor_y = self.cursor_y.clamp(0, h - 1);
    }

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

    pub fn find_city_entrance_at_cursor(&self) -> Option<MapCoord> {
        let state = self.state.as_ref()?;
        let coord = MapCoord::new(self.cursor_x.max(0) as u32, self.cursor_y.max(0) as u32);
        let city_tiles = engine::state_flood::flood_city(&state.map, coord);
        let cursor = MapCoord::new(coord.x, coord.y);
        city_tiles
            .iter()
            .filter(|c| {
                state.map.get_tile(**c).map(|t| t.kind == Tiles::CityEntrance).unwrap_or(false)
            })
            .min_by_key(|c| {
                (c.x as i32).abs_diff(cursor.x as i32) + (c.y as i32).abs_diff(cursor.y as i32)
            })
            .copied()
    }

    pub fn cursor_structure(&self) -> Option<StructureInfo> {
        let state = self.state.as_ref()?;
        let map = &state.map;
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

    pub fn cursor_hero_id(&self) -> Option<HeroId> {
        let coord = self.cursor_coord()?;
        self.state.as_ref()?.hero_at(&coord).map(|hero| hero.get_id())
    }

    fn attack_cursor_hero(&mut self) -> Result<(), EngineError> {
        let attacker_id = self.selected_hero_id.ok_or(EngineError::NoSelectedHero)?;
        let coord = self.cursor_coord().ok_or(EngineError::NoTargetCoord)?;
        let defender_id = {
            let state = self.state.as_ref().ok_or_else(missing_state_error)?;
            state.can_attack(attacker_id, coord)?
        };
        let state = self.state.as_mut().ok_or_else(missing_state_error)?;
        let events = state.attack_hero(attacker_id, defender_id)?;
        let defeated = events
            .iter()
            .any(|event| matches!(event, engine::game_state::TurnEvent::HeroDefeated { .. }));
        let status = if defeated { "Combat resolved (defeat)" } else { "Combat resolved" };
        self.status = Some(status.to_string());
        Ok(())
    }

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
                self.cycle_selected_hero();
                self.sync_cursor_to_hero();
                self.center_on_hero(visible_cols, visible_rows);
                self.status = Some(self.summary());
                MapViewOutcome::Changed
            }
            InputEvent::PlaceRod => match self.place_resource_rod() {
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
                    let can_attack = self
                        .state
                        .as_ref()
                        .and_then(|state| {
                            let attacker_id = self.selected_hero_id?;
                            let coord = self.cursor_coord()?;
                            state.can_attack(attacker_id, coord).ok()
                        })
                        .is_some();
                    if can_attack {
                        match self.attack_cursor_hero() {
                            Ok(()) => MapViewOutcome::Changed,
                            Err(e) => {
                                self.status = Some(e.to_string());
                                MapViewOutcome::Changed
                            }
                        }
                    } else {
                        MapViewOutcome::OpenHeroInfo
                    }
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
        let Some(state) = self.state.as_ref() else {
            return MapViewOutcome::NoChange;
        };
        let map = &state.map;
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
        let Some(state) = self.state.as_ref() else {
            return false;
        };
        let w = state.map.tile_width() as isize;
        let h = state.map.tile_height() as isize;
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
        let Some(state) = self.state.as_ref() else {
            return false;
        };
        let coord = MapCoord::new(self.cursor_x as u32, self.cursor_y as u32);
        state.map.get_tile(coord).map(|tile| matches!(tile.kind, Tiles::City)).unwrap_or(false)
    }

    fn move_cursor_out_of_city(&mut self, event: InputEvent) -> bool {
        let (dx, dy) = match event {
            InputEvent::CursorUp => (0, -1),
            InputEvent::CursorDown => (0, 1),
            InputEvent::CursorLeft => (-1, 0),
            InputEvent::CursorRight => (1, 0),
            _ => return false,
        };
        let Some(state) = self.state.as_ref() else {
            return false;
        };
        let map = &state.map;
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
            InputEvent::Up => Some(engine::Direction::North),
            InputEvent::Down => Some(engine::Direction::South),
            InputEvent::Left => Some(engine::Direction::West),
            InputEvent::Right => Some(engine::Direction::East),
            _ => None,
        };

        let Some(direction) = direction else {
            return MapViewOutcome::NoChange;
        };

        match self.move_selected_hero(direction) {
            Ok(_position) => {
                self.status = Some(self.summary());
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

    pub fn center_on_hero(&mut self, visible_cols: usize, visible_rows: usize) -> bool {
        let Some(state) = self.state.as_ref() else {
            return false;
        };
        let cx = self.cursor_x.max(0) as usize;
        let cy = self.cursor_y.max(0) as usize;

        let map = &state.map;
        let max_x = map.tile_width().saturating_sub(visible_cols as u32) as usize;
        let max_y = map.tile_height().saturating_sub(visible_rows as u32) as usize;

        let target_x = if cx >= visible_cols / 2 { (cx - visible_cols / 2).min(max_x) } else { 0 };
        let target_y = if cy >= visible_rows / 2 { (cy - visible_rows / 2).min(max_y) } else { 0 };

        let changed = self.view_x != target_x || self.view_y != target_y;
        self.view_x = target_x;
        self.view_y = target_y;
        changed
    }

    pub fn focus_on_team_city(
        &mut self,
        team_id: TeamId,
        visible_cols: usize,
        visible_rows: usize,
    ) -> bool {
        let Some(state) = self.state.as_ref() else {
            return false;
        };
        let coord =
            state.city_entrance_for_team(team_id).or_else(|| state.city_owner_for_team(team_id));
        if let Some(coord) = coord {
            self.cursor_x = coord.x as isize;
            self.cursor_y = coord.y as isize;
            self.scroll_cursor_into_view(visible_cols, visible_rows);
            return self.center_on_hero(visible_cols, visible_rows);
        }
        self.sync_cursor_to_hero();
        self.center_on_hero(visible_cols, visible_rows)
    }
}

const TEXT_COLOR: Color = Color::srgb(0.85, 0.85, 0.88);
const FOOTER_COLOR: Color = Color::srgb(0.5, 0.5, 0.55);
const OVERLAY_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.7);
const OVERLAY_PANEL_BG: Color = Color::srgb(0.18, 0.18, 0.24);
const OVERLAY_PANEL_BORDER: Color = Color::srgb(0.5, 0.5, 0.6);

// Button theme (matches splash.rs)
const BTN_BG: Color = Color::srgb(0.14, 0.14, 0.18);
const BTN_BG_HOVER: Color = Color::srgb(0.22, 0.22, 0.28);
const BTN_BG_SELECTED: Color = Color::srgb(0.28, 0.28, 0.35);
const BTN_BG_PRESSED: Color = Color::srgb(0.35, 0.35, 0.42);
const BTN_BORDER: Color = Color::srgb(0.4, 0.4, 0.48);
const BTN_BORDER_HOVER: Color = Color::srgb(0.55, 0.55, 0.62);
const BTN_BORDER_SELECTED: Color = Color::srgb(0.7, 0.7, 0.78);
const BTN_BORDER_PRESSED: Color = Color::srgb(0.65, 0.65, 0.72);

const RESOURCE_ROD_ATLAS_INDEX: usize = 344;
const AI_STEP_SECONDS: f32 = 0.3;

/// Fallback color for heroes whose team is not found.
const HERO_COLOR: Color = Color::srgb(1.0, 1.0, 1.0);
const NEUTRAL_RESOURCE_COLOR: Color = Color::srgb(1.0, 1.0, 1.0);

pub struct MapViewPlugin;

impl Plugin for MapViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapViewState>()
            .add_systems(OnEnter(AppState::MapView), enter_map_view)
            .add_systems(OnExit(AppState::MapView), exit_map_view)
            .add_systems(Update, update_map_view.run_if(in_state(AppState::MapView)));
    }
}

impl Default for MapViewState {
    fn default() -> Self {
        Self {
            state: None,
            selected_hero_id: None,
            view_x: 0,
            view_y: 0,
            cursor_x: 0,
            cursor_y: 0,
            status: None,
            tile_size: 32.0,
            visible_cols: 0,
            visible_rows: 0,
            needs_initial_draw: true,
            end_turn_overlay: false,
            end_turn_selected: 0,
            pause_overlay: false,
            pause_selected: 0,
            last_mouse_tile: None,
            atlas_image: Handle::default(),
            atlas_layout: Handle::default(),
            ai_turn_state: AiTurnState::default(),
            ai_default_strategy: AiStrategyKind::ResourceRush,
            ai_strategies: BTreeMap::new(),
            turn_start_overlay: false,
            pending_defeat_skip: false,
            defeated_teams: BTreeSet::new(),
        }
    }
}

fn missing_state_error() -> EngineError {
    EngineError::NoGameStateLoaded
}

fn select_hero(state: &GameState) -> Option<HeroId> {
    let active_team = state.get_active_team_id().ok().copied();
    // Do NOT fall back to AI heroes — if the player team has no heroes,
    // selected_hero_id stays None and the UI shows "hire hero" prompt.
    active_team
        .and_then(|team_id| state.get_active_hero(team_id))
        .or_else(|| active_team.and_then(|team_id| state.get_next_hero(team_id)))
        .or_else(|| state.living_heroes(true).first().map(|hero| hero.get_id()))
}

/// Color used for tiles of a neutral (unowned) city.
const NEUTRAL_CITY_COLOR: Color = Color::srgb(1.0, 0.0, 1.0); // magenta

fn tile_color_for(kind: Tiles, tile_config: &TileConfig) -> Color {
    let (r, g, b) = kind.color_with_config(tile_config);
    rgb_color(r, g, b)
}

fn rgb_color(r: u8, g: u8, b: u8) -> Color {
    Color::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

fn tile_atlas_index(kind: Tiles, tile_config: &TileConfig) -> usize {
    kind.atlas_index_with_config(tile_config) as usize
}

fn resource_atlas_index(kind: ResourceKind) -> usize {
    match kind {
        ResourceKind::Resource1 => 1089,
        ResourceKind::Resource2 => 1092,
        ResourceKind::Resource3 => 1093,
        ResourceKind::Resource4 => 1094,
        ResourceKind::GoldMine => 1091,
    }
}

/// Common button node style for overlay buttons.
fn button_node(w: f32, h: f32) -> Node {
    Node {
        width: Val::Px(w),
        height: Val::Px(h),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(2.0)),
        ..default()
    }
}

/// Atlas index for the cursor overlay sprite.
const CURSOR_ATLAS_INDEX: usize = 624;
const CURSOR_OVERLAY_COLOR: Color = Color::srgb(1.0, 1.0, 0.47);
const CITY_CURSOR_SCALE: f32 = 3.0;
const CITY_CURSOR_Z: f32 = 0.5;

#[derive(Component)]
pub struct CursorOverlay;

#[derive(Component)]
pub struct MapTile;

#[derive(Component)]
pub struct LandOwnerTile;

#[derive(Component)]
pub struct ResourceRodTile;

/// Overlay sprite that draws the owning team's logo on a city core tile.
#[derive(Component)]
pub struct CityLogoTile;

#[derive(Component)]
pub struct MapTilePos {
    pub col: usize,
    pub row: usize,
}

fn enter_map_view(
    commands: Commands,
    map_view_state: ResMut<MapViewState>,
    loaded: Option<ResMut<LoadedSession>>,
    window: Single<&Window>,
    atlas: Res<TileAtlas>,
) {
    enter_map_view_impl(commands, map_view_state, loaded, window, atlas);
}

fn is_city_core_tile(kind: Tiles) -> bool {
    matches!(kind, Tiles::City)
}

/// Returns the single centre cell of every owned city.
///
/// `set_city_owner` floods the whole connected city block, so ownership covers
/// every city tile. To draw a team logo on just one cell, we flood each city
/// once and pick the core (`City`) cell nearest the block's centroid.
fn owned_city_centers(
    state: &engine::game_state::GameState,
) -> std::collections::HashSet<MapCoord> {
    use std::collections::HashSet;
    let map = &state.map;
    let mut visited: HashSet<MapCoord> = HashSet::new();
    let mut centers: HashSet<MapCoord> = HashSet::new();
    for &coord in state.city_owners.keys() {
        if !visited.insert(coord) {
            continue;
        }
        let cells = engine::state_flood::flood_city(map, coord);
        for &c in &cells {
            visited.insert(c);
        }
        let cores: Vec<MapCoord> = cells
            .iter()
            .copied()
            .filter(|c| map.get_tile(*c).map(|t| is_city_core_tile(t.kind)).unwrap_or(false))
            .collect();
        if cores.is_empty() {
            continue;
        }
        let cx = cores.iter().map(|c| c.x as u64).sum::<u64>() / cores.len() as u64;
        let cy = cores.iter().map(|c| c.y as u64).sum::<u64>() / cores.len() as u64;
        if let Some(center) = cores.iter().copied().min_by_key(|c| {
            let dx = c.x as i64 - cx as i64;
            let dy = c.y as i64 - cy as i64;
            dx * dx + dy * dy
        }) {
            // Drop the logo one cell below the centroid when that cell is still
            // part of the city — it reads better than sitting at the top.
            let below = MapCoord::new(center.x, center.y + 1);
            let center = if cores.contains(&below) { below } else { center };
            centers.insert(center);
        }
    }
    centers
}

fn mouse_visible_tile(
    window: &Window,
    cursor_position: Vec2,
    tile_size: f32,
    visible_cols: usize,
    visible_rows: usize,
) -> Option<(usize, usize)> {
    let total_w = visible_cols as f32 * tile_size;
    let total_h = visible_rows as f32 * tile_size;
    let left = (window.width() - total_w) * 0.5;
    let top = (window.height() - total_h) * 0.5;
    let local_x = cursor_position.x - left;
    let local_y = cursor_position.y - top;

    if local_x < 0.0 || local_y < 0.0 || local_x >= total_w || local_y >= total_h {
        return None;
    }

    Some(((local_x / tile_size) as usize, (local_y / tile_size) as usize))
}

fn enter_map_view_impl(
    mut commands: Commands,
    mut map_view_state: ResMut<MapViewState>,
    mut loaded: Option<ResMut<LoadedSession>>,
    window: Single<&Window>,
    atlas: Res<TileAtlas>,
) {
    if !map_view_state.has_state() {
        let Some(mut loaded) = loaded.take() else {
            return;
        };

        let Some(state) = loaded.state.take() else {
            return;
        };

        map_view_state.load_state(state);
    }

    spawn_map_view_entities(&mut commands, &mut map_view_state, &window, &atlas);

    // Center camera on the selected hero on first entry.
    // Must run after spawn_map_view_entities which computes visible_cols/rows.
    let vc = map_view_state.visible_cols;
    let vr = map_view_state.visible_rows;
    map_view_state.sync_cursor_to_hero();
    map_view_state.center_on_hero(vc, vr);
}

fn spawn_map_view_entities(
    commands: &mut Commands,
    map_view_state: &mut MapViewState,
    window: &Window,
    atlas: &TileAtlas,
) {
    let tile_size = map_view_state.tile_size;
    let map_h = window.height() - 40.0;
    let visible_cols = (window.width() / tile_size).max(1.0) as usize;
    let visible_rows = (map_h / tile_size).max(1.0) as usize;

    map_view_state.visible_cols = visible_cols;
    map_view_state.visible_rows = visible_rows;
    map_view_state.needs_initial_draw = true;
    map_view_state.last_mouse_tile = None;

    let total_w = visible_cols as f32 * tile_size;
    let total_h = visible_rows as f32 * tile_size;
    let offset_x = -total_w / 2.0 + tile_size / 2.0;
    let offset_y = total_h / 2.0 - tile_size / 2.0;

    let atlas_handle = atlas.image.clone();
    let layout_handle = atlas.layout.clone();

    // Store handles in MapViewState so other systems (e.g. hero selection)
    // can reuse the same atlas.
    map_view_state.atlas_image = atlas_handle.clone();
    map_view_state.atlas_layout = layout_handle.clone();

    for row in 0..visible_rows {
        for col in 0..visible_cols {
            let x = offset_x + col as f32 * tile_size;
            let y = offset_y - row as f32 * tile_size;
            commands.spawn((
                Sprite {
                    color: Color::NONE,
                    custom_size: Some(Vec2::splat(tile_size)),
                    ..Default::default()
                },
                Transform::from_xyz(x, y, -0.1),
                LandOwnerTile,
                MapTilePos { col, row },
            ));

            commands.spawn((
                Sprite {
                    image: atlas_handle.clone(),
                    texture_atlas: Some(TextureAtlas { layout: layout_handle.clone(), index: 0 }),
                    custom_size: Some(Vec2::splat(tile_size)),
                    ..Default::default()
                },
                Transform::from_xyz(x, y, 0.0),
                MapTile,
                MapTilePos { col, row },
            ));

            commands.spawn((
                Sprite {
                    image: atlas_handle.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: layout_handle.clone(),
                        index: RESOURCE_ROD_ATLAS_INDEX,
                    }),
                    color: Color::NONE,
                    custom_size: Some(Vec2::splat(tile_size)),
                    ..Default::default()
                },
                Transform::from_xyz(x, y, 0.2),
                ResourceRodTile,
                MapTilePos { col, row },
            ));

            commands.spawn((
                Sprite {
                    image: atlas_handle.clone(),
                    texture_atlas: Some(TextureAtlas { layout: layout_handle.clone(), index: 0 }),
                    color: Color::NONE,
                    custom_size: Some(Vec2::splat(tile_size)),
                    ..Default::default()
                },
                Transform::from_xyz(x, y, 0.25),
                CityLogoTile,
                MapTilePos { col, row },
            ));
        }
    }

    // Cursor overlay sprite — drawn on top of all tiles at z=1.
    commands.spawn((
        Sprite {
            image: atlas_handle.clone(),
            texture_atlas: Some(TextureAtlas {
                layout: layout_handle.clone(),
                index: CURSOR_ATLAS_INDEX,
            }),
            color: CURSOR_OVERLAY_COLOR,
            custom_size: Some(Vec2::splat(tile_size)),
            ..Default::default()
        },
        Transform::from_xyz(offset_x, offset_y, 1.0),
        CursorOverlay,
    ));

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(40.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.14)),
            MapViewRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(TEXT_COLOR),
                StatusText,
            ));
        });

    let tile_config = map_view_state
        .get_game_state()
        .map(|state| state.tile_config().clone())
        .unwrap_or_default();
    spawn_top_bar(commands, atlas_handle, layout_handle, &tile_config);
}

/// Spawns the top HUD bar: current turn number, gold balance, and the four
/// resource stockpiles, each labelled with its atlas pictogram. The text
/// entities are tagged with [`TopBarField`] so [`update_map_view`] can refresh
/// their values every redraw.
fn spawn_top_bar(
    commands: &mut Commands,
    atlas_image: Handle<Image>,
    atlas_layout: Handle<TextureAtlasLayout>,
    tile_config: &TileConfig,
) {
    // One labelled cell: optional pictogram + value text tagged with `field`.
    fn cell(
        parent: &mut ChildSpawnerCommands,
        atlas_image: &Handle<Image>,
        atlas_layout: &Handle<TextureAtlasLayout>,
        icon: Option<usize>,
        field: TopBarField,
        color: Color,
    ) {
        parent
            .spawn((Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },))
            .with_children(|cell| {
                if let Some(index) = icon {
                    cell.spawn((
                        ImageNode {
                            image: atlas_image.clone(),
                            texture_atlas: Some(TextureAtlas {
                                layout: atlas_layout.clone(),
                                index,
                            }),
                            ..default()
                        },
                        Node { width: Val::Px(18.0), height: Val::Px(18.0), ..default() },
                    ));
                }
                cell.spawn((
                    Text::new("0"),
                    TextFont { font_size: FontSize::Px(15.0), ..default() },
                    TextColor(color),
                    field,
                ));
            });
    }

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(32.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                column_gap: Val::Px(18.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.14)),
            TopBarRoot,
        ))
        .with_children(|parent| {
            let resource_icons = resource_icon_indices(tile_config);
            // Turn number (no icon, plain text label baked into the value).
            cell(parent, &atlas_image, &atlas_layout, None, TopBarField::Turn, TEXT_COLOR);
            // Gold balance.
            cell(
                parent,
                &atlas_image,
                &atlas_layout,
                Some(gold_icon_index(tile_config)),
                TopBarField::Gold,
                GOLD_COLOR,
            );
            // Four resource stockpiles.
            for (idx, &icon) in resource_icons.iter().enumerate() {
                cell(
                    parent,
                    &atlas_image,
                    &atlas_layout,
                    Some(icon),
                    TopBarField::Resource(idx),
                    TEXT_COLOR,
                );
            }
        });
}

fn spawn_end_turn_overlay(commands: &mut Commands, team_name: &str) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(OVERLAY_BG),
            EndTurnOverlay,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        width: Val::Px(400.0),
                        height: Val::Px(260.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(20.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(OVERLAY_PANEL_BG),
                    BorderColor::all(OVERLAY_PANEL_BORDER),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new(format!("End turn for {}?", team_name)),
                        TextFont { font_size: FontSize::Px(22.0), ..default() },
                        TextColor(TEXT_COLOR),
                    ));
                    // Confirm button (selected by default)
                    panel.spawn((
                        Button,
                        EndTurnConfirmButton,
                        button_node(200.0, 50.0),
                        BackgroundColor(BTN_BG_SELECTED),
                        BorderColor::all(BTN_BORDER_SELECTED),
                        children![(
                            Text::new("End Turn"),
                            TextFont { font_size: FontSize::Px(20.0), ..default() },
                            TextColor(TEXT_COLOR),
                        )],
                    ));
                    // Cancel button
                    panel.spawn((
                        Button,
                        EndTurnCancelButton,
                        button_node(200.0, 50.0),
                        BackgroundColor(BTN_BG),
                        BorderColor::all(BTN_BORDER),
                        children![(
                            Text::new("Cancel"),
                            TextFont { font_size: FontSize::Px(20.0), ..default() },
                            TextColor(TEXT_COLOR),
                        )],
                    ));
                    panel.spawn((
                        Text::new("W/S: navigate  Enter: select  Esc: cancel"),
                        TextFont { font_size: FontSize::Px(12.0), ..default() },
                        TextColor(FOOTER_COLOR),
                    ));
                });
        });
}

fn spawn_turn_start_overlay(commands: &mut Commands, team_name: &str, income: TurnIncome) {
    let resources = income.resources;
    let resource_line = format!(
        "Resources: +{} / +{} / +{} / +{}",
        resources[0], resources[1], resources[2], resources[3]
    );
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(OVERLAY_BG),
            TurnStartOverlay,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        width: Val::Px(420.0),
                        height: Val::Px(240.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(14.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(OVERLAY_PANEL_BG),
                    BorderColor::all(OVERLAY_PANEL_BORDER),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new(format!("Turn start: {team_name}")),
                        TextFont { font_size: FontSize::Px(22.0), ..default() },
                        TextColor(TEXT_COLOR),
                    ));
                    panel.spawn((
                        Text::new(format!("Income: +{} gold", income.gold)),
                        TextFont { font_size: FontSize::Px(18.0), ..default() },
                        TextColor(GOLD_COLOR),
                    ));
                    panel.spawn((
                        Text::new(resource_line),
                        TextFont { font_size: FontSize::Px(16.0), ..default() },
                        TextColor(TEXT_COLOR),
                    ));
                    panel.spawn((
                        Button,
                        TurnStartConfirmButton,
                        button_node(200.0, 50.0),
                        BackgroundColor(BTN_BG_SELECTED),
                        BorderColor::all(BTN_BORDER_SELECTED),
                        children![(
                            Text::new("Start"),
                            TextFont { font_size: FontSize::Px(18.0), ..default() },
                            TextColor(TEXT_COLOR),
                        )],
                    ));
                });
        });
}

fn spawn_turn_defeat_overlay(commands: &mut Commands, team_name: &str) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(OVERLAY_BG),
            TurnStartOverlay,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        width: Val::Px(420.0),
                        height: Val::Px(220.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(14.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(OVERLAY_PANEL_BG),
                    BorderColor::all(OVERLAY_PANEL_BORDER),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new(format!("Team {team_name} defeated")),
                        TextFont { font_size: FontSize::Px(22.0), ..default() },
                        TextColor(TEXT_COLOR),
                    ));
                    panel.spawn((
                        Text::new("No cities and no heroes left."),
                        TextFont { font_size: FontSize::Px(16.0), ..default() },
                        TextColor(TEXT_COLOR),
                    ));
                    panel.spawn((
                        Button,
                        TurnStartConfirmButton,
                        button_node(200.0, 50.0),
                        BackgroundColor(BTN_BG_SELECTED),
                        BorderColor::all(BTN_BORDER_SELECTED),
                        children![(
                            Text::new("Continue"),
                            TextFont { font_size: FontSize::Px(18.0), ..default() },
                            TextColor(TEXT_COLOR),
                        )],
                    ));
                });
        });
}

fn spawn_pause_overlay(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(OVERLAY_BG),
            PauseOverlay,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        width: Val::Px(400.0),
                        height: Val::Px(300.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(20.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(OVERLAY_PANEL_BG),
                    BorderColor::all(OVERLAY_PANEL_BORDER),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("PAUSED"),
                        TextFont { font_size: FontSize::Px(28.0), ..default() },
                        TextColor(TEXT_COLOR),
                    ));
                    // Resume button (selected by default)
                    panel.spawn((
                        Button,
                        PauseResumeButton,
                        button_node(200.0, 50.0),
                        BackgroundColor(BTN_BG_SELECTED),
                        BorderColor::all(BTN_BORDER_SELECTED),
                        children![(
                            Text::new("Resume"),
                            TextFont { font_size: FontSize::Px(20.0), ..default() },
                            TextColor(TEXT_COLOR),
                        )],
                    ));
                    // Quit button
                    panel.spawn((
                        Button,
                        PauseQuitButton,
                        button_node(200.0, 50.0),
                        BackgroundColor(BTN_BG),
                        BorderColor::all(BTN_BORDER),
                        children![(
                            Text::new("Quit to Menu"),
                            TextFont { font_size: FontSize::Px(20.0), ..default() },
                            TextColor(TEXT_COLOR),
                        )],
                    ));
                    panel.spawn((
                        Text::new("W/S: navigate  Enter: select  Esc: resume"),
                        TextFont { font_size: FontSize::Px(12.0), ..default() },
                        TextColor(FOOTER_COLOR),
                    ));
                });
        });
}

/// Update button visual state based on selection and interaction.
#[allow(clippy::type_complexity)]
fn update_button_style(
    is_selected: bool,
    interaction: &Interaction,
    bg: &mut BackgroundColor,
    border: &mut BorderColor,
) {
    let hovered = matches!(interaction, Interaction::Hovered);
    let pressed = matches!(interaction, Interaction::Pressed);
    if pressed {
        *bg = BackgroundColor(BTN_BG_PRESSED);
        *border = BorderColor::all(BTN_BORDER_PRESSED);
    } else if is_selected {
        *bg = BackgroundColor(BTN_BG_SELECTED);
        *border = BorderColor::all(BTN_BORDER_SELECTED);
    } else if hovered {
        *bg = BackgroundColor(BTN_BG_HOVER);
        *border = BorderColor::all(BTN_BORDER_HOVER);
    } else {
        *bg = BackgroundColor(BTN_BG);
        *border = BorderColor::all(BTN_BORDER);
    }
}

fn handle_ai_turn(
    commands: &mut Commands,
    map_view_state: &mut MapViewState,
    time: &Time,
    visible_cols: usize,
    visible_rows: usize,
) -> bool {
    let Some(state) = map_view_state.state.as_ref() else {
        map_view_state.ai_turn_state.reset();
        return false;
    };
    let Ok(&team_id) = state.get_active_team_id() else {
        map_view_state.ai_turn_state.reset();
        return false;
    };
    if state.is_player_controlled(team_id) {
        map_view_state.ai_turn_state.reset();
        return false;
    }

    if !map_view_state.ai_turn_state.running
        || map_view_state.ai_turn_state.active_team != Some(team_id)
    {
        let ctx = AiContext { team_id, state };
        let strategy_kind = map_view_state.strategy_for_team(team_id);
        let factory = AiFactory::new(strategy_kind);
        let mut strategy = factory.build(team_id);
        let strategy_name = strategy.name();
        let actions = strategy.plan(&ctx);
        map_view_state.ai_turn_state.running = true;
        map_view_state.ai_turn_state.active_team = Some(team_id);
        map_view_state.ai_turn_state.pending_actions = VecDeque::from(actions);
        map_view_state.ai_turn_state.timer.reset();
        map_view_state.ai_turn_state.strategy_name = Some(strategy_name);
        let team_name = state.team_name_by_id(team_id).unwrap_or("CPU");
        map_view_state.set_status(Some(format!("CPU {team_name}: {strategy_name}")));
    }

    map_view_state.ai_turn_state.timer.tick(time.delta());
    if map_view_state.ai_turn_state.timer.just_finished() {
        if let Some(action) = map_view_state.ai_turn_state.pending_actions.pop_front() {
            let ended = execute_ai_action(
                commands,
                map_view_state,
                team_id,
                action,
                visible_cols,
                visible_rows,
            );
            if ended {
                map_view_state.ai_turn_state.reset();
            }
        } else {
            finish_ai_turn(commands, map_view_state, visible_cols, visible_rows);
            map_view_state.ai_turn_state.reset();
        }
    }

    true
}

fn execute_ai_action(
    commands: &mut Commands,
    map_view_state: &mut MapViewState,
    team_id: TeamId,
    action: AiAction,
    visible_cols: usize,
    visible_rows: usize,
) -> bool {
    let mut handled = false;
    let mut ended_turn = false;
    match action {
        AiAction::Move { hero_id, direction } => {
            map_view_state.set_selected_hero_id(hero_id);
            if let Some(state) = map_view_state.get_game_state_mut() {
                state.set_active_hero(team_id, Some(hero_id));
            }
            match map_view_state.move_selected_hero(direction) {
                Ok(_) => handled = true,
                Err(e) => map_view_state.set_status(Some(e.to_string())),
            }
        }
        AiAction::PlaceRod { hero_id } => {
            map_view_state.set_selected_hero_id(hero_id);
            if let Some(state) = map_view_state.get_game_state_mut() {
                state.set_active_hero(team_id, Some(hero_id));
            }
            match map_view_state.place_resource_rod() {
                Ok(_) => handled = true,
                Err(e) => map_view_state.set_status(Some(e.to_string())),
            }
        }
        AiAction::HireHero { candidate_idx, coord } => {
            let mut hired_id = None;
            if let Some(state) = map_view_state.get_game_state_mut() {
                if let Some(candidate) = state.get_hero_candidate_at(candidate_idx).cloned() {
                    match state.hire_hero(&candidate, &coord) {
                        Ok(hero_id) => {
                            state.set_active_hero(team_id, Some(hero_id));
                            hired_id = Some(hero_id);
                        }
                        Err(e) => map_view_state.set_status(Some(e.to_string())),
                    }
                }
            }
            if let Some(hero_id) = hired_id {
                map_view_state.set_selected_hero_id(hero_id);
                handled = true;
            }
        }
        AiAction::Attack { attacker_id, defender_id } => {
            map_view_state.set_selected_hero_id(attacker_id);
            if let Some(state) = map_view_state.get_game_state_mut() {
                state.set_active_hero(team_id, Some(attacker_id));
                match state.attack_hero(attacker_id, defender_id) {
                    Ok(_) => handled = true,
                    Err(e) => map_view_state.set_status(Some(e.to_string())),
                }
            }
        }
        AiAction::EndTurn => {
            finish_ai_turn(commands, map_view_state, visible_cols, visible_rows);
            ended_turn = true;
        }
    }

    if handled {
        map_view_state.sync_cursor_to_hero();
        map_view_state.center_on_hero(visible_cols, visible_rows);
        map_view_state.needs_initial_draw = true;
    }

    ended_turn
}

fn finish_ai_turn(
    commands: &mut Commands,
    map_view_state: &mut MapViewState,
    visible_cols: usize,
    visible_rows: usize,
) {
    advance_turn_with_skips(commands, map_view_state, visible_cols, visible_rows);
}

fn advance_turn_with_skips(
    commands: &mut Commands,
    map_view_state: &mut MapViewState,
    visible_cols: usize,
    visible_rows: usize,
) {
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 32 {
            map_view_state.set_status(Some("Turn skip loop detected".to_string()));
            break;
        }
        match map_view_state.end_turn() {
            Ok(report) => {
                map_view_state.end_turn_overlay = false;
                let action =
                    handle_turn_start(commands, map_view_state, report, visible_cols, visible_rows);
                if action == TurnStartAction::Skip {
                    continue;
                }
                break;
            }
            Err(e) => {
                map_view_state.set_status(Some(e.to_string()));
                break;
            }
        }
    }
}

#[derive(PartialEq, Eq)]
enum TurnStartAction {
    Done,
    Skip,
}

fn handle_turn_start(
    commands: &mut Commands,
    map_view_state: &mut MapViewState,
    report: TurnStartReport,
    visible_cols: usize,
    visible_rows: usize,
) -> TurnStartAction {
    let (team_name, is_player, defeated) = match map_view_state.state.as_ref() {
        Some(state) => (
            state.team_name_by_id(report.team_id).unwrap_or("Unknown").to_string(),
            state.is_player_controlled(report.team_id),
            state.is_team_defeated(report.team_id),
        ),
        None => return TurnStartAction::Done,
    };
    let team_id = report.team_id;

    map_view_state.focus_on_team_city(team_id, visible_cols, visible_rows);
    map_view_state.needs_initial_draw = true;
    map_view_state.set_status(Some(map_view_state.summary()));

    if defeated {
        let already_notified = map_view_state.defeated_teams.contains(&team_id);
        map_view_state.defeated_teams.insert(team_id);
        if is_player && !already_notified {
            map_view_state.turn_start_overlay = true;
            map_view_state.pending_defeat_skip = true;
            spawn_turn_defeat_overlay(commands, &team_name);
            return TurnStartAction::Done;
        }
        return TurnStartAction::Skip;
    }

    if is_player {
        map_view_state.turn_start_overlay = true;
        spawn_turn_start_overlay(commands, &team_name, report.income);
    } else {
        map_view_state.turn_start_overlay = false;
    }
    TurnStartAction::Done
}

/// Per-tile sprite layers plus the assets needed to draw team logos, bundled to
/// keep [`update_map_view`] under the system parameter limit.
#[allow(clippy::type_complexity)]
#[derive(SystemParam)]
struct TileLayers<'w, 's> {
    tiles: Query<
        'w,
        's,
        (&'static MapTilePos, &'static mut Sprite),
        (
            With<MapTile>,
            Without<LandOwnerTile>,
            Without<ResourceRodTile>,
            Without<CityLogoTile>,
            Without<CursorOverlay>,
        ),
    >,
    land: Query<
        'w,
        's,
        (&'static MapTilePos, &'static mut Sprite),
        (
            With<LandOwnerTile>,
            Without<MapTile>,
            Without<ResourceRodTile>,
            Without<CityLogoTile>,
            Without<CursorOverlay>,
        ),
    >,
    rod: Query<
        'w,
        's,
        (&'static MapTilePos, &'static mut Sprite),
        (
            With<ResourceRodTile>,
            Without<MapTile>,
            Without<LandOwnerTile>,
            Without<CityLogoTile>,
            Without<CursorOverlay>,
        ),
    >,
    logo: Query<
        'w,
        's,
        (&'static MapTilePos, &'static mut Sprite),
        (
            With<CityLogoTile>,
            Without<MapTile>,
            Without<LandOwnerTile>,
            Without<ResourceRodTile>,
            Without<CursorOverlay>,
        ),
    >,
    atlas: Res<'w, TileAtlas>,
    logo_images: ResMut<'w, TeamLogoImages>,
    images: ResMut<'w, Assets<Image>>,
}

#[allow(clippy::type_complexity)]
#[derive(SystemParam)]
struct OverlayQueries<'w, 's> {
    turn_start: Query<'w, 's, Entity, With<TurnStartOverlay>>,
    end_turn: Query<'w, 's, Entity, With<EndTurnOverlay>>,
    pause: Query<'w, 's, Entity, With<PauseOverlay>>,
    turn_start_buttons: Query<
        'w,
        's,
        (&'static Interaction, &'static mut BackgroundColor, &'static mut BorderColor),
        With<TurnStartConfirmButton>,
    >,
    buttons: Query<
        'w,
        's,
        (
            Option<&'static EndTurnConfirmButton>,
            Option<&'static EndTurnCancelButton>,
            Option<&'static PauseResumeButton>,
            Option<&'static PauseQuitButton>,
            &'static mut BackgroundColor,
            &'static mut BorderColor,
            &'static Interaction,
        ),
        Or<(
            With<EndTurnConfirmButton>,
            With<EndTurnCancelButton>,
            With<PauseResumeButton>,
            With<PauseQuitButton>,
        )>,
    >,
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
fn update_map_view(
    mut commands: Commands,
    mut map_view_state: ResMut<MapViewState>,
    mut next_state: ResMut<NextState<AppState>>,
    mut reader: MessageReader<UiAction>,
    mut status_query: Query<&mut Text, (With<StatusText>, Without<TopBarField>)>,
    mut top_bar_query: Query<(&mut Text, &TopBarField), Without<StatusText>>,
    mut layers: TileLayers,
    mut cursor_query: Query<
        (&mut Transform, &mut Sprite),
        (
            With<CursorOverlay>,
            Without<MapTile>,
            Without<LandOwnerTile>,
            Without<ResourceRodTile>,
            Without<CityLogoTile>,
        ),
    >,
    cooldown: Res<InputCooldown>,
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    mut overlays: OverlayQueries,
) {
    let visible_cols = map_view_state.visible_cols;
    let visible_rows = map_view_state.visible_rows;

    if !map_view_state.has_state() {
        return;
    }

    if handle_ai_turn(&mut commands, &mut map_view_state, &time, visible_cols, visible_rows) {
        return;
    }

    let actions: Vec<UiAction> = reader.read().copied().collect();
    let frame = |action: UiAction| actions.contains(&action);

    // ── Turn-start overlay input handling ─────────────────────────────────
    if map_view_state.turn_start_overlay {
        if frame(UiAction::Confirm) || frame(UiAction::Cancel) {
            if let Some(entity) = overlays.turn_start.iter().next() {
                commands.entity(entity).despawn();
            }
            map_view_state.turn_start_overlay = false;
            if map_view_state.pending_defeat_skip {
                map_view_state.pending_defeat_skip = false;
                advance_turn_with_skips(
                    &mut commands,
                    &mut map_view_state,
                    visible_cols,
                    visible_rows,
                );
            }
            return;
        }

        for (interaction, mut bg, mut border) in overlays.turn_start_buttons.iter_mut() {
            update_button_style(true, interaction, &mut bg, &mut border);
            let clicked = frame(UiAction::Confirm);
            let pressed = matches!(interaction, Interaction::Pressed);
            if clicked || pressed {
                if let Some(entity) = overlays.turn_start.iter().next() {
                    commands.entity(entity).despawn();
                }
                map_view_state.turn_start_overlay = false;
                if map_view_state.pending_defeat_skip {
                    map_view_state.pending_defeat_skip = false;
                    advance_turn_with_skips(
                        &mut commands,
                        &mut map_view_state,
                        visible_cols,
                        visible_rows,
                    );
                }
                return;
            }
        }
        return;
    }

    // ── Pause overlay input handling ──────────────────────────────────────
    if map_view_state.pause_overlay {
        let selected = map_view_state.pause_selected;

        if frame(UiAction::Up) || frame(UiAction::CursorUp) {
            map_view_state.pause_selected = selected.saturating_sub(1);
        }
        if frame(UiAction::Down) || frame(UiAction::CursorDown) {
            map_view_state.pause_selected = (selected + 1).min(1);
        }

        // Esc always resumes
        if frame(UiAction::Cancel) {
            if let Some(entity) = overlays.pause.iter().next() {
                commands.entity(entity).despawn();
            }
            map_view_state.pause_overlay = false;
            map_view_state.pause_selected = 0;
            return;
        }

        // Update button styles and handle clicks
        let sel = map_view_state.pause_selected;
        for (et_confirm, et_cancel, resume_opt, quit_opt, mut bg, mut border, interaction) in
            overlays.buttons.iter_mut()
        {
            if et_confirm.is_some() || et_cancel.is_some() {
                continue;
            }
            let is_sel = match (resume_opt, quit_opt) {
                (Some(_), None) => sel == 0,
                (None, Some(_)) => sel == 1,
                _ => continue,
            };
            update_button_style(is_sel, interaction, &mut bg, &mut border);
            let clicked = is_sel && frame(UiAction::Confirm);
            let pressed = matches!(interaction, Interaction::Pressed);
            if clicked || pressed {
                if resume_opt.is_some() {
                    if let Some(entity) = overlays.pause.iter().next() {
                        commands.entity(entity).despawn();
                    }
                    map_view_state.pause_overlay = false;
                    map_view_state.pause_selected = 0;
                    return;
                }
                if quit_opt.is_some() {
                    if let Some(entity) = overlays.pause.iter().next() {
                        commands.entity(entity).despawn();
                    }
                    map_view_state.pause_overlay = false;
                    map_view_state.pause_selected = 0;
                    next_state.set(AppState::Splash);
                    return;
                }
            }
        }

        return;
    }

    // ── End-turn overlay input handling ───────────────────────────────────
    if map_view_state.end_turn_overlay {
        let selected = map_view_state.end_turn_selected;

        if frame(UiAction::Up) || frame(UiAction::CursorUp) {
            map_view_state.end_turn_selected = selected.saturating_sub(1);
        }
        if frame(UiAction::Down) || frame(UiAction::CursorDown) {
            map_view_state.end_turn_selected = (selected + 1).min(1);
        }

        // Esc cancels
        if frame(UiAction::Cancel) {
            if let Some(entity) = overlays.end_turn.iter().next() {
                commands.entity(entity).despawn();
            }
            map_view_state.end_turn_overlay = false;
            map_view_state.end_turn_selected = 0;
            return;
        }

        // NextTurn action always confirms (fast action)
        if frame(UiAction::NextTurn) {
            if let Some(entity) = overlays.end_turn.iter().next() {
                commands.entity(entity).despawn();
            }
            map_view_state.end_turn_overlay = false;
            map_view_state.end_turn_selected = 0;
            advance_turn_with_skips(&mut commands, &mut map_view_state, visible_cols, visible_rows);
            return;
        }

        // Update button styles and handle clicks
        let sel = map_view_state.end_turn_selected;
        for (confirm_opt, cancel_opt, p_resume, p_quit, mut bg, mut border, interaction) in
            overlays.buttons.iter_mut()
        {
            if p_resume.is_some() || p_quit.is_some() {
                continue;
            }
            let is_sel = match (confirm_opt, cancel_opt) {
                (Some(_), None) => sel == 0,
                (None, Some(_)) => sel == 1,
                _ => continue,
            };
            update_button_style(is_sel, interaction, &mut bg, &mut border);
            let clicked = is_sel && frame(UiAction::Confirm);
            let button_pressed = matches!(interaction, Interaction::Pressed);
            if clicked || button_pressed {
                if confirm_opt.is_some() {
                    if let Some(entity) = overlays.end_turn.iter().next() {
                        commands.entity(entity).despawn();
                    }
                    map_view_state.end_turn_overlay = false;
                    map_view_state.end_turn_selected = 0;
                    advance_turn_with_skips(
                        &mut commands,
                        &mut map_view_state,
                        visible_cols,
                        visible_rows,
                    );
                    return;
                }
                if cancel_opt.is_some() {
                    if let Some(entity) = overlays.end_turn.iter().next() {
                        commands.entity(entity).despawn();
                    }
                    map_view_state.end_turn_overlay = false;
                    map_view_state.end_turn_selected = 0;
                    return;
                }
            }
        }

        return;
    }

    // ── Normal game input ────────────────────────────────────────────────
    let mut events = Vec::new();
    if frame(UiAction::Up) {
        events.push(InputEvent::Up);
    }
    if frame(UiAction::Down) {
        events.push(InputEvent::Down);
    }
    if frame(UiAction::Left) {
        events.push(InputEvent::Left);
    }
    if frame(UiAction::Right) {
        events.push(InputEvent::Right);
    }
    if frame(UiAction::CursorLeft) {
        events.push(InputEvent::CursorLeft);
    }
    if frame(UiAction::CursorDown) {
        events.push(InputEvent::CursorDown);
    }
    if frame(UiAction::CursorUp) {
        events.push(InputEvent::CursorUp);
    }
    if frame(UiAction::CursorRight) {
        events.push(InputEvent::CursorRight);
    }
    if frame(UiAction::PanUp) {
        events.push(InputEvent::PanUp);
    }
    if frame(UiAction::PanDown) {
        events.push(InputEvent::PanDown);
    }
    if frame(UiAction::PanLeft) {
        events.push(InputEvent::PanLeft);
    }
    if frame(UiAction::PanRight) {
        events.push(InputEvent::PanRight);
    }
    if frame(UiAction::NextHero) {
        events.push(InputEvent::NextHero);
    }
    if frame(UiAction::PlaceRod) {
        events.push(InputEvent::PlaceRod);
    }
    if frame(UiAction::Confirm) {
        events.push(InputEvent::Enter);
    }

    // KeyQ is not in UiAction, keep raw key check for now
    // (TODO: add Quit/KeyQ to keybindings.toml if desired)
    // No direct KeyCode access here — relies on MessageReader<UiAction>

    // NextTurn triggers end-turn overlay
    if frame(UiAction::NextTurn) {
        events.push(InputEvent::NextTurn);
    }

    // Cancel opens pause overlay instead of going back immediately.
    if frame(UiAction::Cancel) {
        map_view_state.pause_overlay = true;
        map_view_state.pause_selected = 0;
        spawn_pause_overlay(&mut commands);
        return;
    }

    let mut needs_redraw = map_view_state.needs_initial_draw;
    map_view_state.needs_initial_draw = false;
    let mut request_end_turn = false;
    let view_x = map_view_state.view_x();
    let view_y = map_view_state.view_y();

    if let Some(cursor_position) = window.cursor_position() {
        if let Some((col, row)) = mouse_visible_tile(
            &window,
            cursor_position,
            map_view_state.tile_size,
            visible_cols,
            visible_rows,
        ) {
            let target_x = view_x + col;
            let target_y = view_y + row;
            let target_tile = (target_x, target_y);
            if map_view_state.last_mouse_tile != Some(target_tile)
                && map_view_state.set_cursor_from_pointer(
                    target_x,
                    target_y,
                    visible_cols,
                    visible_rows,
                )
            {
                needs_redraw = true;
            }
            map_view_state.last_mouse_tile = Some(target_tile);

            // Mouse click on the tile under the cursor acts as Enter,
            // but only if the input cooldown has elapsed (prevents stale clicks
            // after a state transition from bleeding through).
            if mouse_buttons.just_pressed(MouseButton::Left)
                && !cooldown.is_cooling_down(time.elapsed_secs_f64())
            {
                events.push(InputEvent::Enter);
            }
        } else {
            map_view_state.last_mouse_tile = None;
        }
    } else {
        map_view_state.last_mouse_tile = None;
    }

    for event in events {
        let outcome = map_view_state.handle_input(event, visible_cols, visible_rows);
        match outcome {
            MapViewOutcome::NoChange => {}
            MapViewOutcome::Changed | MapViewOutcome::CursorChanged => {
                needs_redraw = true;
            }
            MapViewOutcome::RequestEndTurn => {
                request_end_turn = true;
            }
            MapViewOutcome::OpenStructureOverlay { name } => match name.as_str() {
                "City" | "City Entrance" => {
                    // Find the nearest CityEntrance tile within the city structure
                    // so we can show the hire-hero screen.
                    if let Some(entrance) = map_view_state.find_city_entrance_at_cursor() {
                        map_view_state.set_cursor_pos(entrance.x, entrance.y);
                    }
                    next_state.set(AppState::CityEntrance);
                    return;
                }
                _ => {
                    map_view_state.set_status(Some(format!("Structure: {}", name)));
                    needs_redraw = true;
                }
            },
            MapViewOutcome::OpenHeroInfo => {
                next_state.set(AppState::Hero);
                return;
            }
        }
    }

    if request_end_turn {
        map_view_state.end_turn_overlay = true;
        map_view_state.end_turn_selected = 0;
        let team_name = map_view_state
            .get_game_state()
            .and_then(|state| state.get_active_team().ok().map(|t| t.get_name()))
            .unwrap_or("Unknown");
        spawn_end_turn_overlay(&mut commands, team_name);
        return;
    }

    let status_text = map_view_state.status().unwrap_or("").to_string();
    let team_name = map_view_state
        .get_game_state()
        .and_then(|state| state.get_active_team().ok().map(|t| t.get_name()))
        .unwrap_or("Unknown");
    let status_text = if status_text.is_empty() {
        format!("Turn: {team_name}")
    } else {
        format!("Turn: {team_name} | {status_text}")
    };
    for mut text in status_query.iter_mut() {
        text.0 = status_text.clone();
    }

    if needs_redraw {
        let Some(state) = map_view_state.get_game_state() else {
            return;
        };
        let map = &state.map;
        let tile_config = state.tile_config();
        let city_centers = owned_city_centers(state);
        let view_x = map_view_state.view_x();
        let view_y = map_view_state.view_y();
        let cursor_x = map_view_state.cursor_x();
        let cursor_y = map_view_state.cursor_y();
        let selected_hero_id = map_view_state.selected_hero_id();

        // Refresh the top HUD bar (turn number, score, gold, resources) for the active team.
        if let Ok(team) = state.get_active_team() {
            let turn = team.get_turn();
            let team_id = team.get_id();
            let gold = team.gold();
            let resources = team.resources();
            let score = state.team_score(team_id);
            for (mut text, field) in top_bar_query.iter_mut() {
                text.0 = match *field {
                    TopBarField::Turn => format!("T{turn} ⚑{}", score.total()),
                    TopBarField::Gold => format!("{gold}"),
                    TopBarField::Resource(idx) => {
                        format!("{}", resources.get(idx).copied().unwrap_or(0))
                    }
                };
            }
        }

        let city_cursor = if cursor_x >= 0 && cursor_y >= 0 {
            let cursor_coord = MapCoord::new(cursor_x as u32, cursor_y as u32);
            map.get_tile(cursor_coord).map(|tile| is_city_core_tile(tile.kind)).unwrap_or(false)
        } else {
            false
        };

        for (tile_pos, mut sprite) in layers.tiles.iter_mut() {
            let tx = view_x + tile_pos.col;
            let ty = view_y + tile_pos.row;
            let coord = MapCoord::new(tx as u32, ty as u32);
            sprite.custom_size = Some(Vec2::splat(map_view_state.tile_size));

            // Check for hero on this tile first — hero sprite takes priority.
            if let Some(hero) = state.hero_at(&coord) {
                let is_selected = selected_hero_id == Some(hero.get_id());
                // Color: team color for selected, 50% alpha team color for others.
                let team_color = state
                    .get_team(hero.get_team_id())
                    .map(|t| {
                        let (r, g, b) = t.get_color();
                        Color::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
                    })
                    .unwrap_or(HERO_COLOR);
                sprite.color = if is_selected { team_color } else { team_color.with_alpha(0.5) };
                if let Some(atlas) = sprite.texture_atlas.as_mut() {
                    atlas.index = hero.get_atlas_index();
                }
                continue;
            }

            // Tile always shows its own color/atlas — cursor is a separate overlay sprite.
            let tile = map.get_tile(coord).ok();
            let resource_node = map.resource_node_at(coord);
            // A resource point is anything the engine treats as ownable via a
            // rod: an explicit resource node, or a bare Gold/Resource tile (e.g.
            // mines loaded from a Tiled map that carries no resource nodes).
            // Tint it by its owner so a captured mine matches its claimed land.
            let is_resource_point =
                resource_node.is_some() || tile.map(|t| t.is_resource()).unwrap_or(false);
            sprite.color = match (tile, is_resource_point) {
                (_, true) => match state.resource_owner(coord) {
                    Some(team_id) => state
                        .get_team(team_id)
                        .map(|team| {
                            let (r, g, b) = team.get_color();
                            rgb_color(r, g, b)
                        })
                        .unwrap_or(NEUTRAL_RESOURCE_COLOR),
                    None => NEUTRAL_RESOURCE_COLOR,
                },
                (Some(t), false) if t.is_city() => {
                    match state.city_owner(&coord) {
                        // The owned city centre cell is hidden so the team logo
                        // overlay (CityLogoTile layer) takes its place; the rest of
                        // the city keeps its owner-tinted castle tiles.
                        Some(_) if city_centers.contains(&coord) => Color::NONE,
                        Some(team_id) => state
                            .get_team(team_id)
                            .map(|team| {
                                let (r, g, b) = team.get_color();
                                rgb_color(r, g, b)
                            })
                            .unwrap_or(NEUTRAL_CITY_COLOR),
                        None => NEUTRAL_CITY_COLOR,
                    }
                }
                (Some(t), false) => tile_color_for(t.kind, tile_config),
                (None, false) => Color::BLACK,
            };
            if let Some(atlas) = sprite.texture_atlas.as_mut() {
                let idx = resource_node
                    .map(|node| resource_atlas_index(node.kind))
                    .or_else(|| tile.map(|t| tile_atlas_index(t.kind, tile_config)))
                    .unwrap_or(0);
                atlas.index = idx;
            }
        }

        for (tile_pos, mut sprite) in layers.land.iter_mut() {
            let tx = view_x + tile_pos.col;
            let ty = view_y + tile_pos.row;
            let coord = MapCoord::new(tx as u32, ty as u32);
            sprite.custom_size = Some(Vec2::splat(map_view_state.tile_size));
            sprite.color = state
                .land_owner(coord)
                .and_then(|team_id| state.get_team(team_id))
                .map(|team| {
                    let (r, g, b) = team.get_color();
                    rgb_color(r, g, b).with_alpha(0.35)
                })
                .unwrap_or(Color::NONE);
        }

        for (tile_pos, mut sprite) in layers.rod.iter_mut() {
            let tx = view_x + tile_pos.col;
            let ty = view_y + tile_pos.row;
            let coord = MapCoord::new(tx as u32, ty as u32);
            sprite.custom_size = Some(Vec2::splat(map_view_state.tile_size));
            sprite.color = state
                .resource_rod_owner(coord)
                .and_then(|team_id| state.get_team(team_id))
                .map(|team| {
                    let (r, g, b) = team.get_color();
                    rgb_color(r, g, b)
                })
                .unwrap_or(Color::NONE);
            if let Some(atlas) = sprite.texture_atlas.as_mut() {
                atlas.index = RESOURCE_ROD_ATLAS_INDEX;
            }
        }

        // City logo overlay: draw the owning team's logo on its city core tile.
        let catalog = state.team_catalog();
        for (tile_pos, mut sprite) in layers.logo.iter_mut() {
            let tx = view_x + tile_pos.col;
            let ty = view_y + tile_pos.row;
            let coord = MapCoord::new(tx as u32, ty as u32);
            sprite.custom_size = Some(Vec2::splat(map_view_state.tile_size));

            // Resolve a logo only for a city centre cell with no hero on it.
            let is_center = city_centers.contains(&coord);
            let resolved = if is_center && state.hero_at(&coord).is_none() {
                state.city_owner(&coord).and_then(|team_id| state.get_team(team_id)).and_then(
                    |team| {
                        catalog
                            .by_name(team.get_name())
                            .map(|def| (def.get_logo(), team.get_name(), team.get_color()))
                    },
                )
            } else {
                None
            };

            match resolved {
                Some((logo, name, (r, g, b))) => {
                    let tint = rgb_color(r, g, b);
                    match &logo {
                        TeamLogo::Tile(index) => {
                            sprite.image = layers.atlas.image.clone();
                            sprite.texture_atlas = Some(TextureAtlas {
                                layout: layers.atlas.layout.clone(),
                                index: *index as usize,
                            });
                            sprite.color = tint;
                        }
                        TeamLogo::Bitmap(_) => {
                            match layers.logo_images.handle(&mut layers.images, name, logo) {
                                Some(handle) => {
                                    sprite.image = handle;
                                    sprite.texture_atlas = None;
                                    sprite.color = tint;
                                }
                                None => sprite.color = Color::NONE,
                            }
                        }
                    }
                }
                None => sprite.color = Color::NONE,
            }
        }

        // Move cursor overlay to the correct tile position.
        let cx = cursor_x - view_x as isize;
        let cy = cursor_y - view_y as isize;
        if cx >= 0 && cy >= 0 && (cx as usize) < visible_cols && (cy as usize) < visible_rows {
            let tile_size = map_view_state.tile_size;
            let total_w = visible_cols as f32 * tile_size;
            let total_h = visible_rows as f32 * tile_size;
            let offset_x = -total_w / 2.0 + tile_size / 2.0;
            let offset_y = total_h / 2.0 - tile_size / 2.0;
            let new_x = offset_x + cx as f32 * tile_size;
            let new_y = offset_y - cy as f32 * tile_size;
            for (mut transform, mut sprite) in cursor_query.iter_mut() {
                transform.translation.x = new_x;
                transform.translation.y = new_y;
                transform.translation.z = CITY_CURSOR_Z;
                sprite.custom_size = Some(Vec2::splat(if city_cursor {
                    tile_size * CITY_CURSOR_SCALE
                } else {
                    tile_size
                }));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn exit_map_view(
    mut commands: Commands,
    query: Query<Entity, With<MapViewRoot>>,
    top_bar_query: Query<Entity, With<TopBarRoot>>,
    tile_query: Query<Entity, With<MapTile>>,
    land_query: Query<Entity, With<LandOwnerTile>>,
    rod_query: Query<Entity, With<ResourceRodTile>>,
    logo_query: Query<Entity, With<CityLogoTile>>,
    cursor_query: Query<Entity, With<CursorOverlay>>,
    pause_query: Query<Entity, With<PauseOverlay>>,
    end_turn_query: Query<Entity, With<EndTurnOverlay>>,
    turn_start_query: Query<Entity, With<TurnStartOverlay>>,
    mut map_view_state: ResMut<MapViewState>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in top_bar_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in tile_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in land_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in rod_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in logo_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in cursor_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in pause_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in end_turn_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in turn_start_query.iter() {
        commands.entity(entity).despawn();
    }
    map_view_state.pause_overlay = false;
    map_view_state.end_turn_overlay = false;
    map_view_state.turn_start_overlay = false;
    map_view_state.ai_turn_state.reset();
    map_view_state.pending_defeat_skip = false;
    map_view_state.defeated_teams.clear();
}
