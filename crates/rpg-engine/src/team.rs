//! Team configuration for the game session.

use alloc::string::String;

use serde::{Deserialize, Serialize};

use crate::hero::TeamId;

// ─── Team ─────────────────────────────────────────────────────────────────────

/// Team configuration: identity, display name, and color.
///
/// Used to define player and AI teams in the game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    /// Unique numeric identifier (0-8).
    id: TeamId,
    /// Human-readable team name (e.g. "Red", "Blue").
    pub name: String,
    /// Display color as RGB tuple.
    pub color: (u8, u8, u8),
    /// `true` if the human player can select and command heroes on this team.
    player_controlled: bool,
    /// How many turns this team has taken (0 = not yet started).
    /// Incremented by [`GameState::on_turn`] at the start of each of this team's turns.
    turn: u32,
}

impl Team {
    /// Creates a new team with the given properties.
    pub fn new(
        id: TeamId,
        name: impl Into<String>,
        color: (u8, u8, u8),
        player_controlled: bool,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            color,
            player_controlled,
            turn: 0,
        }
    }

    pub(crate) fn reset_id(&mut self, id: TeamId) {
        self.id = id;
    }

    pub fn get_id(&self) -> TeamId {
        self.id
    }

    pub fn get_turn(&self) -> u32 {
        self.turn
    }

    /// Sets the team's turn counter.
    ///
    /// # Arguments
    /// * `turn` - New per-team turn index.
    pub(crate) fn set_turn(&mut self, turn: u32) {
        self.turn = turn;
    }

    pub fn is_player_controlled(&self) -> bool {
        self.player_controlled
    }

    pub(crate) fn increment_turn(&mut self) {
        self.turn += 1;
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
