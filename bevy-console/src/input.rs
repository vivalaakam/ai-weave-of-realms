//! Unified input mapping: keyboard + gamepad → UiAction.
//!
//! Reads `assets/keybindings.toml` for configurable bindings.
//! Keyboard uses `just_pressed()`. Gamepad uses a debounce system:
//! a button must be held for `DEBOUNCE_FRAMES` consecutive frames before
//! emitting a UiAction. This filters out phantom presses from macOS
//! virtual gamepads with broken gilrs default mappings.
//!
//! ## Gamepad selection
//!
//! On devices with multiple gamepad slots (e.g. GPD Win 4 with its built-in
//! controller appearing on a non-zero slot), iterating all `Gamepad`
//! entities and overwriting stick values would clobber real input with zeroes
//! from an empty slot. We now pick the **first connected gamepad** that
//! reports non-zero input and use it exclusively, avoiding the overwrite bug.
use crate::screens;
use bevy::input::InputSystems;
use bevy::input::gamepad::{GamepadConnection, GamepadConnectionEvent, RawGamepadAxisChangedEvent};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::CursorOptions;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── UiAction ─────────────────────────────────────────────────────────

/// High-level UI/game action emitted by the input system each frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Message)]
pub enum UiAction {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Cancel,
    NextHero,
    NextTurn,
    CursorUp,
    CursorDown,
    CursorLeft,
    CursorRight,
    PanUp,
    PanDown,
    PanLeft,
    PanRight,
}

// ── Gamepad debounce ─────────────────────────────────────────────────

/// How many consecutive frames a gamepad button must be held before
/// we recognise it as a real press. Filters out phantom one-frame
/// spikes from macOS virtual gamepads with broken gilrs mappings.
const DEBOUNCE_FRAMES: u32 = 3;

/// Frames a D-pad / stick axis must stay past the deadzone before
/// we emit a directional UiAction.
const AXIS_DEBOUNCE_FRAMES: u32 = 3;

/// Tracks per-button hold duration to debounce phantom presses.
#[derive(Resource, Default)]
struct GamepadDebounce {
    /// Button → consecutive frames it has been held.
    button_held: HashMap<GamepadButton, u32>,
    /// Whether the button has already been "fired" this hold cycle.
    button_fired: HashMap<GamepadButton, bool>,
    /// Axis direction → consecutive frames past deadzone.
    axis_held: HashMap<(GamepadAxis, Sign), u32>,
    /// Whether the axis direction has already been "fired" this cycle.
    axis_fired: HashMap<(GamepadAxis, Sign), bool>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Sign {
    Positive,
    Negative,
}

/// Cooldown after state transition. Prevents stale Confirm/Cancel
/// from bleeding into the next screen.
#[derive(Resource, Default)]
pub struct InputCooldown {
    /// Time (seconds) when cooldown started. 0.0 = no cooldown.
    pub start: f64,
    /// How many seconds to ignore input after transition.
    pub duration: f64,
}

impl InputCooldown {
    pub fn trigger(now: f64, duration: f64) -> Self {
        Self { start: now, duration }
    }

