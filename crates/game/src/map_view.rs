//! Shared gameplay map-view state and input handling.

use alloc::{string::String, string::ToString};

use engine::Direction;

use crate::input::InputEvent;
use crate::session::GameSession;

/// Result of applying one shared input event to the map view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MapViewOutcome {
    /// State did not change.
    NoChange,
    /// State changed and should be redrawn.
    Changed,
    /// User requested leaving the map view.
    BackRequested,
    /// User requested ending current turn.
    RequestEndTurn,
    /// Game over — the match ended in victory or defeat.
    GameOver { won: bool, message: String },
}

/// Shared gameplay screen model for embedded and terminal frontends.
pub struct MapViewApp {
    session: GameSession,
    view_x: usize,
    view_y: usize,
    status: Option<String>,
}

impl MapViewApp {
    /// Creates a new shared map-view state.
    pub fn new(session: GameSession, view_x: usize, view_y: usize, status: Option<String>) -> Self {
        Self { session, view_x, view_y, status }
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

    /// Returns the current topmost visible tile.
    pub fn view_y(&self) -> usize {
        self.view_y
    }

    /// Returns the current footer status line.
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// Replaces the current footer status line.
    pub fn set_status(&mut self, status: Option<String>) {
        self.status = status;
    }

    /// Clamps the current viewport to the map dimensions.
    pub fn clamp_view_to_map(&mut self, visible_cols: usize, visible_rows: usize) {
        let map = &self.session.state().map;
        let max_x = map.tile_width().saturating_sub(visible_cols as u32) as usize;
        let max_y = map.tile_height().saturating_sub(visible_rows as u32) as usize;
        self.view_x = self.view_x.min(max_x);
        self.view_y = self.view_y.min(max_y);
    }

    /// Applies a single input event to the shared map view.
    ///
    /// Arrow keys move the selected hero one tile.
    /// WASD / HJKL pan the viewport.
    /// Tab cycles to the next hero and centers the camera on them.
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
            InputEvent::NextHero => {
                self.session_mut().cycle_selected_hero();
                self.center_on_hero(visible_cols, visible_rows);
                self.status = Some(self.session.summary());
                MapViewOutcome::Changed
            }
            InputEvent::Key(ch) => match ch.to_ascii_lowercase() {
                'w' | 'k' => self.pan_view(InputEvent::Up, visible_cols, visible_rows),
                's' | 'j' => self.pan_view(InputEvent::Down, visible_cols, visible_rows),
                'a' | 'h' => self.pan_view(InputEvent::Left, visible_cols, visible_rows),
                'd' | 'l' => self.pan_view(InputEvent::Right, visible_cols, visible_rows),
                'q' => MapViewOutcome::BackRequested,
                _ => MapViewOutcome::NoChange,
            },
            InputEvent::PanUp => self.pan_view(InputEvent::Up, visible_cols, visible_rows),
            InputEvent::PanDown => self.pan_view(InputEvent::Down, visible_cols, visible_rows),
            InputEvent::PanLeft => self.pan_view(InputEvent::Left, visible_cols, visible_rows),
            InputEvent::PanRight => self.pan_view(InputEvent::Right, visible_cols, visible_rows),
            InputEvent::Enter => MapViewOutcome::NoChange,
            InputEvent::NextTurn => MapViewOutcome::RequestEndTurn,
            InputEvent::Back => MapViewOutcome::BackRequested,
            InputEvent::None => MapViewOutcome::NoChange,
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
                self.center_on_hero(visible_cols, visible_rows);
                MapViewOutcome::Changed
            }
            Err(err) => {
                self.status = Some(err.to_string());
                MapViewOutcome::Changed
            }
        }
    }

    fn center_on_hero(&mut self, visible_cols: usize, visible_rows: usize) -> bool {
        let hero = self.session.selected_hero_position();
        let hero_x = hero.x as usize;
        let hero_y = hero.y as usize;

        let map = &self.session.state().map;
        let max_x = map.tile_width().saturating_sub(visible_cols as u32) as usize;
        let max_y = map.tile_height().saturating_sub(visible_rows as u32) as usize;

        let target_x =
            if hero_x >= visible_cols / 2 { (hero_x - visible_cols / 2).min(max_x) } else { 0 };
        let target_y =
            if hero_y >= visible_rows / 2 { (hero_y - visible_rows / 2).min(max_y) } else { 0 };

        let changed = self.view_x != target_x || self.view_y != target_y;
        self.view_x = target_x;
        self.view_y = target_y;
        changed
    }
}
