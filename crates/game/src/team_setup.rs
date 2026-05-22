//! Team setup screen — configure team names, colors, and controllers before starting a game.
//!
//! This screen appears after selecting a map (or random map) but before entering gameplay.
use crate::input::InputEvent;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::time::Duration;

// ─── Vocabulary for team name generation ────────────────────────────────────────

const ADJECTIVES: [&str; 30] = [
    "Ember", "Crimson", "Azure", "Golden", "Shadow", "Iron", "Silver",
    "Storm", "Frost", "Blood", "Dragon", "Phoenix", "Thunder", "Night",
    "Dawn", "Eternal", "Sacred", "Ancient", "Noble", "Savage",
    "Wild", "Dark", "Bright", "Royal", "Venom", "Crystal", "Flame",
    "Phantom", "Solar", "Void",
];

const NOUNS: [&str; 30] = [
    "Legion", "Vanguard", "Order", "Clan", "Guild", "Empire", "Kingdom",
    "Horde", "Tribe", "Covenant", "Alliance", "Fellowship", "Brotherhood",
    "Dominion", "Republic", "Dynasty", "Host", "Swarm", "Collective",
    "Circle", "Syndicate", "Cult", "Squad", "Brigade", "Regiment",
    "Battalion", "Phalanx", "Guard", "Watch", "Sentinels",
];

/// Generates a deterministic but varied team name based on index.
pub fn generate_team_name(index: usize) -> String {
    let adj = ADJECTIVES[index % ADJECTIVES.len()];
    let noun = NOUNS[(index / ADJECTIVES.len()) % NOUNS.len()];
    let mut s = String::with_capacity(adj.len() + 1 + noun.len());
    s.push_str(adj);
    s.push(' ');
    s.push_str(noun);
    s
}

/// Generate a distinct color for team at `index` out of `total` teams.
pub fn generate_team_color(index: usize, total: usize) -> (u8, u8, u8) {
    // Distribute hues evenly around the color wheel.
    let hue = if total > 0 {
        (index as f64 / total as f64 * 360.0) as u16 % 360
    } else {
        0
    };
    // 75% saturation, 55% lightness = vivid but not blinding.
    hsl_to_rgb(hue, 75, 55)
}

/// Convert HSL (0-360 hue, 0-100 sat/light) to RGB.
fn hsl_to_rgb(h: u16, s: u8, l: u8) -> (u8, u8, u8) {
    let h = h as f64 / 360.0;
    let s = s as f64 / 100.0;
    let l = l as f64 / 100.0;

    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r1, g1, b1) = match (h * 6.0) as u8 {
        0..=0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    (
        ((r1 + m) * 255.0).clamp(0.0, 255.0) as u8,
        ((g1 + m) * 255.0).clamp(0.0, 255.0) as u8,
        ((b1 + m) * 255.0).clamp(0.0, 255.0) as u8,
    )
}

// ─── TeamConfig ───────────────────────────────────────────────────────────────

/// Configuration for a single team before game start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamConfig {
    /// Human-readable team name.
    pub name: String,
    /// Display color (R, G, B).
    pub color: (u8, u8, u8),
    /// `true` if the human player controls this team.
    pub player_controlled: bool,
}

impl TeamConfig {
    /// Create a default team config.
    pub fn new(name: String, color: (u8, u8, u8), player_controlled: bool) -> Self {
        Self { name, color, player_controlled }
    }
}

// ─── TeamSetupScreen ──────────────────────────────────────────────────────────

/// Row types in the team setup screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupRow {
    /// Row for selecting the number of teams.
    TeamCount,
    /// Row for a specific team's settings (index into `teams`).
    Team { index: usize, field: TeamField },
    /// Play button row.
    Play,
    /// Back button row.
    Back,
}

/// Which field of a team row is focused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamField {
    Name,
    Color,
    Controller,
}

