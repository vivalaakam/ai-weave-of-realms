//! Shared data types used across game modules.

use alloc::string::String;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListEntry {
    /// Stable host-specific identifier used for loading the selected item.
    pub id: String,
    /// Primary display label.
    pub label: String,
    /// Secondary numeric metadata, usually file size.
    pub meta: u32,
}
