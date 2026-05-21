//! End-of-turn confirmation overlay for embedded frontends.

use crate::input::InputEvent;

/// End-of-turn confirmation overlay with Yes / Cancel buttons.
pub struct EndTurnOverlay {
    /// Selected button index: 0 = Yes, 1 = Cancel.
    pub selected: usize,
}

impl Default for EndTurnOverlay {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of applying one input event to the end-turn overlay.
pub enum EndTurnOverlayOutcome {
    /// Overlay state did not change.
    NoChange,
    /// Selection moved.
    Changed,
    /// Overlay should be closed without ending turn.
    Close,
    /// Player confirmed — end the turn.
    ConfirmEndTurn,
}

impl EndTurnOverlay {
    /// Creates a new end-turn overlay.
    pub fn new() -> Self {
        Self { selected: 0 }
    }

    /// Applies one input event to the overlay.
    pub fn handle_input(&mut self, event: InputEvent) -> EndTurnOverlayOutcome {
        match event {
            InputEvent::Up | InputEvent::Left => {
                if self.selected > 0 {
                    self.selected = self.selected.saturating_sub(1);
                    EndTurnOverlayOutcome::Changed
                } else {
                    EndTurnOverlayOutcome::NoChange
                }
            }
            InputEvent::Down | InputEvent::Right => {
                if self.selected < 1 {
                    self.selected = 1;
                    EndTurnOverlayOutcome::Changed
                } else {
                    EndTurnOverlayOutcome::NoChange
                }
            }
            InputEvent::Enter => {
                if self.selected == 0 {
                    EndTurnOverlayOutcome::ConfirmEndTurn
                } else {
                    EndTurnOverlayOutcome::Close
                }
            }
            InputEvent::Back => EndTurnOverlayOutcome::Close,
            _ => EndTurnOverlayOutcome::NoChange,
        }
    }
}