    pub fn is_cooling_down(&self, now: f64) -> bool {
        self.duration > 0.0 && (now - self.start) < self.duration
    }
}

// ── TOML config ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeybindingsConfig {
    pub keyboard: KeyboardBindings,
    pub gamepad: GamepadBindings,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeyboardBindings {
    pub up: Vec<String>,
    pub down: Vec<String>,
    pub left: Vec<String>,
    pub right: Vec<String>,
    pub confirm: Vec<String>,
    pub cancel: Vec<String>,
    pub next_hero: Vec<String>,
    pub next_turn: Vec<String>,
    #[serde(default)]
    pub cursor_up: Vec<String>,
    #[serde(default)]
    pub cursor_down: Vec<String>,
    #[serde(default)]
    pub cursor_left: Vec<String>,
    #[serde(default)]
    pub cursor_right: Vec<String>,
    #[serde(default)]
    pub pan_up: Vec<String>,
    #[serde(default)]
    pub pan_down: Vec<String>,
    #[serde(default)]
    pub pan_left: Vec<String>,
    #[serde(default)]
    pub pan_right: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GamepadBindings {
    #[serde(default)]
    pub enabled: bool,
    pub up: Vec<String>,
    pub down: Vec<String>,
    pub left: Vec<String>,
    pub right: Vec<String>,
    pub confirm: Vec<String>,
    pub cancel: Vec<String>,
    pub next_hero: Vec<String>,
    pub next_turn: Vec<String>,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        toml::from_str(include_str!("../../assets/keybindings.toml"))
            .expect("default keybindings.toml should parse")
    }
}

// ── Compiled mapping (Resource) ─────────────────────────────────────

#[derive(Resource)]
pub struct InputMapping {
    keyboard_map: HashMap<KeyCode, Vec<UiAction>>,
    gamepad_map: HashMap<GamepadButton, Vec<UiAction>>,
    gamepad_enabled: bool,
    right_stick_pan: bool,
}

impl InputMapping {
    pub fn from_config(config: &KeybindingsConfig) -> Self {
        let mut kb_map: HashMap<KeyCode, Vec<UiAction>> = HashMap::new();

        let kb_pairs: [(&Vec<String>, UiAction); 16] = [
            (&config.keyboard.up, UiAction::Up),
            (&config.keyboard.down, UiAction::Down),
            (&config.keyboard.left, UiAction::Left),
            (&config.keyboard.right, UiAction::Right),
            (&config.keyboard.confirm, UiAction::Confirm),
            (&config.keyboard.cancel, UiAction::Cancel),
            (&config.keyboard.next_hero, UiAction::NextHero),
            (&config.keyboard.next_turn, UiAction::NextTurn),
            (&config.keyboard.cursor_up, UiAction::CursorUp),
            (&config.keyboard.cursor_down, UiAction::CursorDown),
            (&config.keyboard.cursor_left, UiAction::CursorLeft),
            (&config.keyboard.cursor_right, UiAction::CursorRight),
            (&config.keyboard.pan_up, UiAction::PanUp),
            (&config.keyboard.pan_down, UiAction::PanDown),
            (&config.keyboard.pan_left, UiAction::PanLeft),
            (&config.keyboard.pan_right, UiAction::PanRight),
        ];

        for (keys, action) in kb_pairs {
            for key_str in keys {
                if let Some(kc) = parse_keycode(key_str) {
                    kb_map.entry(kc).or_default().push(action);
                }
            }
        }

        let mut gp_map: HashMap<GamepadButton, Vec<UiAction>> = HashMap::new();
        let gp_pairs: [(&Vec<String>, UiAction); 8] = [
            (&config.gamepad.up, UiAction::Up),
            (&config.gamepad.down, UiAction::Down),
            (&config.gamepad.left, UiAction::Left),
            (&config.gamepad.right, UiAction::Right),
            (&config.gamepad.confirm, UiAction::Confirm),
            (&config.gamepad.cancel, UiAction::Cancel),
            (&config.gamepad.next_hero, UiAction::NextHero),
            (&config.gamepad.next_turn, UiAction::NextTurn),
        ];

        for (buttons, action) in gp_pairs {
            for btn_str in buttons {
                if let Some(gb) = parse_gamepad_button(btn_str) {
                    gp_map.entry(gb).or_default().push(action);
                }
            }
        }

        Self {
            keyboard_map: kb_map,
            gamepad_map: gp_map,
            gamepad_enabled: config.gamepad.enabled,
            right_stick_pan: config.gamepad.enabled,
        }
    }

