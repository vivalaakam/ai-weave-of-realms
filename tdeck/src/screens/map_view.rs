//! Active map and gameplay screen state.

use alloc::{string::String, vec::Vec};

use crate::storage::MapEntry;
use crate::system_info::SystemInfoSnapshot;
use rpg_embedded::map_view::MapViewApp;

/// Screen model for the loaded map session.
pub struct MapViewScreen {
    /// Engine-backed game session.
    pub app: MapViewApp,
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
