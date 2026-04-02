use crate::hero::TeamId;

#[derive(thiserror::Error, Debug)]
pub enum GameError {
    #[error("No active team")]
    NoActiveTeam,
    #[error("Next active team does not exist")]
    NextActiveTeam,
    #[error("active team {0} does not exist")]
    ActiveTeamNotFound(TeamId),
}
