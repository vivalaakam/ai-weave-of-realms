#[derive(Debug, thiserror::Error)]
pub enum GameMapError {
    #[error("Spawn error: {0}")]
    SpawnError(#[from] crate::spawn::SpawnError),
}
