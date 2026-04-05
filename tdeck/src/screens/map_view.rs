//! Active map and gameplay screen state.

use alloc::{string::String, vec::Vec};

use crate::session::GameSession;
use crate::storage::MapEntry;
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
    /// Optional save/load modal shown over the map.
    pub save_overlay: Option<SaveOverlay>,
}

/// Modal save/load state.
pub enum SaveOverlay {
    /// Root save menu (save/load/cancel).
    Menu {
        /// Selected menu index.
        selected: usize,
        /// Optional status message.
        status: Option<String>,
    },
    /// Filename entry for saving.
    SaveName {
        /// Current filename input.
        name: String,
        /// Optional status message.
        status: Option<String>,
    },
    /// List of saves to load.
    LoadList {
        /// Saves discovered on the SD card.
        saves: Vec<MapEntry>,
        /// Selected save index.
        selected: usize,
        /// Scroll offset for the list.
        scroll: usize,
        /// Optional status message.
        status: Option<String>,
    },
}