/// Screen model for team configuration before starting a game.
pub struct TeamSetupScreen {
    /// Number of teams (1–8).
    pub team_count: usize,
    /// Which row is currently selected.
    pub selected_row: usize,
    /// Team configurations.
    pub teams: Vec<TeamConfig>,
    /// Index of the team controlled by the human player.
    pub player_team_index: usize,
    /// Optional status / error line.
    pub status: Option<String>,
}

/// Result of applying one input event to the team setup screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeamSetupOutcome {
    /// Nothing changed.
    NoChange,
    /// Selection or data changed.
    Changed,
    /// User pressed Play — ready to build the game state and start.
    PlayRequested,
    /// User pressed Back.
    BackRequested,
}

impl TeamSetupScreen {
    /// Minimum number of teams.
    pub const MIN_TEAMS: usize = 1;
    /// Maximum number of teams.
    pub const MAX_TEAMS: usize = 8;

    /// Creates a new team setup screen with the default number of teams.
    pub fn new(default_count: usize) -> Self {
        let count = default_count.clamp(Self::MIN_TEAMS, Self::MAX_TEAMS);
        let teams = generate_teams(count, 0);
        Self {
            team_count: count,
            selected_row: 1, // Skip TeamCount, start at first team name
            teams,
            player_team_index: 0,
            status: None,
        }
    }

    /// Rebuilds teams when the count changes, preserving existing configs where possible.
    fn rebuild_teams(&mut self, new_count: usize) {
        let new_count = new_count.clamp(Self::MIN_TEAMS, Self::MAX_TEAMS);
        let old_teams = core::mem::take(&mut self.teams);
        let mut new_teams = Vec::with_capacity(new_count);
        for i in 0..new_count {
            if i < old_teams.len() {
                new_teams.push(old_teams[i].clone());
            } else {
                new_teams.push(TeamConfig::new(
                    generate_team_name(i + old_teams.len()),
                    generate_team_color(i, new_count),
                    false,
                ));
            }
        }
        self.teams = new_teams;
        self.team_count = new_count;
        // Ensure player_team_index stays valid.
        if self.player_team_index >= new_count {
            self.player_team_index = new_count.saturating_sub(1);
        }
    }

    /// Total number of rows (team count row + team rows * 3 + Play + Back).
    pub fn total_rows(&self) -> usize {
        // Row 0: team count
        // Rows 1..team_count*3: team fields (name, color, controller)
        // Next: Play
        // Next: Back
        1 + self.team_count * 3 + 2
    }

    /// Maps a row index to a SetupRow.
    fn row_to_setup(&self, row: usize) -> SetupRow {
        if row == 0 {
            SetupRow::TeamCount
        } else if row <= self.team_count * 3 {
            let team_idx = (row - 1) / 3;
            let field = match (row - 1) % 3 {
                0 => TeamField::Name,
                1 => TeamField::Color,
                _ => TeamField::Controller,
            };
            SetupRow::Team { index: team_idx, field }
        } else if row == self.team_count * 3 + 1 {
            SetupRow::Play
        } else {
            SetupRow::Back
        }
    }

    /// Applies a single input event.
    pub fn handle_input(&mut self, event: InputEvent) -> TeamSetupOutcome {
        match event {
            InputEvent::Up => {
                let prev = self.selected_row;
                self.selected_row = self.selected_row.saturating_sub(1);
                if self.selected_row != prev {
                    TeamSetupOutcome::Changed
                } else {
                    TeamSetupOutcome::NoChange
                }
            }
            InputEvent::Down => {
                let prev = self.selected_row;
                let last = self.total_rows().saturating_sub(1);
                self.selected_row = (self.selected_row + 1).min(last);
                if self.selected_row != prev {
                    TeamSetupOutcome::Changed
                } else {
                    TeamSetupOutcome::NoChange
                }
            }
            InputEvent::Left | InputEvent::Right => {
                self.handle_field_adjust(event)
            }
            InputEvent::Enter => self.handle_enter(),
            InputEvent::Back => TeamSetupOutcome::BackRequested,
            _ => TeamSetupOutcome::NoChange,
        }
    }

