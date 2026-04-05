//! Internal game-state specific errors.

use core::fmt;

use crate::hero::TeamId;

/// Errors specific to game-state team progression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameError {
    /// There is no active team in the current state.
    NoActiveTeam,
    /// Rotating to the next active team failed unexpectedly.
    NextActiveTeam,
    /// The active team id does not correspond to a registered team.
    ActiveTeamNotFound(TeamId),
}

impl fmt::Display for GameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameError::NoActiveTeam => formatter.write_str("No active team"),
            GameError::NextActiveTeam => formatter.write_str("Next active team does not exist"),
            GameError::ActiveTeamNotFound(team_id) => {
                write!(formatter, "active team {team_id} does not exist")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for GameError {}
