//! Hero entity — represents a unit on the map belonging to any team.

use alloc::{format, string::String};

use serde::{Deserialize, Serialize};

use crate::hero_class::HeroClass;
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
    /// Hero class — determines sprite, base stats, and identity.
    pub class: HeroClass,
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
    pub(crate) fn reset(&mut self, id: HeroId, seed: &SeededRng) {
        self.id = id;
        self.rng = seed.update(&format!("hero_{}", self.id));
    }

    pub fn get_id(&self) -> HeroId {
        self.id
    }

    pub fn get_team_id(&self) -> TeamId {
        self.team_id
    }

    /// Computes the total movement points for a hero with the given speed.
    ///
    /// Formula: `20 + spd`
    pub fn movement_for_spd(spd: u32) -> u32 {
        20 + spd
    }

    /// Creates a new hero with full HP and full movement points.
    ///
    /// Stats (hp, atk, def, spd) are taken from the hero class via
    /// [`HeroClass::base_hp`], etc. The provided `spd` parameter is **ignored**
    /// in favour of the class base — use [`Hero::new_with_stats`] to override.
    ///
    /// Movement is derived automatically from spd via [`Hero::movement_for_spd`].
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: HeroId,
        class: HeroClass,
        name: impl Into<String>,
        position: MapCoord,
        team_id: TeamId,
    ) -> Self {
        let hp = class.base_hp();
        let atk = class.base_atk();
        let def = class.base_def();
        let spd = class.base_spd();
        let mov = Self::movement_for_spd(spd);
        Self {
            id,
            class,
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
            rng: SeededRng::new("default"),
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

    /// Creates a hero with **custom stats** overriding the class defaults.
    ///
    /// Use this in tests and save-file loading where stats must be precise.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_stats(
        id: HeroId,
        class: HeroClass,
        name: impl Into<String>,
        hp: u32,
        atk: u32,
        def: u32,
        spd: u32,
        position: MapCoord,
        team_id: TeamId,
    ) -> Self {
        let mov = Self::movement_for_spd(spd);
        Self {
            id,
            class,
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
            rng: SeededRng::new("default"),
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn hero() -> Hero {
        Hero::new(0, HeroClass::Knight, "Arthur", MapCoord::new(0, 0), 1)
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
