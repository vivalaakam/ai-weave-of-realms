//! Hero entity — represents a unit on the map belonging to any team.

use serde::{Deserialize, Serialize};

use crate::map::game_map::MapCoord;
use crate::rng::SeededRng;

// ─── HeroId / TeamId ──────────────────────────────────────────────────────────

/// Numeric identifier for a hero; equals the hero's index in [`GameState::heroes`].
pub type HeroId = u8;

/// Numeric identifier for a team; equals the team's index in [`GameState::teams`].
pub type TeamId = u8;

// ─── Hero ─────────────────────────────────────────────────────────────────────

/// A hero unit on the game map.
///
/// Stats:
/// - `hp` / `max_hp` — health pool
/// - `atk` — attack power (raw damage before defence)
/// - `def` — defence rating (reduces incoming damage)
/// - `spd` — speed (determines combat initiative)
/// - `mov` — movement points per turn
///
/// Identity:
/// - `team_id` — which team this hero belongs to; look up [`crate::team::Team`] from
///   [`GameState::teams`] to get the name, color, and player-controlled flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hero {
    /// Unique identifier within the game session; equals the hero's index in [`GameState::heroes`].
    id: HeroId,
    /// Display name.
    pub name: String,
    /// Current hit points.
    pub hp: u32,
    /// Maximum hit points.
    pub max_hp: u32,
    /// Attack power.
    pub atk: u32,
    /// Defence rating.
    pub def: u32,
    /// Speed (combat initiative).
    pub spd: u32,
    /// Total movement points per turn.
    pub mov: u32,
    /// Movement points remaining this turn.
    pub mov_remaining: u32,
    /// Current tile position on the map.
    ///
    /// This is the **authoritative** position.  Visual layers must query the
    /// engine for position rather than caching it themselves.
    pub position: MapCoord,
    /// The team this hero belongs to.
    /// Look up [`crate::team::Team`] in [`GameState::teams`] by this id to get the full team data.
    pub team_id: TeamId,
    /// Personal RNG for this hero, derived from the session seed.
    ///
    /// Used during combat to compute this hero's attack rolls.
    /// Derive with [`SeededRng::derive_for_hero`] from the session RNG.
    pub rng: SeededRng,
}

impl Hero {
    pub(crate) fn reset_id(&mut self, id: HeroId) {
        self.id = id;
    }

    pub fn get_id(&self) -> HeroId {
        self.id
    }

    /// Computes the total movement points for a hero with the given speed.
    ///
    /// Formula: `20 + spd`
    pub fn movement_for_spd(spd: u32) -> u32 {
        20 + spd
    }

    /// Creates a new hero with full HP and full movement points.
    ///
    /// Movement is derived automatically from `spd` via [`Hero::movement_for_spd`].
    ///
    /// `rng` should be derived from the session RNG via
    /// [`SeededRng::derive_for_hero`] so that each hero has an independent,
    /// reproducible random stream tied to the session seed.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: HeroId,
        name: impl Into<String>,
        hp: u32,
        atk: u32,
        def: u32,
        spd: u32,
        position: MapCoord,
        team_id: TeamId,
        rng: SeededRng,
    ) -> Self {
        let mov = Self::movement_for_spd(spd);
        Self {
            id,
            name: name.into(),
            hp,
            max_hp: hp,
            atk,
            def,
            spd,
            mov,
            mov_remaining: mov,
            position,
            team_id,
            rng,
        }
    }

    /// Returns `true` if the hero is still alive (`hp > 0`).
    pub fn is_alive(&self) -> bool {
        self.hp > 0
    }

    /// Applies `damage` to the hero, clamping HP at zero.
    pub fn take_damage(&mut self, damage: u32) {
        self.hp = self.hp.saturating_sub(damage);
    }

    /// Resets movement points to the full `mov` value (call at turn start).
    pub fn reset_movement(&mut self) {
        self.mov_remaining = self.mov;
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn hero() -> Hero {
        let rng = SeededRng::new("test").derive_for_hero(1);
        Hero::new(0, "Arthur", 100, 20, 10, 15, MapCoord::new(0, 0), 1, rng)
    }

    #[test]
    fn new_hero_is_alive_with_full_hp() {
        let h = hero();
        assert!(h.is_alive());
        assert_eq!(h.hp, h.max_hp);
    }

    #[test]
    fn take_damage_reduces_hp() {
        let mut h = hero();
        h.take_damage(30);
        assert_eq!(h.hp, 70);
    }

    #[test]
    fn take_damage_clamps_at_zero() {
        let mut h = hero();
        h.take_damage(9999);
        assert_eq!(h.hp, 0);
        assert!(!h.is_alive());
    }

    #[test]
    fn movement_derived_from_spd() {
        let h = hero(); // spd = 15 → mov = 35
        assert_eq!(h.mov, Hero::movement_for_spd(15));
        assert_eq!(h.mov, 35);
        assert_eq!(h.mov_remaining, 35);
    }

    #[test]
    fn reset_movement_restores_full_mov() {
        let mut h = hero();
        h.mov_remaining = 0;
        h.reset_movement();
        assert_eq!(h.mov_remaining, h.mov);
    }
}
