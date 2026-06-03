//! Score system — per-team scoring and territory expansion tracking.
//!
//! Each team accumulates points from multiple sources:
//! - City ownership
//! - Land (territory) tiles
//! - Resource points
//! - Resource rods
//! - Living heroes
//!
//! The [`ScoreBoard`] also maintains per-team expansion state that controls
//! when cities and rods grow their territory outward.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::hero::TeamId;
use crate::map_coord::MapCoord;

// ─── Point values ─────────────────────────────────────────────────────────────

/// Points awarded per city tile owned.
pub const CITY_TILE_POINTS: i32 = 50;
/// Points awarded per land (territory) tile owned.
pub const LAND_TILE_POINTS: i32 = 10;
/// Points awarded per owned resource point.
pub const RESOURCE_POINT_POINTS: i32 = 30;
/// Points awarded per resource rod placed.
pub const ROD_POINTS: i32 = 20;
/// Points awarded per living hero.
pub const HERO_ALIVE_POINTS: i32 = 15;

// ─── Territory expansion constants ────────────────────────────────────────────

/// Cities expand claimed territory by 1 tile every N team turns.
pub const CITY_EXPANSION_INTERVAL: u32 = 5;
/// Rods expand claimed territory by 1 tile every N team turns.
pub const ROD_EXPANSION_INTERVAL: u32 = 10;
/// Initial claim radius around a city (Manhattan distance from any city tile).
pub const CITY_INITIAL_RADIUS: u32 = 2;

// ─── ScoreEvent ───────────────────────────────────────────────────────────────

/// A game event that contributes points to a team's score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScoreEvent {
    /// Player captured a city tile.
    CityCapture { city: MapCoord },
    /// Player defeated an enemy unit.
    EnemyDefeated { enemy_id: crate::hero::HeroId },
    /// Player collected a resource deposit.
    ResourceCollected { coord: MapCoord },
    /// Player collected a gold mine.
    GoldCollected { coord: MapCoord },
    /// Player survived a full turn.
    TurnSurvived,
}

impl ScoreEvent {
    /// Returns the point value awarded for this event.
    pub fn points(&self) -> i32 {
        match self {
            ScoreEvent::CityCapture { .. } => 500,
            ScoreEvent::EnemyDefeated { .. } => 100,
            ScoreEvent::ResourceCollected { .. } => 50,
            ScoreEvent::GoldCollected { .. } => 200,
            ScoreEvent::TurnSurvived => 10,
        }
    }
}

// ─── TeamScore ────────────────────────────────────────────────────────────────

/// Running score breakdown for a single team.
///
/// Stores event history and provides a method to compute the current score
/// from live game state (cities, territory, resources, rods, heroes).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TeamScore {
    /// Accumulated event-based points (captures, defeats, etc.).
    event_points: i32,
    /// History of score events for this team.
    events: Vec<(ScoreEvent, i32)>,
}

impl TeamScore {
    /// Creates an empty team score.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a score event and adds its point value.
    pub fn record(&mut self, event: ScoreEvent) {
        let points = event.points();
        self.event_points += points;
        self.events.push((event, points));
    }

    /// Returns the event-based point total (captures, defeats, etc.).
    pub fn event_points(&self) -> i32 {
        self.event_points
    }

    /// Returns all recorded events paired with their point values.
    pub fn events(&self) -> &[(ScoreEvent, i32)] {
        &self.events
    }
}

// ─── ScoreBoard ───────────────────────────────────────────────────────────────

/// Per-team score board that tracks events and expansion state.
///
/// Each team gets its own [`TeamScore`] plus expansion counters that control
/// when territory grows outward from cities and rods.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoreBoard {
    /// Per-team score entries, keyed by [`TeamId`].
    teams: alloc::collections::BTreeMap<TeamId, TeamScore>,
}

impl ScoreBoard {
    /// Creates an empty score board.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ensures a team score entry exists for `team_id`, inserting a default
    /// if absent.  Returns a mutable reference to it.
    pub fn team_mut(&mut self, team_id: TeamId) -> &mut TeamScore {
        self.teams.entry(team_id).or_default()
    }

    /// Records a score event for the given team.
    pub fn record_for(&mut self, team_id: TeamId, event: ScoreEvent) {
        self.team_mut(team_id).record(event);
    }

    /// Returns the legacy global total (sum of all team event points).
    ///
    /// Kept for backward compatibility with [`WinCondition::ScoreThreshold`].
    pub fn total(&self) -> i32 {
        self.teams.values().map(|t| t.event_points()).sum()
    }

    /// Returns the event-based score for one team, or 0 if the team has
    /// never scored.
    pub fn team_total(&self, team_id: TeamId) -> i32 {
        self.teams.get(&team_id).map(|t| t.event_points()).unwrap_or(0)
    }
}

/// Computed score breakdown for a single team, derived from live state.
#[derive(Debug, Clone, Default)]
pub struct ScoreBreakdown {
    pub cities: i32,
    pub land: i32,
    pub resources: i32,
    pub rods: i32,
    pub heroes: i32,
    pub events: i32,
}

impl ScoreBreakdown {
    /// Total points across all categories.
    pub fn total(&self) -> i32 {
        self.cities + self.land + self.resources + self.rods + self.heroes + self.events
    }
}

impl fmt::Display for ScoreBreakdown {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "C:{} L:{} R:{} Rod:{} H:{} Ev:{} = {}",
            self.cities,
            self.land,
            self.resources,
            self.rods,
            self.heroes,
            self.events,
            self.total()
        )
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_board_has_zero_score() {
        let board = ScoreBoard::new();
        assert_eq!(board.total(), 0);
        assert_eq!(board.team_total(0), 0);
    }

    #[test]
    fn recording_events_accumulates_total() {
        let mut board = ScoreBoard::new();
        board.record_for(0, ScoreEvent::TurnSurvived);
        board.record_for(0, ScoreEvent::EnemyDefeated { enemy_id: 1 });
        assert_eq!(board.team_total(0), 10 + 100);
        assert_eq!(board.total(), 110);
    }

    #[test]
    fn per_team_scores_are_independent() {
        let mut board = ScoreBoard::new();
        board.record_for(0, ScoreEvent::TurnSurvived);
        board.record_for(1, ScoreEvent::TurnSurvived);
        board.record_for(1, ScoreEvent::CityCapture { city: MapCoord::new(5, 5) });
        assert_eq!(board.team_total(0), 10);
        assert_eq!(board.team_total(1), 10 + 500);
        assert_eq!(board.total(), 520);
    }

    #[test]
    fn city_capture_awards_correct_points() {
        let mut board = ScoreBoard::new();
        board.record_for(0, ScoreEvent::CityCapture { city: MapCoord::new(5, 5) });
        assert_eq!(board.team_total(0), 500);
    }

    #[test]
    fn gold_awards_more_than_resource() {
        let gold = ScoreEvent::GoldCollected { coord: MapCoord::new(0, 0) };
        let res = ScoreEvent::ResourceCollected { coord: MapCoord::new(0, 0) };
        assert!(gold.points() > res.points());
    }

    #[test]
    fn score_breakdown_total_sums_all_categories() {
        let bd = ScoreBreakdown {
            cities: 100,
            land: 200,
            resources: 50,
            rods: 20,
            heroes: 30,
            events: 10,
        };
        assert_eq!(bd.total(), 410);
    }
}
