//! Score system — tracks game events and computes a running total.

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::hero::HeroId;
use crate::map::game_map::MapCoord;

// ─── Point values ─────────────────────────────────────────────────────────────

const CITY_CAPTURE_POINTS: i32 = 500;
const ENEMY_DEFEATED_POINTS: i32 = 100;
const RESOURCE_COLLECTED_POINTS: i32 = 50;
const GOLD_COLLECTED_POINTS: i32 = 200;
const TURN_SURVIVED_POINTS: i32 = 10;

// ─── ScoreEvent ───────────────────────────────────────────────────────────────

/// A game event that contributes points to the score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScoreEvent {
    /// Player captured a city tile.
    CityCapture { city: MapCoord },
    /// Player defeated an enemy unit.
    EnemyDefeated { enemy_id: HeroId },
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
            ScoreEvent::CityCapture { .. } => CITY_CAPTURE_POINTS,
            ScoreEvent::EnemyDefeated { .. } => ENEMY_DEFEATED_POINTS,
            ScoreEvent::ResourceCollected { .. } => RESOURCE_COLLECTED_POINTS,
            ScoreEvent::GoldCollected { .. } => GOLD_COLLECTED_POINTS,
            ScoreEvent::TurnSurvived => TURN_SURVIVED_POINTS,
        }
    }
}

// ─── ScoreBoard ───────────────────────────────────────────────────────────────

/// Tracks all score events and computes a running total.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoreBoard {
    events: Vec<(ScoreEvent, i32)>,
    total: i32,
}

impl ScoreBoard {
    /// Creates an empty score board.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a score event and adds its point value to the total.
    pub fn record(&mut self, event: ScoreEvent) {
        let points = event.points();
        self.total += points;
        self.events.push((event, points));
    }

    /// Returns the current total score.
    pub fn total(&self) -> i32 {
        self.total
    }

    /// Returns all recorded events paired with their point values.
    pub fn events(&self) -> &[(ScoreEvent, i32)] {
        &self.events
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
        assert!(board.events().is_empty());
    }

    #[test]
    fn recording_events_accumulates_total() {
        let mut board = ScoreBoard::new();
        board.record(ScoreEvent::TurnSurvived);
        board.record(ScoreEvent::EnemyDefeated { enemy_id: 1 });
        assert_eq!(board.total(), TURN_SURVIVED_POINTS + ENEMY_DEFEATED_POINTS);
        assert_eq!(board.events().len(), 2);
    }

    #[test]
    fn city_capture_awards_correct_points() {
        let mut board = ScoreBoard::new();
        board.record(ScoreEvent::CityCapture {
            city: MapCoord::new(5, 5),
        });
        assert_eq!(board.total(), CITY_CAPTURE_POINTS);
    }

    #[test]
    fn gold_awards_more_than_resource() {
        let gold = ScoreEvent::GoldCollected {
            coord: MapCoord::new(0, 0),
        };
        let res = ScoreEvent::ResourceCollected {
            coord: MapCoord::new(0, 0),
        };
        assert!(gold.points() > res.points());
    }
}