    fn handle_field_adjust(&mut self, event: InputEvent) -> TeamSetupOutcome {
        match self.row_to_setup(self.selected_row) {
            SetupRow::TeamCount => {
                let delta = if matches!(event, InputEvent::Left) { -1i32 } else { 1i32 };
                let new_count = (self.team_count as i32 + delta)
                    .clamp(Self::MIN_TEAMS as i32, Self::MAX_TEAMS as i32) as usize;
                if new_count != self.team_count {
                    self.rebuild_teams(new_count);
                    TeamSetupOutcome::Changed
                } else {
                    TeamSetupOutcome::NoChange
                }
            }
            SetupRow::Team { index, field: TeamField::Color } => {
                // Shift hue by 15° left/right.
                let (r, g, b) = self.teams[index].color;
                let shift: i32 = if matches!(event, InputEvent::Left) { -15 } else { 15 };
                self.teams[index].color = shift_color((r, g, b), shift);
                TeamSetupOutcome::Changed
            }
            SetupRow::Team { index, field: TeamField::Controller } => {
                self.player_team_index = index;
                for (i, team) in self.teams.iter_mut().enumerate() {
                    team.player_controlled = i == index;
                }
                TeamSetupOutcome::Changed
            }
            _ => TeamSetupOutcome::NoChange,
        }
    }

    fn handle_enter(&mut self) -> TeamSetupOutcome {
        match self.row_to_setup(self.selected_row) {
            SetupRow::TeamCount => {
                // Cycle through common counts: 2 → 3 → 4 → 5 → 6 → 7 → 8 → 1 → 2
                let new_count = if self.team_count >= Self::MAX_TEAMS {
                    Self::MIN_TEAMS
                } else {
                    self.team_count + 1
                };
                self.rebuild_teams(new_count);
                TeamSetupOutcome::Changed
            }
            SetupRow::Team { index, field: TeamField::Name } => {
                self.teams[index].name = generate_team_name(index);
                TeamSetupOutcome::Changed
            }
            SetupRow::Team { index, field: TeamField::Color } => {
                self.teams[index].color = generate_team_color(index, self.team_count);
                TeamSetupOutcome::Changed
            }
            SetupRow::Team { index, field: TeamField::Controller } => {
                self.player_team_index = index;
                for (i, team) in self.teams.iter_mut().enumerate() {
                    team.player_controlled = i == index;
                }
                TeamSetupOutcome::Changed
            }
            SetupRow::Play => {
                // Validate: at least one team must be human-controlled.
                if !self.teams.iter().any(|t| t.player_controlled) {
                    self.status = Some("At least one team must be human-controlled".to_string());
                    return TeamSetupOutcome::Changed;
                }
                TeamSetupOutcome::PlayRequested
            }
            SetupRow::Back => TeamSetupOutcome::BackRequested,
        }
    }

