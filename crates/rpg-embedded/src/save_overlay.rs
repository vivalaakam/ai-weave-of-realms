//! Shared save/load overlay state and input handling.

use alloc::string::{String, ToString};

use crate::input::InputEvent;
use crate::list::{ListOutcome, ListScreen};

/// Shared save/load overlay model.
pub enum SaveOverlay {
    /// Root save menu (save/load/cancel).
    Menu {
        /// Selected menu index.
        selected: usize,
        /// Optional status message.
        status: Option<String>,
    },
    /// Filename entry for saving.
    SaveName {
        /// Current filename input.
        name: String,
        /// Optional status message.
        status: Option<String>,
    },
    /// List of existing saves to load.
    LoadList {
        /// Shared selectable save list.
        list: ListScreen,
    },
}

/// Result of applying one input event to the save overlay.
pub enum SaveOverlayOutcome {
    /// Overlay state did not change.
    NoChange,
    /// Overlay state changed and should be redrawn.
    Changed,
    /// Overlay should be closed.
    Close,
    /// Host should discover saves and reopen the load list.
    RequestDiscoverSaves,
    /// Host should save the current game using this name.
    RequestSave(String),
    /// Host should load the selected save entry.
    RequestLoad(usize),
}

impl SaveOverlay {
    /// Creates the root save menu.
    pub fn menu() -> Self {
        Self::Menu {
            selected: 0,
            status: None,
        }
    }

    /// Creates a save-name entry screen.
    pub fn save_name() -> Self {
        Self::SaveName {
            name: String::new(),
            status: None,
        }
    }

    /// Creates a load-list screen from a shared list.
    ///
    /// # Arguments
    /// * `list` - Shared list model containing discovered saves.
    pub fn load_list(list: ListScreen) -> Self {
        Self::LoadList { list }
    }

    /// Applies one input event to the save overlay.
    ///
    /// # Arguments
    /// * `event` - Platform-neutral input event.
    /// * `visible_rows` - Number of visible rows in the load-list viewport.
    pub fn handle_input(&mut self, event: InputEvent, visible_rows: usize) -> SaveOverlayOutcome {
        match self {
            SaveOverlay::Menu { selected, status } => match menu_event(event) {
                InputEvent::Up => {
                    *selected = selected.saturating_sub(1);
                    SaveOverlayOutcome::Changed
                }
                InputEvent::Down => {
                    *selected = (*selected + 1).min(2);
                    SaveOverlayOutcome::Changed
                }
                InputEvent::Enter => match *selected {
                    0 => {
                        *self = SaveOverlay::save_name();
                        SaveOverlayOutcome::Changed
                    }
                    1 => SaveOverlayOutcome::RequestDiscoverSaves,
                    _ => SaveOverlayOutcome::Close,
                },
                InputEvent::Back => SaveOverlayOutcome::Close,
                _ => {
                    let _ = status;
                    SaveOverlayOutcome::NoChange
                }
            },
            SaveOverlay::SaveName { name, status } => {
                const MAX_NAME_LEN: usize = 24;
                match event {
                    InputEvent::Key(ch) => {
                        if name.len() < MAX_NAME_LEN {
                            if let Some(mapped) = normalize_save_char(ch) {
                                name.push(mapped);
                                *status = None;
                            } else {
                                *status = Some("Allowed: A-Z 0-9 _ - space".to_string());
                            }
                        } else {
                            *status = Some("Name is too long".to_string());
                        }
                        SaveOverlayOutcome::Changed
                    }
                    InputEvent::Back => {
                        if name.pop().is_some() {
                            SaveOverlayOutcome::Changed
                        } else {
                            *self = SaveOverlay::menu();
                            SaveOverlayOutcome::Changed
                        }
                    }
                    InputEvent::Enter => {
                        let trimmed = name.trim();
                        if trimmed.is_empty() {
                            *status = Some("Enter a save name".to_string());
                            SaveOverlayOutcome::Changed
                        } else {
                            SaveOverlayOutcome::RequestSave(trimmed.to_string())
                        }
                    }
                    _ => SaveOverlayOutcome::NoChange,
                }
            }
            SaveOverlay::LoadList { list } => match list.handle_input(event, visible_rows) {
                ListOutcome::NoChange => SaveOverlayOutcome::NoChange,
                ListOutcome::Changed => SaveOverlayOutcome::Changed,
                ListOutcome::BackRequested => {
                    *self = SaveOverlay::Menu {
                        selected: 1,
                        status: None,
                    };
                    SaveOverlayOutcome::Changed
                }
                ListOutcome::Selected(selected) => SaveOverlayOutcome::RequestLoad(selected),
            },
        }
    }

    /// Sets a status line on the active overlay state when supported.
    ///
    /// # Arguments
    /// * `status` - New optional status message.
    pub fn set_status(&mut self, status: Option<String>) {
        match self {
            SaveOverlay::Menu { status: slot, .. } => *slot = status,
            SaveOverlay::SaveName { status: slot, .. } => *slot = status,
            SaveOverlay::LoadList { list } => list.status = status,
        }
    }
}

fn menu_event(event: InputEvent) -> InputEvent {
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

fn normalize_save_char(ch: char) -> Option<char> {
    if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ' ' {
        Some(ch)
    } else {
        None
    }
}
