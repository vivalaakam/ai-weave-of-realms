#[derive(Debug, thiserror::Error)]
pub enum HelpersError {
    #[error("I/O error: {0}")]
    FileMetadata(std::io::Error),
}
