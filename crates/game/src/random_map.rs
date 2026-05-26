//! Random-map seed generator and selection screen.
//!
//! Two-word seed phrases are drawn from a 30×30 vocabulary and fed into
//! the procedural map generator.

use alloc::string::{String, ToString};
use core::time::Duration;

use crate::input::InputEvent;

// ─── Vocabulary ───────────────────────────────────────────────────────────────

/// First half of the seed phrase (adjectives / atmosphere).
const FIRST_WORDS: [&str; 30] = [
    "ancient", "burning", "crimson", "dark", "ember", "frozen", "gentle", "hidden", "iron", "jade",
    "kindred", "lost", "misty", "noble", "obsidian", "pale", "quiet", "radiant", "savage", "titan",
    "undying", "velvet", "wild", "xeric", "yellow", "zealous", "bitter", "calm", "dread", "elder",
];

/// Second half of the seed phrase (nouns / places).
const SECOND_WORDS: [&str; 30] = [
    "forests", "citadel", "valley", "wastes", "isles", "peaks", "marsh", "steppes", "caverns",
    "plains", "ravine", "tundra", "jungle", "desert", "shores", "meadows", "dungeon", "harbor",
    "towers", "grove", "thicket", "basin", "fjords", "oasis", "ruins", "sanctum", "village",
    "passage", "waters", "fields",
];

/// Generates a random two-word seed phrase.
pub fn generate_seed_phrase() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    let hash = now.as_nanos() as usize;
    let first = FIRST_WORDS[hash % FIRST_WORDS.len()];
    let second = SECOND_WORDS[(hash / FIRST_WORDS.len()) % SECOND_WORDS.len()];
    format!("{first} {second}")
}

// ─── RandomMapScreen ──────────────────────────────────────────────────────────

/// Screen model for the random-map generator.
pub struct RandomMapScreen {
    /// Currently selected option index.
    pub selected: usize,
    /// Generated seed phrase (None until first Random press).
    pub seed_phrase: Option<String>,
    /// Optional status line.
    pub status: Option<String>,
}

impl RandomMapScreen {
    /// Creates a new empty random-map screen.
    pub fn new() -> Self {
        Self { selected: 0, seed_phrase: None, status: None }
    }
}

/// Result of applying one input event to the random-map screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RandomMapOutcome {
    /// Nothing changed.
    NoChange,
    /// Selection or seed changed.
    Changed,
    /// User pressed "Play" — the seed is ready to generate a map.
    PlayRequested { seed: String },
    /// User pressed "Back".
    BackRequested,
}

impl RandomMapScreen {
    /// Menu option labels visible to the renderer.
    pub const OPTIONS: [&str; 3] = ["Random", "Play", "Back"];

    /// Applies a single input event.
    pub fn handle_input(&mut self, event: InputEvent) -> RandomMapOutcome {
        match event {
            InputEvent::Up => {
                let prev = self.selected;
                self.selected = self.selected.saturating_sub(1);
                if self.selected != prev {
                    RandomMapOutcome::Changed
                } else {
                    RandomMapOutcome::NoChange
                }
            }
            InputEvent::Down => {
                let prev = self.selected;
                let last = Self::OPTIONS.len().saturating_sub(1);
                self.selected = (self.selected + 1).min(last);
                if self.selected != prev {
                    RandomMapOutcome::Changed
                } else {
                    RandomMapOutcome::NoChange
                }
            }
            InputEvent::Enter => match self.selected {
                0 => {
                    self.seed_phrase = Some(generate_seed_phrase());
                    RandomMapOutcome::Changed
                }
                1 => {
                    if let Some(ref seed) = self.seed_phrase {
                        RandomMapOutcome::PlayRequested { seed: seed.clone() }
                    } else {
                        self.status = Some("Press Random first".to_string());
                        RandomMapOutcome::Changed
                    }
                }
                _ => RandomMapOutcome::BackRequested,
            },
            InputEvent::Back => RandomMapOutcome::BackRequested,
            _ => RandomMapOutcome::NoChange,
        }
    }

    /// Returns the currently selected option label.
    pub fn selected_label(&self) -> &'static str {
        Self::OPTIONS[self.selected.min(Self::OPTIONS.len() - 1)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_seed_returns_non_empty() {
        let seed = generate_seed_phrase();
        assert!(!seed.is_empty());
        assert!(seed.contains(' '));
    }

    #[test]
    fn random_map_navigates() {
        let mut screen = RandomMapScreen::new();
        assert_eq!(screen.selected, 0);
        assert!(matches!(screen.handle_input(InputEvent::Down), RandomMapOutcome::Changed));
        assert_eq!(screen.selected, 1);
        assert!(matches!(screen.handle_input(InputEvent::Up), RandomMapOutcome::Changed));
        assert_eq!(screen.selected, 0);
        assert!(matches!(screen.handle_input(InputEvent::Enter), RandomMapOutcome::Changed));
        assert!(screen.seed_phrase.is_some());
        assert!(matches!(screen.handle_input(InputEvent::Down), RandomMapOutcome::Changed));
        assert_eq!(screen.selected, 1);
        assert!(matches!(
            screen.handle_input(InputEvent::Enter),
            RandomMapOutcome::PlayRequested { .. }
        ));
    }

    #[test]
    fn play_without_seed_shows_error() {
        let mut screen = RandomMapScreen::new();
        screen.selected = 1;
        assert!(matches!(screen.handle_input(InputEvent::Enter), RandomMapOutcome::Changed));
        assert_eq!(screen.status, Some("Press Random first".to_string()));
    }

    #[test]
    fn back_button_works() {
        let mut screen = RandomMapScreen::new();
        screen.selected = 2;
        assert!(matches!(screen.handle_input(InputEvent::Enter), RandomMapOutcome::BackRequested));
    }
}