    /// Returns the list of mapped gamepad buttons as a Vec for passing
    /// to `pick_active_gamepad`.
    fn gamepad_map_keys(&self) -> Vec<GamepadButton> {
        self.gamepad_map.keys().copied().collect()
    }
}

// ── Parsing helpers ──────────────────────────────────────────────────

fn parse_keycode(s: &str) -> Option<KeyCode> {
    use KeyCode::*;
    Some(match s {
        "ArrowUp" => ArrowUp,
        "ArrowDown" => ArrowDown,
        "ArrowLeft" => ArrowLeft,
        "ArrowRight" => ArrowRight,
        "Enter" => Enter,
        "Escape" => Escape,
        "Space" => Space,
        "Backspace" => Backspace,
        "Tab" => Tab,
        "KeyA" => KeyA,
        "KeyB" => KeyB,
        "KeyC" => KeyC,
        "KeyD" => KeyD,
        "KeyE" => KeyE,
        "KeyF" => KeyF,
        "KeyG" => KeyG,
        "KeyH" => KeyH,
        "KeyI" => KeyI,
        "KeyJ" => KeyJ,
        "KeyK" => KeyK,
        "KeyL" => KeyL,
        "KeyM" => KeyM,
        "KeyN" => KeyN,
        "KeyO" => KeyO,
        "KeyP" => KeyP,
        "KeyQ" => KeyQ,
        "KeyR" => KeyR,
        "KeyS" => KeyS,
        "KeyT" => KeyT,
        "KeyU" => KeyU,
        "KeyV" => KeyV,
        "KeyW" => KeyW,
        "KeyX" => KeyX,
        "KeyY" => KeyY,
        "KeyZ" => KeyZ,
        "Numpad0" => Numpad0,
        "Numpad1" => Numpad1,
        "Numpad2" => Numpad2,
        "Numpad3" => Numpad3,
        "Numpad4" => Numpad4,
        "Numpad5" => Numpad5,
        "Numpad6" => Numpad6,
        "Numpad7" => Numpad7,
        "Numpad8" => Numpad8,
        "Numpad9" => Numpad9,
        "NumpadEnter" => NumpadEnter,
        "NumpadAdd" => NumpadAdd,
        "NumpadSubtract" => NumpadSubtract,
        "F1" => F1,
        "F2" => F2,
        "F3" => F3,
        "F4" => F4,
        "F5" => F5,
        "F6" => F6,
        "F7" => F7,
        "F8" => F8,
        "F9" => F9,
        "F10" => F10,
        "F11" => F11,
        "F12" => F12,
        _ => {
            tracing::warn!("unknown KeyCode in keybindings.toml: {s}");
            return None;
        }
    })
}

fn parse_gamepad_button(s: &str) -> Option<GamepadButton> {
    use GamepadButton::*;
    Some(match s {
        "South" => South,
        "East" => East,
        "North" => North,
        "West" => West,
        "Select" => Select,
        "Start" => Start,
        "Mode" => Mode,
        "LeftTrigger" => LeftTrigger,
        "LeftTrigger2" => LeftTrigger2,
        "RightTrigger" => RightTrigger,
        "RightTrigger2" => RightTrigger2,
        "LeftThumb" => LeftThumb,
        "RightThumb" => RightThumb,
        "DPadUp" => DPadUp,
        "DPadDown" => DPadDown,
        "DPadLeft" => DPadLeft,
        "DPadRight" => DPadRight,
        _ => {
            tracing::warn!("unknown GamepadButton in keybindings.toml: {s}");
            return None;
        }
    })
}

// ── Plugin ───────────────────────────────────────────────────────────

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        let config = match std::fs::read_to_string("assets/keybindings.toml") {
            Ok(toml_str) => toml::from_str::<KeybindingsConfig>(&toml_str).unwrap_or_else(|e| {
                tracing::warn!("keybindings.toml parse error ({e}), using defaults");
                KeybindingsConfig::default()
            }),
            Err(_) => {
                tracing::info!("no keybindings.toml found, using compiled-in defaults");
                KeybindingsConfig::default()
            }
        };

        let mapping = InputMapping::from_config(&config);
        app.insert_resource(mapping)
            .init_resource::<GamepadDebounce>()
            .init_resource::<InputCooldown>()
            .init_resource::<CursorVisibility>()
            .add_message::<UiAction>()
            .add_systems(PreUpdate, collect_ui_actions)
            .add_systems(PreUpdate, log_gamepad_events.after(InputSystems))
            .add_systems(PreUpdate, update_cursor_visibility.after(collect_ui_actions))
            .add_systems(OnEnter(screens::AppState::Splash), trigger_input_cooldown)
            .add_systems(OnEnter(screens::AppState::MapSelect), trigger_input_cooldown)
            .add_systems(OnEnter(screens::AppState::SaveSelect), trigger_input_cooldown)
            .add_systems(OnEnter(screens::AppState::RandomMap), trigger_input_cooldown)
            .add_systems(OnEnter(screens::AppState::TeamSetup), trigger_input_cooldown)
            .add_systems(OnEnter(screens::AppState::MapView), trigger_input_cooldown)
            .add_systems(OnEnter(screens::AppState::City), trigger_input_cooldown);
    }
}

