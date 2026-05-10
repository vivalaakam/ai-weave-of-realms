//! Internal game-state specific errors.

use crate::hero::TeamId;

/// Errors specific to game-state team progression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GameError {
    /// There is no active team in the current state.
    #[error("No active team")]
    NoActiveTeam,
    /// Rotating to the next active team failed unexpectedly.
    #[error("Next active team does not exist")]
    NextActiveTeam,
    /// The active team id does not correspond to a registered team.
    #[error("active team {0} does not exist")]
    ActiveTeamNotFound(TeamId),
}
