//! Map selection screen state.

use alloc::{string::String, vec::Vec};

use crate::storage::MapEntry;

/// Map list browser model.
pub struct MapSelectScreen {
    /// All discovered `.tmx` maps.
    pub maps: Vec<MapEntry>,
    /// Currently highlighted item index.
    pub selected: usize,
    /// First visible item index in the scroll window.
    pub scroll: usize,
    /// Optional footer status line.
    pub status: Option<String>,
}