/// Pick the best gamepad from a query.
///
/// On devices with multiple gamepad slots (e.g. GPD Win 4 — built-in
/// controller + possible external gamepads, or desktops with virtual gamepads),
/// iterating ALL of them and overwriting stick values on each pass would
/// clobber real stick input with zeroes from empty/disconnected slots.
///
/// This function picks the gamepad that reports any activity (pressed
/// buttons, non-zero stick axes, D-pad), preferring it over silent gamepads.
/// If no gamepad is active, falls back to the first one (covers idle state).
fn pick_active_gamepad(
    gamepads: &Query<&Gamepad>,
    mapped_buttons: &[GamepadButton],
) -> Option<GamepadValues> {
    let mut first_values: Option<GamepadValues> = None;
    let mut active_values: Option<GamepadValues> = None;

    for gamepad in gamepads {
        let ls = gamepad.left_stick();
        let rs = gamepad.right_stick();
        let dpad = gamepad.dpad();

        // Snapshot pressed state for mapped buttons.
        let mut pressed_buttons: Vec<GamepadButton> = Vec::new();
        for &gb in mapped_buttons {
            if gamepad.pressed(gb) {
                pressed_buttons.push(gb);
            }
        }

        // Check if this gamepad shows any activity.
        let any_button = !pressed_buttons.is_empty();
        let threshold = 0.04; // Below Bevy's default axis deadzone (0.05)
        let any_stick = ls.x.abs() > threshold
            || ls.y.abs() > threshold
            || rs.x.abs() > threshold
            || rs.y.abs() > threshold;
        let any_dpad = dpad.x.abs() > 0.0 || dpad.y.abs() > 0.0;

        // Remember the first gamepad as fallback.
        if first_values.is_none() {
            first_values = Some(GamepadValues { pressed_buttons: pressed_buttons.clone(), ls, rs });
        }

        if any_button || any_stick || any_dpad {
            // Use the first active gamepad we find.
            if active_values.is_none() {
                active_values = Some(GamepadValues { pressed_buttons, ls, rs });
            }
        }
    }

    // Prefer an active gamepad; fall back to the first one if idle.
    active_values.or(first_values)
}

/// Snapshot of gamepad values extracted from a single `Gamepad` component.
/// Avoids holding a reference into the query that causes lifetime issues.
struct GamepadValues {
    pressed_buttons: Vec<GamepadButton>,
    ls: Vec2,
    rs: Vec2,
}

