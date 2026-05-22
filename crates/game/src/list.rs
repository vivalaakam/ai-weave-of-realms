//! Shared selectable list screen state and input handling.

use crate::input::InputEvent;
use crate::types::ListEntry;
use alloc::string::String;
use alloc::vec::Vec;

/// Shared list browser model.
pub struct ListScreen {
    /// All available entries.
    pub entries: Vec<ListEntry>,
    /// Currently highlighted item index.
    pub selected: usize,
    /// First visible item index in the scroll window.
    pub scroll: usize,
    /// Optional footer status line.
    pub status: Option<String>,
}

/// Result of applying one list-screen input event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListOutcome {
    /// State did not change.
    NoChange,
    /// Selection or scroll changed.
    Changed,
    /// User confirmed the current item.
    Selected(usize),
    /// User requested leaving the list.
    BackRequested,
}

impl ListScreen {
    /// Creates a new list screen model.
    ///
    /// # Arguments
    /// * `entries` - Entries shown in the selector.
    /// * `status` - Initial optional status message.
    pub fn new(entries: Vec<ListEntry>, status: Option<String>) -> Self {
        Self { entries, selected: 0, scroll: 0, status }
    }

    /// Applies one input event to the shared list view.
    ///
    /// # Arguments
    /// * `event` - Platform-neutral input event.
    /// * `visible_rows` - Number of visible rows in the list viewport.
    pub fn handle_input(&mut self, event: InputEvent, visible_rows: usize) -> ListOutcome {
        if self.entries.is_empty() {
            return match list_event(event) {
                InputEvent::Enter => ListOutcome::Selected(0),
                _ => ListOutcome::NoChange,
            };
        }

        let mut changed = false;
        match list_event(event) {
            InputEvent::Up => {
                let previous = self.selected;
                self.selected = self.selected.saturating_sub(1);
                changed = self.selected != previous;
            }
            InputEvent::Down => {
                let previous = self.selected;
                if self.selected + 1 < self.entries.len() {
                    self.selected += 1;
                }
                changed = self.selected != previous;
            }
            InputEvent::Enter => return ListOutcome::Selected(self.selected),
            InputEvent::Back => return ListOutcome::BackRequested,
            InputEvent::None
            | InputEvent::Left
            | InputEvent::Right
            | InputEvent::Key(_)
            | InputEvent::NextHero
            | InputEvent::NextTurn
            | InputEvent::PanUp
            | InputEvent::PanDown
            | InputEvent::PanLeft
            | InputEvent::PanRight
            | InputEvent::CursorUp
            | InputEvent::CursorDown
            | InputEvent::CursorLeft
            | InputEvent::CursorRight => {}
        }

        let previous_scroll = self.scroll;
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + visible_rows {
            self.scroll = self.selected.saturating_sub(visible_rows.saturating_sub(1));
        }

        if changed || self.scroll != previous_scroll {
            ListOutcome::Changed
        } else {
            ListOutcome::NoChange
        }
    }
}

fn list_event(event: InputEvent) -> InputEvent {
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
