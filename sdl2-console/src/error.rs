use std::fmt;

#[derive(Debug)]
#[allow(dead_code)]
pub enum HostError {
    Message(String),
    Io(std::io::Error),
    Engine(String),
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HostError::Message(message) => write!(f, "{message}"),
            HostError::Io(error) => write!(f, "I/O error: {error}"),
            HostError::Engine(message) => write!(f, "Engine error: {message}"),
        }
    }
}

impl std::error::Error for HostError {}

impl From<std::io::Error> for HostError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<game::io::IoError> for HostError {
    fn from(error: game::io::IoError) -> Self {
        Self::Engine(error.to_string())
    }
}

impl From<String> for HostError {
    fn from(error: String) -> Self {
        Self::Message(error)
    }
}

impl From<&str> for HostError {
    fn from(error: &str) -> Self {
        Self::Message(error.to_string())
    }
}