/// Read keyboard state + debounced gamepad state and emit `UiAction` messages.
fn collect_ui_actions(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    gamepads: Query<&Gamepad>,
    mapping: Res<InputMapping>,
    mut debounce: ResMut<GamepadDebounce>,
    cooldown: ResMut<InputCooldown>,
    exit_state: Res<crate::screens::exit_confirm::ExitConfirmState>,
    mut writer: MessageWriter<UiAction>,
) {
    // ── Cooldown check ────────────────────────────────────────
    let now = time.elapsed_secs_f64();
    if cooldown.is_cooling_down(now) {
        return;
    }

    // When the exit-confirmation overlay is open, suppress all
    // UiAction messages so screen-specific input handlers don't
    // react. The exit-confirm system reads UiAction directly via
    // its own MessageReader.
    if exit_state.showing {
        return;
    }

    // ── Keyboard (no debounce needed) ────────────────────────
    for (kc, action_list) in &mapping.keyboard_map {
        if keys.just_pressed(*kc) {
            for &action in action_list {
                writer.write(action);
            }
        }
    }

    // ── Gamepad (debounced) ──────────────────────────────────
    if !mapping.gamepad_enabled {
        return;
    }

    // Pick the best gamepad. Avoids the old bug where iterating all
    // gamepads and overwriting stick values clobbered real input with
    // zeroes from empty slots (e.g. GPD Win 4 built-in controller).
    let Some(gv) = pick_active_gamepad(&gamepads, &mapping.gamepad_map_keys()) else {
        return;
    };

    // Collect currently-pressed buttons from the active gamepad only.
    let mut currently_pressed: HashMap<GamepadButton, bool> = HashMap::new();
    for &gb in mapping.gamepad_map.keys() {
        if gv.pressed_buttons.contains(&gb) {
            currently_pressed.insert(gb, true);
        }
    }

    let left_stick_x = gv.ls.x;
    let left_stick_y = gv.ls.y;
    let right_stick_x;
    let right_stick_y;
    if mapping.right_stick_pan {
        right_stick_x = gv.rs.x;
        right_stick_y = gv.rs.y;
    } else {
        right_stick_x = 0.0;
        right_stick_y = 0.0;
    }

    // Debounce: emit action only when a button has been held for
    // DEBOUNCE_FRAMES consecutive frames. After firing, don't fire
    // again until the button is released and re-pressed.
    let mut to_fire: Vec<GamepadButton> = Vec::new();
    let mut button_held = std::mem::take(&mut debounce.button_held);
    let mut button_fired = std::mem::take(&mut debounce.button_fired);
    for &gb in mapping.gamepad_map.keys() {
        let held = currently_pressed.get(&gb).copied().unwrap_or(false);
        let held_count = button_held.entry(gb).or_insert(0);
        let fired = button_fired.entry(gb).or_insert(false);

        if held {
            *held_count = held_count.saturating_add(1);
            if *held_count >= DEBOUNCE_FRAMES && !*fired {
                *fired = true;
                to_fire.push(gb);
            }
        } else {
            *held_count = 0;
            *fired = false;
        }
    }
    debounce.button_held = button_held;
    debounce.button_fired = button_fired;
    for gb in to_fire {
        if let Some(action_list) = mapping.gamepad_map.get(&gb) {
            for &action in action_list {
                writer.write(action);
            }
        }
    }

    // Debounce left stick axes → map cursor actions. D-pad remains hero movement.
    let axis_deadzone = 0.5;
    let axis_actions = [
        (
            GamepadAxis::LeftStickX,
            Sign::Positive,
            left_stick_x > axis_deadzone,
            UiAction::CursorRight,
        ),
        (
            GamepadAxis::LeftStickX,
            Sign::Negative,
            left_stick_x < -axis_deadzone,
            UiAction::CursorLeft,
        ),
        (GamepadAxis::LeftStickY, Sign::Positive, left_stick_y > axis_deadzone, UiAction::CursorUp),
        (
            GamepadAxis::LeftStickY,
            Sign::Negative,
            left_stick_y < -axis_deadzone,
            UiAction::CursorDown,
        ),
    ];
    let mut axis_to_fire: Vec<UiAction> = Vec::new();
    let mut axis_held = std::mem::take(&mut debounce.axis_held);
    let mut axis_fired = std::mem::take(&mut debounce.axis_fired);
    for (axis, sign, past_deadzone, action) in axis_actions {
        let key = (axis, sign);
        let held_count = axis_held.entry(key).or_insert(0);
        let fired = axis_fired.entry(key).or_insert(false);
        if past_deadzone {
            *held_count = held_count.saturating_add(1);
            if *held_count >= AXIS_DEBOUNCE_FRAMES && !*fired {
                *fired = true;
                axis_to_fire.push(action);
            }
        } else {
            *held_count = 0;
            *fired = false;
        }
    }
    debounce.axis_held = axis_held;
    debounce.axis_fired = axis_fired;
    for action in axis_to_fire {
        writer.write(action);
    }

    // Right stick → pan (continuous while held past deadzone).
    if mapping.right_stick_pan {
        let stick_deadzone = 0.5;
        if right_stick_x > stick_deadzone {
            writer.write(UiAction::PanRight);
        } else if right_stick_x < -stick_deadzone {
            writer.write(UiAction::PanLeft);
        }
        if right_stick_y > stick_deadzone {
            writer.write(UiAction::PanUp);
        } else if right_stick_y < -stick_deadzone {
            writer.write(UiAction::PanDown);
        }
    }
}

