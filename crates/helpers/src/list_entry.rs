/// Shared list entry shown in embedded selector UIs.
#[derive(Clone)]
pub struct ListEntry {
    /// Stable host-specific identifier used for loading the selected item.
    pub id: String,
    /// Primary display label.
    pub label: String,
    /// Secondary numeric metadata, usually file size.
    pub meta: u32,
}
