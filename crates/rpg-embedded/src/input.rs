//! Shared input events for embedded frontends.

/// Discrete input events used by shared frontend state machines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputEvent {
    /// No input this frame.
    None,
    /// Keyboard character input.
    Key(char),
    /// Confirm / primary action.
    Enter,
    /// Back / cancel.
    Back,
    /// Up direction.
    Up,
    /// Down direction.
    Down,
    /// Left direction.
    Left,
    /// Right direction.
    Right,
    /// Cycle to the next hero.
    Tab,
}
