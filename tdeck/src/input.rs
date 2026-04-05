//! Keyboard and trackball input polling.

use esp_hal::gpio::Input;
use esp_hal::i2c::master::I2c;

const KEYBOARD_I2C_ADDRESS: u8 = 0x55;

/// Discrete input events used by the app state machine.
#[derive(Clone, Copy)]
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
}

/// Edge-trigger bookkeeping for trackball buttons.
pub struct InputState {
    last_click_high: bool,
    last_up_high: bool,
    last_down_high: bool,
    last_left_high: bool,
    last_right_high: bool,
}

impl InputState {
    /// Creates a new input state snapshot.
    pub fn new(
        click_high: bool,
        up_high: bool,
        down_high: bool,
        left_high: bool,
        right_high: bool,
    ) -> Self {
        Self {
            last_click_high: click_high,
            last_up_high: up_high,
            last_down_high: down_high,
            last_left_high: left_high,
            last_right_high: right_high,
        }
    }
}

/// Polls keyboard and trackball hardware and returns a single app event.
pub fn poll_input(
    keyboard: &mut I2c<'_, esp_hal::Blocking>,
    trackball_click: &Input<'_>,
    trackball_up: &Input<'_>,
    trackball_down: &Input<'_>,
    trackball_left: &Input<'_>,
    trackball_right: &Input<'_>,
    input_state: &mut InputState,
) -> InputEvent {
    let mut key_data: [u8; 1] = [0; 1];
    if keyboard.read(KEYBOARD_I2C_ADDRESS, &mut key_data).is_ok() {
        let key = key_data[0];
        let event = match key {
            b'\r' | b'\n' => InputEvent::Enter,
            0x08 | 0x1B | 0x7F => InputEvent::Back,
            0x20..=0x7E => InputEvent::Key(key as char),
            _ => InputEvent::None,
        };

        if !matches!(event, InputEvent::None) {
            return event;
        }
    }

    let click_high = trackball_click.is_high();
    if click_high != input_state.last_click_high {
        input_state.last_click_high = click_high;
        if !click_high {
            return InputEvent::Enter;
        }
    }

    let up_high = trackball_up.is_high();
    if up_high != input_state.last_up_high {
        input_state.last_up_high = up_high;
        if !up_high {
            return InputEvent::Up;
        }
    }

    let down_high = trackball_down.is_high();
    if down_high != input_state.last_down_high {
        input_state.last_down_high = down_high;
        if !down_high {
            return InputEvent::Down;
        }
    }

    let left_high = trackball_left.is_high();
    if left_high != input_state.last_left_high {
        input_state.last_left_high = left_high;
        if !left_high {
            return InputEvent::Left;
        }
    }

    let right_high = trackball_right.is_high();
    if right_high != input_state.last_right_high {
        input_state.last_right_high = right_high;
        if !right_high {
            return InputEvent::Right;
        }
    }

    InputEvent::None
}