    /// Returns the label text for a given row index.
    pub fn row_label(&self, row: usize) -> String {
        match self.row_to_setup(row) {
            SetupRow::TeamCount => format!("Teams: {}", self.team_count),
            SetupRow::Team { index, field } => match field {
                TeamField::Name => format!("  Name: {}", self.teams[index].name),
                TeamField::Color => {
                    let (r, g, b) = self.teams[index].color;
                    format!("  Color: ({}, {}, {})", r, g, b)
                }
                TeamField::Controller => {
                    let ctrl = if self.teams[index].player_controlled { "Human" } else { "AI" };
                    format!("  Controller: {}", ctrl)
                }
            },
            SetupRow::Play => "Play".to_string(),
            SetupRow::Back => "Back".to_string(),
        }
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────────

fn generate_teams(count: usize, _seed_offset: u64) -> Vec<TeamConfig> {
    let mut teams = Vec::with_capacity(count);
    for i in 0..count {
        teams.push(TeamConfig::new(
            generate_team_name(i),
            generate_team_color(i, count),
            i == 0, // First team is human by default
        ));
    }
    teams
}

/// Shift a color's hue by `degrees` (positive = right on color wheel).
fn shift_color(rgb: (u8, u8, u8), degrees: i32) -> (u8, u8, u8) {
    let (h, s, l) = rgb_to_hsl(rgb.0, rgb.1, rgb.2);
    let new_h = ((h as i32 + degrees).rem_euclid(360)) as u16;
    hsl_to_rgb(new_h, s, l)
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (u16, u8, u8) {
    let rf = r as f64 / 255.0;
    let gf = g as f64 / 255.0;
    let bf = b as f64 / 255.0;

    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let l = (max + min) / 2.0;

    let s = if max == min {
        0.0
    } else {
        let d = max - min;
        if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) }
    };

    let h = if max == min {
        0.0
    } else if max == rf {
        60.0 * ((gf - bf) / (max - min)).rem_euclid(6.0)
    } else if max == gf {
        60.0 * ((bf - rf) / (max - min)) + 120.0
    } else {
        60.0 * ((rf - gf) / (max - min)) + 240.0
    };

    (
        (h.rem_euclid(360.0)) as u16,
        (s * 100.0).clamp(0.0, 100.0) as u8,
        (l * 100.0).clamp(0.0, 100.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_name_is_non_empty() {
        let name = generate_team_name(0);
        assert!(!name.is_empty());
        assert!(name.contains(' '));
    }

    #[test]
    fn generate_color_is_valid() {
        for i in 0..8 {
            let (r, g, b) = generate_team_color(i, 8);
            assert!(r <= 255 && g <= 255 && b <= 255);
        }
    }

    #[test]
    fn team_setup_navigates() {
        let mut screen = TeamSetupScreen::new(2);
        assert_eq!(screen.team_count, 2);
        assert_eq!(screen.teams[0].player_controlled, true);
        assert_eq!(screen.teams[1].player_controlled, false);

        // Navigate down through rows
        assert!(matches!(screen.handle_input(InputEvent::Down), TeamSetupOutcome::Changed));
        assert!(matches!(screen.handle_input(InputEvent::Down), TeamSetupOutcome::Changed));
        assert!(matches!(screen.handle_input(InputEvent::Down), TeamSetupOutcome::Changed));

        // Change controller to team 1
        screen.selected_row = 6; // Team 1, controller field
        assert!(matches!(screen.handle_input(InputEvent::Enter), TeamSetupOutcome::Changed));
        assert_eq!(screen.player_team_index, 1);
        assert!(screen.teams[1].player_controlled);
        assert!(!screen.teams[0].player_controlled);
    }

    #[test]
    fn team_count_can_be_adjusted() {
        let mut screen = TeamSetupScreen::new(2);
        screen.selected_row = 0; // Team count row
        assert!(matches!(screen.handle_input(InputEvent::Enter), TeamSetupOutcome::Changed));
        assert_eq!(screen.team_count, 3);
        assert_eq!(screen.teams.len(), 3);

        // Decrease with Left
        assert!(matches!(screen.handle_input(InputEvent::Left), TeamSetupOutcome::Changed));
        assert_eq!(screen.team_count, 2);
    }

    #[test]
    fn play_requires_human_team() {
        let mut screen = TeamSetupScreen::new(2);
        // Make all teams AI
        for team in screen.teams.iter_mut() {
            team.player_controlled = false;
        }
        screen.selected_row = screen.total_rows() - 2; // Play row
        let outcome = screen.handle_input(InputEvent::Enter);
        assert!(matches!(outcome, TeamSetupOutcome::Changed));
        assert!(screen.status.is_some());
    }
}
