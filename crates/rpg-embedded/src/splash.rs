//! Shared splash-screen state and input handling.

use alloc::string::String;

use crate::input::InputEvent;

/// Shared splash screen model.
pub struct SplashScreen {
    /// Selected menu index.
    pub selected: usize,
    /// Optional status line shown at the bottom of the screen.
    pub status: Option<String>,
}

/// Result of applying one splash-screen input event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplashOutcome {
    /// State did not change.
    NoChange,
    /// Selection or status changed.
    Changed,
    /// User confirmed the current menu item.
    Selected(usize),
    /// User requested going back / exiting.
    BackRequested,
}

impl SplashScreen {
    /// Creates a new splash screen state.
    ///
    /// # Arguments
    /// * `selected` - Initial selected menu index.
    /// * `status` - Initial optional status line.
    pub fn new(selected: usize, status: Option<String>) -> Self {
        Self { selected, status }
    }

    /// Applies a single input event to the splash screen.
    ///
    /// # Arguments
    /// * `event` - Platform-neutral input event.
    /// * `options_len` - Number of available menu options.
    pub fn handle_input(&mut self, event: InputEvent, options_len: usize) -> SplashOutcome {
        let event = splash_event(event);
        match event {
            InputEvent::Up => {
                let previous = self.selected;
                self.selected = self.selected.saturating_sub(1);
                if self.selected != previous {
                    SplashOutcome::Changed
                } else {
                    SplashOutcome::NoChange
                }
            }
            InputEvent::Down => {
                let previous = self.selected;
                let last = options_len.saturating_sub(1);
                self.selected = (self.selected + 1).min(last);
                if self.selected != previous {
                    SplashOutcome::Changed
                } else {
                    SplashOutcome::NoChange
                }
            }
            InputEvent::Enter => SplashOutcome::Selected(self.selected),
            InputEvent::Back => SplashOutcome::BackRequested,
            InputEvent::None
            | InputEvent::Left
            | InputEvent::Right
            | InputEvent::Key(_)
            | InputEvent::Tab => SplashOutcome::NoChange,
        }
    }
}

fn splash_event(event: InputEvent) -> InputEvent {
    match event {
        InputEvent::Key(ch) => match ch.to_ascii_lowercase() {
            'w' | 'k' => InputEvent::Up,
            's' | 'j' => InputEvent::Down,
            'q' => InputEvent::Back,
            _ => InputEvent::None,
        },
        other => other,
    }
}
