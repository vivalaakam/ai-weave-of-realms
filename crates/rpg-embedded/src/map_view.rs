//! Shared gameplay map-view state and input handling.

use alloc::{string::String, string::ToString};

use rpg_engine::Direction;

use crate::input::InputEvent;
use crate::session::GameSession;

/// Interaction mode for the shared map view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionMode {
    /// Direction keys move the viewport.
    Pan,
    /// Direction keys move the selected hero through `rpg-engine`.
    Hero,
}

/// Result of applying one shared input event to the map view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapViewOutcome {
    /// State did not change.
    NoChange,
    /// State changed and should be redrawn.
    Changed,
    /// User requested leaving the map view.
    BackRequested,
}

/// Shared gameplay screen model for embedded and terminal frontends.
pub struct MapViewApp {
    session: GameSession,
    view_x: usize,
    view_y: usize,
    mode: InteractionMode,
    status: Option<String>,
}

impl MapViewApp {
    /// Creates a new shared map-view state.
    ///
    /// # Arguments
    /// * `session` - Gameplay session to present.
    /// * `view_x` - Initial leftmost visible tile column.
    /// * `view_y` - Initial topmost visible tile row.
    /// * `status` - Initial status line.
    pub fn new(session: GameSession, view_x: usize, view_y: usize, status: Option<String>) -> Self {
        Self { session, view_x, view_y, mode: InteractionMode::Pan, status }
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

    /// Returns the current interaction mode.
    pub fn mode(&self) -> InteractionMode {
        self.mode
    }

    /// Returns the current footer status line.
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// Replaces the current footer status line.
    ///
    /// # Arguments
    /// * `status` - New optional status text.
    pub fn set_status(&mut self, status: Option<String>) {
        self.status = status;
    }

    /// Replaces the current interaction mode.
    ///
    /// # Arguments
    /// * `mode` - New control mode.
    pub fn set_mode(&mut self, mode: InteractionMode) {
        self.mode = mode;
    }

    /// Clamps the current viewport to the map dimensions.
    ///
    /// # Arguments
    /// * `visible_cols` - Number of visible tile columns.
    /// * `visible_rows` - Number of visible tile rows.
    pub fn clamp_view_to_map(&mut self, visible_cols: usize, visible_rows: usize) {
        let map = &self.session.state().map;
        let max_x = map.tile_width().saturating_sub(visible_cols as u32) as usize;
        let max_y = map.tile_height().saturating_sub(visible_rows as u32) as usize;
        self.view_x = self.view_x.min(max_x);
        self.view_y = self.view_y.min(max_y);
    }

    /// Applies a single input event to the shared map view.
    ///
    /// # Arguments
    /// * `event` - Platform-neutral input event.
    /// * `visible_cols` - Number of visible tile columns on the target device.
    /// * `visible_rows` - Number of visible tile rows on the target device.
    pub fn handle_input(
        &mut self,
        event: InputEvent,
        visible_cols: usize,
        visible_rows: usize,
    ) -> MapViewOutcome {
        let event = map_view_event(event);
        match event {
            InputEvent::Enter => {
                self.mode = match self.mode {
                    InteractionMode::Pan => InteractionMode::Hero,
                    InteractionMode::Hero => InteractionMode::Pan,
                };
                self.status = Some(match self.mode {
                    InteractionMode::Pan => "Pan mode: arrows move the viewport".to_string(),
                    InteractionMode::Hero => "Hero mode: arrows move the selected hero".to_string(),
                });
                return MapViewOutcome::Changed;
            }
            InputEvent::Back => return MapViewOutcome::BackRequested,
            InputEvent::None | InputEvent::Key(_) => return MapViewOutcome::NoChange,
            InputEvent::Up | InputEvent::Down | InputEvent::Left | InputEvent::Right => {}
        }

        let changed = match self.mode {
            InteractionMode::Pan => self.pan_view(event, visible_cols, visible_rows),
            InteractionMode::Hero => self.move_hero_or_report(event, visible_cols, visible_rows),
        };

        if changed {
            MapViewOutcome::Changed
        } else {
            MapViewOutcome::NoChange
        }
    }

    fn pan_view(&mut self, event: InputEvent, visible_cols: usize, visible_rows: usize) -> bool {
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
            InputEvent::None | InputEvent::Key(_) | InputEvent::Enter | InputEvent::Back => {}
        }

        self.view_x != previous_x || self.view_y != previous_y
    }

    fn move_hero_or_report(
        &mut self,
        event: InputEvent,
        visible_cols: usize,
        visible_rows: usize,
    ) -> bool {
        let direction = match event {
            InputEvent::Up => Some(Direction::North),
            InputEvent::Down => Some(Direction::South),
            InputEvent::Left => Some(Direction::West),
            InputEvent::Right => Some(Direction::East),
            InputEvent::None | InputEvent::Key(_) | InputEvent::Enter | InputEvent::Back => None,
        };

        let Some(direction) = direction else {
            return false;
        };

        match self.session.move_selected_hero(direction) {
            Ok(position) => {
                self.status = Some(self.session.summary());
                self.keep_hero_visible(
                    position.x as usize,
                    position.y as usize,
                    visible_cols,
                    visible_rows,
                )
            }
            Err(err) => {
                self.status = Some(err.to_string());
                true
            }
        }
    }

    fn keep_hero_visible(
        &mut self,
        hero_x: usize,
        hero_y: usize,
        visible_cols: usize,
        visible_rows: usize,
    ) -> bool {
        if hero_x < self.view_x {
            self.view_x = hero_x;
        } else if hero_x >= self.view_x + visible_cols {
            self.view_x = hero_x.saturating_sub(visible_cols.saturating_sub(1));
        }

        if hero_y < self.view_y {
            self.view_y = hero_y;
        } else if hero_y >= self.view_y + visible_rows {
            self.view_y = hero_y.saturating_sub(visible_rows.saturating_sub(1));
        }

        true
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
