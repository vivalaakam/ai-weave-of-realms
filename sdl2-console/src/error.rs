#[derive(Debug)]
#[allow(dead_code)]
pub enum HostError {
    Message(String),
    Io(std::io::Error),
    Engine(String),
}

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
