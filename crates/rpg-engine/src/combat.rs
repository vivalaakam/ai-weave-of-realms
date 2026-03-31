//! Auto-resolve combat system.
//!
//! Combat is fully deterministic given a [`SeededRng`] state.
//!
//! ## Resolution order
//! 1. The unit with higher `spd` attacks first.
//! 2. The defending unit counter-attacks only if it survived the first strike.
//!
//! ## Damage formula
//! ```text
//! base   = max(1, atk − def)
//! roll   = rng in [−variance, +variance]  where variance = max(1, spd / 4)
//! damage = max(1, base + roll)
//! ```

use serde::{Deserialize, Serialize};

use crate::hero::Hero;
use crate::rng::SeededRng;

// ─── CombatResult ─────────────────────────────────────────────────────────────

/// Outcome of a single auto-resolved combat encounter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatResult {
    /// Damage dealt to the attacker.
    pub attacker_damage: u32,
    /// Damage dealt to the defender.
    pub defender_damage: u32,
    /// Whether the attacker survived.
    pub attacker_survived: bool,
    /// Whether the defender survived.
    pub defender_survived: bool,
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Resolves a combat encounter between `attacker` and `defender`.
///
/// The unit with higher `spd` acts first.  The slower unit only
/// counter-attacks if it survived the opening strike.
///
/// The RNG is advanced during resolution; the result is therefore
/// deterministic for a given `rng` state.
pub fn resolve_combat(attacker: &Hero, defender: &Hero, rng: &mut SeededRng) -> CombatResult {
    // Higher spd goes first; ties go to attacker
    let attacker_first = attacker.spd >= defender.spd;

    let (first, second) = if attacker_first {
        (attacker, defender)
    } else {
        (defender, attacker)
    };

    let first_deals = calc_damage(first, second, rng);
    let second_hp_after = second.hp.saturating_sub(first_deals);

    // Second unit counter-attacks only if it survived
    let second_deals = if second_hp_after > 0 {
        calc_damage(second, first, rng)
    } else {
        0
    };

    let (attacker_damage, defender_damage) = if attacker_first {
        (second_deals, first_deals)
    } else {
        (first_deals, second_deals)
    };

    CombatResult {
        attacker_damage,
        defender_damage,
        attacker_survived: attacker.hp.saturating_sub(attacker_damage) > 0,
        defender_survived: defender.hp.saturating_sub(defender_damage) > 0,
    }
}

// ─── Internals ────────────────────────────────────────────────────────────────

/// Calculates damage from `source` to `target` with RNG variance.
fn calc_damage(source: &Hero, target: &Hero, rng: &mut SeededRng) -> u32 {
    let base = (source.atk as i32 - target.def as i32).max(1);
    let variance = (source.spd as i32 / 4).max(1);
    // Range is [-variance, +variance] exclusive of +variance endpoint, add 1
    let roll = rng.random_range_i32(-variance..(variance + 1));
    (base + roll).max(1) as u32
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero::Faction;
    use crate::map::game_map::MapCoord;

    fn make_hero(id: u32, hp: u32, atk: u32, def: u32, spd: u32) -> Hero {
        Hero::new(id, "Hero", hp, atk, def, spd, 4, MapCoord::new(0, 0), Faction::Player)
    }

    #[test]
    fn stronger_hero_wins_deterministically() {
        let strong = make_hero(1, 100, 50, 20, 20);
        let weak   = make_hero(2,  20, 10,  5,  5);
        let mut rng = SeededRng::new("combat-test");
        let result = resolve_combat(&strong, &weak, &mut rng);
        assert!(result.attacker_survived, "strong hero should survive");
        assert!(!result.defender_survived, "weak hero should be defeated");
    }

    #[test]
    fn defender_with_high_defense_takes_min_damage() {
        // atk=5, def=100 → base = max(1, 5-100) = 1, damage always ≥ 1
        let attacker = make_hero(1, 50, 5, 0, 4);
        let tank     = make_hero(2, 50, 5, 100, 4);
        let mut rng = SeededRng::new("tank-test");
        let result = resolve_combat(&attacker, &tank, &mut rng);
        assert!(result.defender_damage >= 1);
    }

    #[test]
    fn faster_unit_attacks_first() {
        // fast is the defender but has higher spd → attacks first and kills slow
        let fast = make_hero(1, 100, 200, 0, 100); // lethal damage in one hit
        let slow = make_hero(2,  10,   5, 0,   1);
        let mut rng = SeededRng::new("speed-test");
        let result = resolve_combat(&slow, &fast, &mut rng);
        // slow (attacker) dies from fast's opening strike
        assert!(!result.attacker_survived, "slow attacker should die");
        // fast (defender) takes 0 damage — slow never got to counter-attack
        assert_eq!(result.defender_damage, 0, "fast defender should take no damage");
    }

    #[test]
    fn same_seed_produces_same_result() {
        let a = make_hero(1, 50, 20, 10, 10);
        let b = make_hero(2, 50, 18, 12,  8);
        let result1 = resolve_combat(&a, &b, &mut SeededRng::new("same"));
        let result2 = resolve_combat(&a, &b, &mut SeededRng::new("same"));
        assert_eq!(result1.attacker_damage, result2.attacker_damage);
        assert_eq!(result1.defender_damage, result2.defender_damage);
    }
}
