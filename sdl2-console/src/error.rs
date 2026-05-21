use helpers::HelpersError;

#[derive(Debug)]
pub enum HostError {
    Message(String),
    Io(std::io::Error),
    Engine(String),
    Helpers(HelpersError),
}