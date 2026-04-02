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
/// Each hero's **own** RNG is consumed to compute their attack roll, so the
/// result depends on each hero's individual random state rather than a shared
/// session RNG.
pub fn resolve_combat(attacker: &mut Hero, defender: &mut Hero) -> CombatResult {
    // Higher spd goes first; ties go to attacker
    let attacker_first = attacker.spd >= defender.spd;

    let (att_deals, def_deals) = if attacker_first {
        // Attacker opens; defender counter-attacks only if it survived
        let att_dmg = calc_damage(attacker, defender);
        let def_dmg = if defender.hp.saturating_sub(att_dmg) > 0 {
            calc_damage(defender, attacker)
        } else {
            0
        };
        (att_dmg, def_dmg)
    } else {
        // Defender is faster — it opens; attacker counter-attacks only if it survived
        let def_dmg = calc_damage(defender, attacker);
        let att_dmg = if attacker.hp.saturating_sub(def_dmg) > 0 {
            calc_damage(attacker, defender)
        } else {
            0
        };
        (att_dmg, def_dmg)
    };

    // att_deals = damage dealt BY the attacker = damage TO the defender
    // def_deals = damage dealt BY the defender = damage TO the attacker
    CombatResult {
        attacker_damage: def_deals,
        defender_damage: att_deals,
        attacker_survived: attacker.hp.saturating_sub(def_deals) > 0,
        defender_survived: defender.hp.saturating_sub(att_deals) > 0,
    }
}

// ─── Internals ────────────────────────────────────────────────────────────────

/// Calculates damage dealt by `source` to `target`.
///
/// Uses `source.rng` — the attacker's personal RNG — for variance rolls.
fn calc_damage(source: &mut Hero, target: &Hero) -> u32 {
    let base = (source.atk as i32 - target.def as i32).max(1);
    let variance = (source.spd as i32 / 4).max(1);
    let roll = source.rng.random_range_i32(-variance..(variance + 1));
    (base + roll).max(1) as u32
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::game_map::MapCoord;
    use crate::rng::SeededRng;

    fn make_hero(id: u32, hp: u32, atk: u32, def: u32, spd: u32) -> Hero {
        let rng = SeededRng::new("combat-test").derive_for_hero(id);
        Hero::new(0, "Hero", hp, atk, def, spd, MapCoord::new(0, 0), 1, rng)
    }

    #[test]
    fn stronger_hero_wins_deterministically() {
        let mut strong = make_hero(1, 100, 50, 20, 20);
        let mut weak = make_hero(2, 20, 10, 5, 5);
        let result = resolve_combat(&mut strong, &mut weak);
        assert!(result.attacker_survived, "strong hero should survive");
        assert!(!result.defender_survived, "weak hero should be defeated");
    }

    #[test]
    fn defender_with_high_defense_takes_min_damage() {
        // atk=5, def=100 → base = max(1, 5-100) = 1, damage always ≥ 1
        let mut attacker = make_hero(1, 50, 5, 0, 4);
        let mut tank = make_hero(2, 50, 5, 100, 4);
        let result = resolve_combat(&mut attacker, &mut tank);
        assert!(result.defender_damage >= 1);
    }

    #[test]
    fn faster_unit_attacks_first() {
        // fast is the defender but has higher spd → attacks first and kills slow
        let mut fast = make_hero(1, 100, 200, 0, 100); // lethal damage in one hit
        let mut slow = make_hero(2, 10, 5, 0, 1);
        let result = resolve_combat(&mut slow, &mut fast);
        // slow (attacker) dies from fast's opening strike
        assert!(!result.attacker_survived, "slow attacker should die");
        // fast (defender) takes 0 damage — slow never got to counter-attack
        assert_eq!(
            result.defender_damage, 0,
            "fast defender should take no damage"
        );
    }

    #[test]
    fn same_seed_produces_same_result() {
        let mut a1 = make_hero(1, 50, 20, 10, 10);
        let mut b1 = make_hero(2, 50, 18, 12, 8);
        let mut a2 = make_hero(1, 50, 20, 10, 10);
        let mut b2 = make_hero(2, 50, 18, 12, 8);
        let result1 = resolve_combat(&mut a1, &mut b1);
        let result2 = resolve_combat(&mut a2, &mut b2);
        assert_eq!(result1.attacker_damage, result2.attacker_damage);
        assert_eq!(result1.defender_damage, result2.defender_damage);
    }
}
