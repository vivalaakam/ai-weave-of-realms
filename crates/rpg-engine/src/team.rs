//! Team configuration for the game session.

use serde::{Deserialize, Serialize};

use crate::hero::TeamId;

// ─── Team ─────────────────────────────────────────────────────────────────────

/// Team configuration: identity, display name, and color.
///
/// Used to define player and AI teams in the game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    /// Unique numeric identifier (0-8).
    pub id: TeamId,
    /// Human-readable team name (e.g. "Red", "Blue").
    pub name: String,
    /// Display color as RGB tuple.
    pub color: (u8, u8, u8),
    /// `true` if the human player can select and command heroes on this team.
    pub player_controlled: bool,
    /// How many turns this team has taken (0 = not yet started).
    /// Incremented by [`GameState::on_turn`] at the start of each of this team's turns.
    pub turn: u32,
}

impl Team {
    /// Creates a new team with the given properties.
    pub fn new(id: TeamId, name: impl Into<String>, color: (u8, u8, u8), player_controlled: bool) -> Self {
        Self {
            id,
            name: name.into(),
            color,
            player_controlled,
            turn: 0,
        }
    }

    /// Creates a default Red player team (id=0, first slot).
    pub fn red() -> Self {
        Self::new(0, "Red", (220, 50, 50), true)
    }

    /// Creates a default Blue player team (id=1, second slot).
    pub fn blue() -> Self {
        Self::new(1, "Blue", (50, 100, 220), true)
    }

    /// Creates a default AI-controlled enemy team (id=2, third slot).
    pub fn enemy() -> Self {
        Self::new(2, "Enemy", (150, 80, 200), false)
    }
}
