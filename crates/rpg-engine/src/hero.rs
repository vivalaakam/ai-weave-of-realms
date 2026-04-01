//! Hero entity — represents a unit on the map belonging to any team.

use serde::{Deserialize, Serialize};

use crate::map::game_map::MapCoord;

// ─── Team ─────────────────────────────────────────────────────────────────────

/// The team a hero belongs to.
///
/// All units — player heroes and enemies alike — are heroes in a `Team`.
/// The `player_controlled` flag is the only thing that distinguishes a team the
/// human commands from one driven by AI.
///
/// # Examples
/// ```
/// use rpg_engine::hero::Team;
///
/// let player_team = Team::player();
/// assert!(player_team.player_controlled);
///
/// let enemy_team = Team::enemy();
/// assert!(!enemy_team.player_controlled);
///
/// let bandit_team = Team::new("bandits", false);
/// assert_eq!(bandit_team.name, "bandits");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    /// Human-readable team identifier (e.g. `"player"`, `"enemy"`).
    pub name: String,
    /// `true` if the human player can select and command heroes on this team.
    pub player_controlled: bool,
}

impl Team {
    /// Creates a team with the given name and `player_controlled` flag.
    pub fn new(name: impl Into<String>, player_controlled: bool) -> Self {
        Self { name: name.into(), player_controlled }
    }

    /// Convenience constructor for the human player's team.
    pub fn player() -> Self {
        Self::new("player", true)
    }

    /// Convenience constructor for a standard AI-controlled enemy team.
    pub fn enemy() -> Self {
        Self::new("enemy", false)
    }
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
///
/// Identity:
/// - `team` — which team this hero belongs to, and whether that team is
///   player-controlled
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
    ///
    /// This is the **authoritative** position.  Visual layers must query the
    /// engine for position rather than caching it themselves.
    pub position: MapCoord,
    /// The team this hero belongs to.
    pub team: Team,
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
        team: Team,
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
            team,
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
        Hero::new(1, "Arthur", 100, 20, 10, 15, 4, MapCoord::new(0, 0), Team::player())
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

    #[test]
    fn team_player_controlled_flag() {
        assert!(Team::player().player_controlled);
        assert!(!Team::enemy().player_controlled);
        let bandits = Team::new("bandits", false);
        assert_eq!(bandits.name, "bandits");
        assert!(!bandits.player_controlled);
    }
}