/// Log gamepad connection/disconnection events.
/// Helps diagnose multi-slot issues (e.g. GPD Win 4).
fn log_gamepad_events(
    mut connection_events: MessageReader<GamepadConnectionEvent>,
    mut axis_events: MessageReader<RawGamepadAxisChangedEvent>,
) {
    for event in connection_events.read() {
        match &event.connection {
            GamepadConnection::Connected { name, .. } => {
                tracing::info!("Gamepad connected: entity={:?} name=\"{name}\"", event.gamepad,);
            }
            GamepadConnection::Disconnected => {
                tracing::info!("Gamepad disconnected: entity={:?}", event.gamepad,);
            }
        }
    }
    // Log raw axis changes at debug level so stick input is visible
    // in RUST_LOG=weave_of_realms_bevy=debug logs. This is crucial for
    // diagnosing devices like GPD Win 4 where gilrs may not correctly
    // map stick axes.
    for event in axis_events.read() {
        tracing::debug!(
            "Raw axis: entity={:?} axis={:?} value={:.4}",
            event.gamepad,
            event.axis,
            event.value,
        );
    }
}

/// Triggered on every AppState transition. Suppresses input for
/// COOLDOWN_DURATION seconds so stale presses don't bleed through.
const COOLDOWN_DURATION: f64 = 0.15;

fn trigger_input_cooldown(
    time: Res<Time>,
    mut cooldown: ResMut<InputCooldown>,
    mut actions: ResMut<Messages<UiAction>>,
) {
    *cooldown = InputCooldown::trigger(time.elapsed_secs_f64(), COOLDOWN_DURATION);
    actions.clear();
}

// ── Cursor visibility ────────────────────────────────────────────────

/// Tracks whether the OS cursor should be visible.
///
/// The cursor is shown when the player uses the mouse (move or click)
/// and hidden after a short idle timeout or when the player uses keyboard
/// or gamepad. On devices like the GPD Win 4 where a gamepad is always
/// present, we only hide the cursor when the gamepad actually produces a
/// `UiAction`, not on any stick drift.
#[derive(Resource)]
pub struct CursorVisibility {
    /// Which input method was last active.
    method: InputMethod,
    /// Seconds since the last mouse activity. Used to auto-hide after idle.
    seconds_since_mouse: f32,
    /// How long the mouse can be idle before we hide the cursor.
    hide_after_secs: f32,
}

/// Which input method was used most recently.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum InputMethod {
    #[default]
    KeyboardGamepad,
    Mouse,
}

impl Default for CursorVisibility {
    fn default() -> Self {
        Self { method: InputMethod::default(), seconds_since_mouse: 0.0, hide_after_secs: 3.0 }
    }
}

/// Show the OS cursor when the mouse moves or clicks, hide it when the
/// player uses keyboard/gamepad (or after the mouse has been idle for
/// `hide_after_secs` seconds).
fn update_cursor_visibility(
    accumulated_motion: Res<AccumulatedMouseMotion>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut cursor: Query<&mut CursorOptions>,
    mut state: ResMut<CursorVisibility>,
    ui_actions: Res<Messages<UiAction>>,
) {
    let dt = time.delta_secs();

    // Mouse activity: any movement (delta above 1px threshold to filter
    // out sub-pixel jitter on some devices) or any button click.
    let mouse_moved = accumulated_motion.delta.length_squared() > 1.0;
    let mouse_clicked = mouse_buttons.just_pressed(MouseButton::Left)
        || mouse_buttons.just_pressed(MouseButton::Right)
        || mouse_buttons.just_pressed(MouseButton::Middle);

    // Any keyboard key pressed this frame → non-mouse input.
    let keyboard_active = keys.get_just_pressed().next().is_some();

    // Gamepad produced a UiAction this frame → non-mouse input.
    // We check UiAction messages instead of raw gamepad state because on
    // devices like GPD Win 4 the built-in controller is always "active"
    // and raw stick readings would permanently suppress the cursor.
    let gamepad_active = !ui_actions.is_empty();

    if mouse_moved || mouse_clicked {
        state.method = InputMethod::Mouse;
        state.seconds_since_mouse = 0.0;
    }

    if keyboard_active || gamepad_active {
        state.method = InputMethod::KeyboardGamepad;
    }

    // Tick the idle timer.
    if state.method == InputMethod::Mouse {
        state.seconds_since_mouse += dt;
    }

    // Decide visibility: show cursor while mouse is active and within
    // the idle window. Hide it the moment keyboard/gamepad takes over,
    // or after the mouse has been idle too long.
    let show_cursor =
        state.method == InputMethod::Mouse && state.seconds_since_mouse < state.hide_after_secs;

    for mut cursor_opts in cursor.iter_mut() {
        cursor_opts.visible = show_cursor;
    }
}
