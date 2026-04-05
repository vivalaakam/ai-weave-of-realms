//! Active map and gameplay screen state.

use alloc::string::String;

use crate::session::GameSession;
use crate::system_info::SystemInfoSnapshot;

/// Interaction mode for the map view.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InteractionMode {
    /// Direction keys move the viewport.
    Pan,
    /// Direction keys move the selected hero through `rpg-engine`.
    Hero,
}

/// Screen model for the loaded map session.
pub struct MapViewScreen {
    /// Engine-backed game session.
    pub session: GameSession,
    /// Leftmost visible tile column.
    pub view_x: usize,
    /// Topmost visible tile row.
    pub view_y: usize,
    /// Current control mode.
    pub mode: InteractionMode,
    /// Optional footer status line.
    pub status: Option<String>,
    /// Optional system info modal shown over the map.
    pub info_overlay: Option<SystemInfoSnapshot>,
}
