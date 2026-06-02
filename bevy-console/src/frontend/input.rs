/// Discrete input events used by shared frontend state machines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputEvent {
    /// Confirm / primary action.
    Enter,
    /// Up direction.
    Up,
    /// Down direction.
    Down,
    /// Left direction.
    Left,
    /// Right direction.
    Right,
    /// Cursor up.
    CursorUp,
    /// Cursor down.
    CursorDown,
    /// Cursor left.
    CursorLeft,
    /// Cursor right.
    CursorRight,
    /// Cycle to the next hero.
    NextHero,
    /// End current player's turn.
    NextTurn,
    /// Pan viewport up (right stick / WASD).
    PanUp,
    /// Pan viewport down.
    PanDown,
    /// Pan viewport left.
    PanLeft,
    /// Pan viewport right.
    PanRight,
}
