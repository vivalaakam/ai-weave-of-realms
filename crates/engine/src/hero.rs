//! Hero entity — represents a unit on the map belonging to any team.

use crate::hero_candidate::HeroCandidate;
use crate::map_coord::MapCoord;
use crate::rng::SeededRng;
use serde::{Deserialize, Serialize};
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
    pub(crate) id: HeroId,
    /// Hero class — determines sprite, base stats, and identity.
    pub(crate) class_id: u32,
    /// Sprite index in the shared tile atlas.
    #[serde(default)]
    pub(crate) atlas_index: usize,
    /// Display name.
    pub(crate) name: String,
    /// Current hit points.
    pub(crate) hp: u32,
    /// Maximum hit points.
    pub(crate) max_hp: u32,
    /// Attack power.
    pub(crate) atk: u32,
    /// Defence rating.
    pub(crate) def: u32,
    /// Speed (combat initiative).
    pub(crate) spd: u32,
    /// Total movement points per turn.
    pub(crate) mov: u32,
    /// Movement points remaining this turn.
    pub(crate) mov_remaining: u32,
    /// Current tile position on the map.
    ///
    /// This is the **authoritative** position.  Visual layers must query the
    /// engine for position rather than caching it themselves.
    pub(crate) position: MapCoord,
    /// The team this hero belongs to.
    /// Look up [`crate::team::Team`] in [`GameState::teams`] by this id to get the full team data.
    pub(crate) team_id: TeamId,
    /// Personal RNG for this hero, derived from the session seed.
    ///
    /// Used during combat to compute this hero's attack rolls.
    /// Derive with [`SeededRng::derive_for_hero`] from the session RNG.
    pub(crate) rng: SeededRng,
}

impl Hero {
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
    pub fn new(
        id: HeroId,
        hero: &HeroCandidate,
        position: &MapCoord,
        team_id: TeamId,
        seed: &SeededRng,
    ) -> Self {
        Self {
            id,
            class_id: hero.get_class_id(),
            atlas_index: hero.get_atlas_index(),
            name: hero.get_name().to_owned(),
            hp: hero.get_hp(),
            max_hp: hero.get_hp(),
            atk: hero.get_atk(),
            def: hero.get_def(),
            spd: hero.get_spd(),
            mov: hero.get_mov(),
            mov_remaining: hero.get_mov(),
            position: *position,
            team_id,
            rng: seed.update(&format!("hero_{}", id)),
        }
    }

    pub fn get_atlas_index(&self) -> usize {
        self.atlas_index
    }

    /// Returns `true` if the hero is still alive (`hp > 0`).
    pub fn is_alive(&self) -> bool {
        self.hp > 0
    }

    /// Applies `damage` to the hero, clamping HP at zero.
    pub(crate) fn take_damage(&mut self, damage: u32) {
        self.hp = self.hp.saturating_sub(damage);
    }

    /// Resets movement points to the full `mov` value (call at turn start).
    pub(crate) fn reset_movement(&mut self) {
        self.mov_remaining = self.mov;
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_hp(&self) -> u32 {
        self.hp
    }

    pub fn get_max_hp(&self) -> u32 {
        self.max_hp
    }

    pub fn get_atk(&self) -> u32 {
        self.atk
    }

    pub fn get_def(&self) -> u32 {
        self.def
    }

    pub fn get_spd(&self) -> u32 {
        self.spd
    }

    pub fn get_mov(&self) -> u32 {
        self.mov
    }

    pub fn get_mov_remaining(&self) -> u32 {
        self.mov_remaining
    }

    pub fn get_position(&self) -> &MapCoord {
        &self.position
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn hero() -> Hero {
        let hero_candidate = HeroCandidate {
            class_id: 0,
            name: "default_hero".to_owned(),
            description: "default_description".to_owned(),
            atlas_index: 0,
            cost: 50,
            hp: 10,
            atk: 10,
            def: 10,
            spd: 8,
        };

        let rnd = SeededRng::new("default");

        Hero::new(0, &hero_candidate, &MapCoord::new(0, 0), 0, &rnd)
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
        h.take_damage(3);
        assert_eq!(h.hp, 7);
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
        let h = hero(); // Knight spd = 8 → mov = 28
        assert_eq!(h.mov, Hero::movement_for_spd(8));
        assert_eq!(h.mov, 28);
        assert_eq!(h.mov_remaining, 28);
    }

    #[test]
    fn reset_movement_restores_full_mov() {
        let mut h = hero();
        h.mov_remaining = 0;
        h.reset_movement();
        assert_eq!(h.mov_remaining, h.mov);
    }

    #[test]
    fn atlas_index_comes_from_candidate_not_class_id() {
        let h = hero();
        assert_eq!(h.class_id, 0);
        assert_eq!(h.get_atlas_index(), 0);

        let hero_candidate = HeroCandidate {
            class_id: 0,
            name: "knight".to_owned(),
            description: "default_description".to_owned(),
            atlas_index: 24,
            cost: 50,
            hp: 10,
            atk: 10,
            def: 10,
            spd: 8,
        };
        let rnd = SeededRng::new("default");
        let h = Hero::new(0, &hero_candidate, &MapCoord::new(0, 0), 0, &rnd);

        assert_eq!(h.class_id, 0);
        assert_eq!(h.get_atlas_index(), 24);
    }
}
