//! Error types for the rpg-engine crate.

use crate::hero::{HeroId, TeamId};
use crate::spawn::SpawnError;

/// All errors that can occur within the rpg-engine crate.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// A map coordinate or index is out of valid bounds.
    #[error("out of bounds: {0}")]
    OutOfBounds(String),
    /// An invalid tile identifier (unknown name, GID or ID) was encountered.
    #[error("invalid tile: {0}")]
    InvalidTileKind(String),
    /// A tile slice or vector had the wrong length.
    #[error("invalid tile data: {0}")]
    InvalidTiles(String),
    /// No game state is loaded when required.
    #[error("no game state loaded")]
    NoGameStateLoaded,
    /// No hero is selected when required.
    #[error("no hero selected")]
    NoSelectedHero,
    /// The requested attack target coordinate does not contain a hero.
    #[error("no target hero at ({x}, {y})")]
    NoTargetHero { x: u32, y: u32 },
    /// The requested action has no target coordinate.
    #[error("no target coordinate")]
    NoTargetCoord,
    /// Attacker and defender belong to the same team.
    #[error("cannot attack own hero {attacker_id} -> {defender_id}")]
    CannotAttackOwnHero { attacker_id: HeroId, defender_id: HeroId },
    /// Target is not adjacent to the attacker.
    #[error("target ({x}, {y}) is not adjacent to hero {attacker_id}")]
    TargetNotAdjacent { attacker_id: HeroId, x: u32, y: u32 },
    /// Cannot hire a hero because another hero occupies the tile.
    #[error("cannot hire hero at ({x}, {y}): tile occupied")]
    HireTileOccupied { x: u32, y: u32 },
    /// Cannot hire a hero because the city is not owned by the active team.
    #[error("cannot hire hero at ({x}, {y}): city not owned by team {team_id}")]
    HireNotOwnedCity { x: u32, y: u32, team_id: TeamId },
    /// Cannot place a rod because one already exists at the coordinate.
    #[error("resource rod already exists at ({x}, {y})")]
    ResourceRodAlreadyExists { x: u32, y: u32 },
    /// No adjacent passable tile exists for the hero after placing a rod.
    #[error("no adjacent passable tile for hero {hero_id} from ({x}, {y})")]
    NoAdjacentPassableTile { hero_id: HeroId, x: u32, y: u32 },
    /// A game state operation was attempted in an invalid state.
    #[error("invalid game state: expected {expected} chunks got {got}")]
    InvalidChunksSize { expected: usize, got: usize },
    /// A game state operation was attempted in an invalid state.
    #[error("invalid game state: expected {expected} tiles got {got}")]
    InvalidTilesSize { expected: usize, got: usize },
    /// The map-generation pipeline does not contain any generators.
    #[error("pipeline must have at least one generator")]
    PipelineEmpty,
    /// A validation-rule directory could not be read.
    #[error("cannot read validation rule directory '{path}': {err}")]
    ValidationRuleDir {
        /// Path that failed to load.
        path: String,
        /// Underlying I/O error.
        err: std::io::Error,
    },
    /// Movement was requested to a tile that cannot be reached.
    #[error("unreachable tile at ({x}, {y})")]
    UnreachableTile { x: u32, y: u32 },
    /// Movement was requested to a tile occupied by another hero.
    #[error("tile at ({x}, {y}) is occupied by another hero")]
    OccupiedTile { x: u32, y: u32 },
    /// Movement was requested to a tile that is not passable terrain.
    #[error("impassable tile at ({x}, {y})")]
    ImpassableTile { x: u32, y: u32 },
    /// Movement was requested but the hero has no movement points remaining.
    #[error("hero {hero_id} has no movement points remaining")]
    NoMovementPoints { hero_id: HeroId },
    /// Save/load serialization failed.
    #[error("save error: {0}")]
    Save(String),
    /// There is no active team in the current state.
    #[error("no active team")]
    NoActiveTeam,
    /// Rotating to the next active team failed unexpectedly.
    #[error("next active team does not exist")]
    NextActiveTeam,
    /// The active team id does not correspond to a registered team.
    #[error("active team {0} does not exist")]
    ActiveTeamNotFound(TeamId),
    #[error("spawn error: {0}")]
    SpawnError(#[from] SpawnError),
    /// A purchase was attempted without enough gold to cover its cost.
    #[error("not enough gold: need {needed}, have {have}")]
    InsufficientGold {
        /// Gold required for the action.
        needed: u32,
        /// Gold currently available to the team.
        have: u32,
    },
    #[error("minicbor serialization error: {0}")]
    MinicborEncode(#[from] minicbor_serde::error::EncodeError<core::convert::Infallible>),
    #[error("minicbor deserialization error: {0}")]
    MinicborDecode(#[from] minicbor_serde::error::DecodeError),
}
