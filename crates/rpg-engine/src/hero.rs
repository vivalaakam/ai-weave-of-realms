//! Hero entity — represents a player or enemy unit on the map.

use serde::{Deserialize, Serialize};

use crate::map::game_map::MapCoord;

// ─── Faction ──────────────────────────────────────────────────────────────────

/// Which side a hero belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Faction {
    Player,
    Enemy,
}

// ─── Hero ─────────────────────────────────────────────────────────────────────

/// A hero unit on the game map.
///
/// Stats:
/// - `hp` / `max_hp` — health pool
/// - `atk` — attack power (raw damage before defence)
/// - `def` — defence rating (reduces incoming damage)
/// - `spd` — speed (determines combat initiative)
/// - `mov` — movement points per turn
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hero {
    /// Unique identifier within the game session.
    pub id: u32,
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
    pub position: MapCoord,
    /// Player or enemy.
    pub faction: Faction,
}

impl Hero {
    /// Creates a new hero with full HP and full movement points.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: u32,
        name: impl Into<String>,
        hp: u32,
        atk: u32,
        def: u32,
        spd: u32,
        mov: u32,
        position: MapCoord,
        faction: Faction,
    ) -> Self {
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
            faction,
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
        Hero::new(1, "Arthur", 100, 20, 10, 15, 4, MapCoord::new(0, 0), Faction::Player)
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
    fn reset_movement_restores_full_mov() {
        let mut h = hero();
        h.mov_remaining = 0;
        h.reset_movement();
        assert_eq!(h.mov_remaining, h.mov);
    }
}
