use crate::helpers_error::HelpersError;
use crate::ListEntry;
use std::fs;
use std::path::Path;

pub fn file_label(path: &Path) -> String {
    path.file_stem().and_then(|value| value.to_str()).unwrap_or("unnamed").to_string()
}

pub fn file_entry(prefix: &str, path: &Path) -> Result<ListEntry, HelpersError> {
    let metadata = fs::metadata(path).map_err(HelpersError::FileMetadata)?;
    Ok(ListEntry {
        id: format!("{prefix}{}", path.display()),
        label: file_label(path),
        meta: u32::try_from(metadata.len()).unwrap_or(u32::MAX),
    })
}
