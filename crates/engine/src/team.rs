//! Team configuration for the game session.

use serde::{Deserialize, Serialize};

use crate::hero::TeamId;
use crate::map::game_map::RESOURCE_KIND_COUNT;

/// Gold every team starts the game with, before any turn income.
pub const STARTING_GOLD: u32 = 100;

// ─── Team ─────────────────────────────────────────────────────────────────────

/// Team configuration: identity, display name, color, and treasury.
///
/// Used to define player and AI teams in the game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    /// Unique numeric identifier (0-8).
    id: TeamId,
    /// Human-readable team name (e.g. "Red", "Blue").
    name: String,
    /// Display color as RGB tuple.
    color: (u8, u8, u8),
    /// `true` if the human player can select and command heroes on this team.
    player_controlled: bool,
    /// How many turns this team has taken (0 = not yet started).
    /// Incremented by [`GameState::on_turn`] at the start of each of this team's turns.
    turn: u32,
    /// Gold balance used to hire heroes and place resource rods.
    gold: u32,
    /// Stockpile of the four common resources, indexed by
    /// [`ResourceKind::resource_index`](crate::map::game_map::ResourceKind::resource_index).
    resources: [u32; RESOURCE_KIND_COUNT],
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
            gold: STARTING_GOLD,
            resources: [0; RESOURCE_KIND_COUNT],
        }
    }

    // ── Treasury ────────────────────────────────────────────────────────────

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_color(&self) -> (u8, u8, u8) {
        self.color
    }

    /// Returns the team's current gold balance.
    pub fn gold(&self) -> u32 {
        self.gold
    }

    /// Returns the stockpiled amount of the resource at `index` (0–3).
    pub fn resource(&self, index: usize) -> u32 {
        self.resources.get(index).copied().unwrap_or(0)
    }

    /// Returns the full resource stockpile.
    pub fn resources(&self) -> [u32; RESOURCE_KIND_COUNT] {
        self.resources
    }

    /// Adds `amount` gold to the treasury.
    pub(crate) fn add_gold(&mut self, amount: u32) {
        self.gold = self.gold.saturating_add(amount);
    }

    /// Spends `amount` gold if the team can afford it.
    ///
    /// Returns `true` and deducts the gold on success, or `false` (leaving the
    /// balance untouched) when there is not enough gold.
    pub(crate) fn spend_gold(&mut self, amount: u32) -> bool {
        if self.gold < amount {
            return false;
        }
        self.gold -= amount;
        true
    }

    /// Adds `amount` of the resource at `index` (0–3) to the stockpile.
    pub(crate) fn add_resource(&mut self, index: usize, amount: u32) {
        if let Some(slot) = self.resources.get_mut(index) {
            *slot = slot.saturating_add(amount);
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

    pub fn is_player_controlled(&self) -> bool {
        self.player_controlled
    }

    pub(crate) fn increment_turn(&mut self) {
        self.turn += 1;
    }
}
