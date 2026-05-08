//! Shared informational modal overlay.

use alloc::{string::String, vec::Vec};

use crate::input::InputEvent;

/// Shared informational overlay with a title and body lines.
pub struct InfoOverlay {
    /// Modal title.
    pub title: String,
    /// Body lines rendered under the title.
    pub lines: Vec<String>,
    /// Footer hint line.
    pub footer: String,
}

/// Result of applying one input event to the info overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InfoOverlayOutcome {
    /// Overlay remains open and unchanged.
    NoChange,
    /// Overlay should be closed.
    Close,
}

impl InfoOverlay {
    /// Creates a new informational overlay.
    ///
    /// # Arguments
    /// * `title` - Modal title text.
    /// * `lines` - Body lines shown in the modal.
    /// * `footer` - Footer hint line.
    pub fn new(title: String, lines: Vec<String>, footer: String) -> Self {
        Self {
            title,
            lines,
            footer,
        }
    }

    /// Applies one input event to the info overlay.
    ///
    /// `Enter`, `Back`, and `q` close the overlay.
    pub fn handle_input(&self, event: InputEvent) -> InfoOverlayOutcome {
        match event {
            InputEvent::Enter | InputEvent::Back => InfoOverlayOutcome::Close,
            InputEvent::Key(ch) if ch.eq_ignore_ascii_case(&'q') => InfoOverlayOutcome::Close,
            _ => InfoOverlayOutcome::NoChange,
        }
    }
}
