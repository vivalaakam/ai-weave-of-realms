//! Hero entity — represents a unit on the map belonging to any team.

use serde::{Deserialize, Serialize};

use crate::map::game_map::MapCoord;
use crate::rng::SeededRng;

// ─── TeamId ───────────────────────────────────────────────────────────────────

/// Numeric identifier for a team (0-8, allowing up to 9 teams).
pub type TeamId = u8;

// ─── TeamInfo ─────────────────────────────────────────────────────────────────

/// Team configuration: identity, display name, and color.
///
/// Used to define player and AI teams in the game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamInfo {
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

impl TeamInfo {
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

    /// Creates a default Red player team.
    pub fn red() -> Self {
        Self::new(1, "Red", (220, 50, 50), true)
    }

    /// Creates a default Blue player team.
    pub fn blue() -> Self {
        Self::new(2, "Blue", (50, 100, 220), true)
    }

    /// Creates a default AI-controlled enemy team.
    pub fn enemy() -> Self {
        Self::new(3, "Enemy", (150, 80, 200), false)
    }
}

// ─── Team (legacy) ─────────────────────────────────────────────────────────────

/// The team a hero belongs to (legacy inline representation).
///
/// For new code, prefer storing `team_id: TeamId` in heroes and
/// looking up `TeamInfo` from `GameState::teams`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    /// Human-readable team identifier (e.g. `"red"`, `"blue"`, `"enemy"`).
    pub name: String,
    /// `true` if the human player can select and command heroes on this team.
    pub player_controlled: bool,
}

impl Team {
    /// Creates a team with the given name and `player_controlled` flag.
    pub fn new(name : impl Into<String>, player_controlled: bool) -> Self {
        Self {
            name: name.into(),
            player_controlled,
        }
    }

    /// Returns the display colour for this team as an `(R, G, B)` tuple.
    ///
    /// Colours are derived from the team name:
    /// - `"red"`  → `(220, 50, 50)`
    /// - `"blue"` → `(50, 100, 220)`
    /// - anything else → `(150, 80, 200)` (purple, used for AI enemies)
    pub fn color(&self) -> (u8, u8, u8) {
        match self.name.as_str() {
            "red" => (220, 50, 50),
            "blue" => (50, 100, 220),
            _ => (150, 80, 200),
        }
    }

    /// Convenience constructor for the **Red** player team.
    pub fn red() -> Self {
        Self::new("red", true)
    }

    /// Convenience constructor for the **Blue** player team.
    pub fn blue() -> Self {
        Self::new("blue", true)
    }

    /// Convenience constructor for the AI-controlled **enemy** team.
    pub fn enemies() -> Self {
        Self::new("enemy", false)
    }

    /// Convenience constructor for the human player's team (legacy; prefer [`Team::red`]).
    pub fn player() -> Self {
        Self::new("player", true)
    }

    /// Convenience constructor for a standard AI-controlled enemy team (legacy; prefer [`Team::enemies`]).
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
    /// Personal RNG for this hero, derived from the session seed.
    ///
    /// Used during combat to compute this hero's attack rolls.
    /// Derive with [`SeededRng::derive_for_hero`] from the session RNG.
    pub rng: SeededRng,
}

impl Hero {
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
        id: u32,
        name: impl Into<String>,
        hp: u32,
        atk: u32,
        def: u32,
        spd: u32,
        position: MapCoord,
        team: Team,
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
            team,
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
        Hero::new(
            1,
            "Arthur",
            100,
            20,
            10,
            15,
            MapCoord::new(0, 0),
            Team::player(),
            rng,
        )
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

    #[test]
    fn team_player_controlled_flag() {
        assert!(Team::player().player_controlled);
        assert!(!Team::enemy().player_controlled);
        let bandits = Team::new("bandits", false);
        assert_eq!(bandits.name, "bandits");
        assert!(!bandits.player_controlled);
    }
}
