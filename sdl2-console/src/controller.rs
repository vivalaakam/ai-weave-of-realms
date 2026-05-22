use sdl2::controller::{Axis, Button, GameController};
use sdl2::event::Event;
use tracing::{info, warn};

use game::input::InputEvent;

use crate::sdl_host::SdlHost;

/// Controller axis dead-zone / trigger thresholds.
const CURSOR_DEAD_ZONE: i16 = 24_000;
const PAN_DEAD_ZONE: i16 = 24_000;
const TRIGGER_THRESHOLD: i16 = 16_000;

/// Open all currently attached game controllers.
///
/// # Arguments
/// * `subsystem` — The SDL2 game-controller subsystem.
///
/// # Returns
/// A vector of successfully opened controllers.
pub fn open_controllers(subsystem: &sdl2::GameControllerSubsystem) -> Vec<GameController> {
    let count = subsystem.num_joysticks().unwrap_or(0);
    let mut out = Vec::new();
    for i in 0..count {
        if subsystem.is_game_controller(i) {
            match subsystem.open(i) {
                Ok(c) => {
                    info!(name = c.name(), "controller connected");
                    out.push(c);
                }
                Err(e) => warn!("failed to open controller {i}: {e}"),
            }
        }
    }
    out
}

/// Process a batch of SDL2 events for controller add/remove/input.
///
/// Mutates `controllers`, `host` and `app_state.needs_redraw`.
///
/// # Arguments
/// * `event`               — Single SDL2 event.
/// * `game_controller`     — SDL2 controller subsystem (for opening new controllers).
/// * `controllers`         — Currently open controller handles.
/// * `host`                — Mutable app host state (for axis tracking booleans).
/// * `layout`              — Current app layout for `handle_input`.
/// * `needs_redraw`        — Whether a UI redraw was triggered this frame.
pub fn handle_controller_event(
    event: &Event,
    game_controller: &sdl2::GameControllerSubsystem,
    controllers: &mut Vec<GameController>,
    host: &mut SdlHost,
    layout: &game::app::AppLayout,
    app_state: &mut game::app::EmbeddedApp,
    needs_redraw: &mut bool,
) {
    match event {
        Event::ControllerDeviceAdded { which, .. }
            if game_controller.is_game_controller(*which) =>
        {
            match game_controller.open(*which) {
                Ok(c) => {
                    info!(name = c.name(), "controller connected");
                    controllers.push(c);
                }
                Err(e) => warn!("failed to open controller {which}: {e}"),
            }
        }
        Event::ControllerDeviceRemoved { which, .. } => {
            controllers.retain(|c| c.instance_id() != *which);
            info!(id = which, "controller disconnected");
        }
        Event::ControllerButtonDown { button, .. } => {
            let input = map_controller_button(*button);
            if app_state.handle_input(host, input, *layout) {
                *needs_redraw = true;
            }
        }
        Event::ControllerAxisMotion { axis, value, .. } => {
            let mut generated = Vec::new();
            match *axis {
                Axis::LeftX => {
                    if *value > CURSOR_DEAD_ZONE {
                        if !host.left_x_right {
                            host.left_x_right = true;
                            generated.push(InputEvent::CursorRight);
                        }
                    } else {
                        host.left_x_right = false;
                    }
                    if *value < -CURSOR_DEAD_ZONE {
                        if !host.left_x_left {
                            host.left_x_left = true;
                            generated.push(InputEvent::CursorLeft);
                        }
                    } else {
                        host.left_x_left = false;
                    }
                }
                Axis::LeftY => {
                    if *value > CURSOR_DEAD_ZONE {
                        if !host.left_y_down {
                            host.left_y_down = true;
                            generated.push(InputEvent::CursorDown);
                        }
                    } else {
                        host.left_y_down = false;
                    }
                    if *value < -CURSOR_DEAD_ZONE {
                        if !host.left_y_up {
                            host.left_y_up = true;
                            generated.push(InputEvent::CursorUp);
                        }
                    } else {
                        host.left_y_up = false;
                    }
                }
                Axis::RightX => {
                    if *value > PAN_DEAD_ZONE {
                        if !host.right_x_right {
                            host.right_x_right = true;
                            generated.push(InputEvent::PanRight);
                        }
                    } else {
                        host.right_x_right = false;
                    }
                    if *value < -PAN_DEAD_ZONE {
                        if !host.right_x_left {
                            host.right_x_left = true;
                            generated.push(InputEvent::PanLeft);
                        }
                    } else {
                        host.right_x_left = false;
                    }
                }
                Axis::RightY => {
                    if *value > PAN_DEAD_ZONE {
                        if !host.right_y_down {
                            host.right_y_down = true;
                            generated.push(InputEvent::PanDown);
                        }
                    } else {
                        host.right_y_down = false;
                    }
                    if *value < -PAN_DEAD_ZONE {
                        if !host.right_y_up {
                            host.right_y_up = true;
                            generated.push(InputEvent::PanUp);
                        }
                    } else {
                        host.right_y_up = false;
                    }
                }
                Axis::TriggerRight => {
                    if *value > TRIGGER_THRESHOLD {
                        if !host.trigger_r_active {
                            host.trigger_r_active = true;
                            generated.push(InputEvent::NextHero);
                        }
                    } else {
                        host.trigger_r_active = false;
                    }
                }
                _ => {}
            }
            for input in generated {
                if app_state.handle_input(host, input, *layout) {
                    *needs_redraw = true;
                }
            }
        }
        // Window events and quit are NOT controller events; they must be
        // handled by the caller (`main.rs`) so we silently ignore them here.
        _ => {}
    }
}

/// Map an SDL2 controller button to a game `InputEvent`.
pub fn map_controller_button(button: Button) -> InputEvent {
    match button {
        Button::DPadUp => InputEvent::Up,
        Button::DPadDown => InputEvent::Down,
        Button::DPadLeft => InputEvent::Left,
        Button::DPadRight => InputEvent::Right,
        Button::A | Button::Start => InputEvent::Enter,
        Button::Y => InputEvent::NextTurn,
        Button::B | Button::Back => InputEvent::Back,
        _ => InputEvent::None,
    }
}
