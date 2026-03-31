//! `ScoreUI` GDExtension node — a `Label` that auto-updates when the score changes.
//!
//! ## Usage (GDScript)
//! ```gdscript
//! func _ready():
//!     var gm: GameManager = get_node("/root/GameManager")
//!     gm.score_changed.connect($ScoreUI.on_score_changed)
//! ```

use godot::classes::{ILabel, Label};
use godot::prelude::*;

// ─── ScoreUI ──────────────────────────────────────────────────────────────────

#[derive(GodotClass)]
#[class(base=Label)]
pub struct ScoreUI {
    base: Base<Label>,

    /// Format string; `{score}` is replaced with the current total.
    #[var]
    pub format: GString,
}

#[godot_api]
impl ILabel for ScoreUI {
    fn init(base: Base<Label>) -> Self {
        Self {
            base,
            format: GString::from("Score: {score}"),
        }
    }

    fn ready(&mut self) {
        // Show "Score: 0" immediately
        self.on_score_changed(0);
    }
}

#[godot_api]
impl ScoreUI {
    /// Connected to `GameManager.score_changed`.
    ///
    /// Updates the label text using the configured `format` string.
    #[func]
    pub fn on_score_changed(&mut self, score: i64) {
        let text = self.format.to_string().replace("{score}", &score.to_string());
        self.base_mut().set_text(&text);
    }

    /// Returns the currently displayed score value parsed from the label text.
    ///
    /// Returns `0` if parsing fails.
    #[func]
    pub fn get_displayed_score(&self) -> i64 {
        let text = self.base().get_text().to_string();
        // Extract the numeric suffix after the last space
        text.split_whitespace()
            .last()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    }
}
