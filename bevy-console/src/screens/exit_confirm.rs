//! Exit-confirmation overlay (Ctrl+Q → "Quit?" → Enter/Cancel).
//!
//! A global overlay that can appear on any screen. Pressing Ctrl+Q shows
//! the dialog; Enter confirms and closes the app; Cancel (Esc) or Ctrl+Q
//! again dismisses it. All other input is blocked while the overlay is open.

use bevy::app::AppExit;
use bevy::prelude::*;

// ── Theme (matches splash.rs / map_view.rs) ──────────────────────────

const OVERLAY_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.7);
const PANEL_BG: Color = Color::srgb(0.18, 0.18, 0.24);
const PANEL_BORDER: Color = Color::srgb(0.5, 0.5, 0.6);
const TEXT_COLOR: Color = Color::srgb(0.85, 0.85, 0.88);
const FOOTER_COLOR: Color = Color::srgb(0.5, 0.5, 0.55);
const BTN_BG: Color = Color::srgb(0.14, 0.14, 0.18);
const BTN_BG_HOVER: Color = Color::srgb(0.22, 0.22, 0.28);
const BTN_BG_SELECTED: Color = Color::srgb(0.28, 0.28, 0.35);
const BTN_BG_PRESSED: Color = Color::srgb(0.35, 0.35, 0.42);
const BTN_BORDER: Color = Color::srgb(0.4, 0.4, 0.48);
const BTN_BORDER_HOVER: Color = Color::srgb(0.55, 0.55, 0.62);
const BTN_BORDER_SELECTED: Color = Color::srgb(0.7, 0.7, 0.78);
const BTN_BORDER_PRESSED: Color = Color::srgb(0.65, 0.65, 0.72);

// ── Components ────────────────────────────────────────────────────────

#[derive(Component)]
pub struct ExitConfirmOverlay;

#[derive(Component)]
struct ExitConfirmButton;

#[derive(Component)]
struct ExitCancelButton;

// ── State ─────────────────────────────────────────────────────────────

/// Whether the exit-confirmation overlay is currently showing.
#[derive(Resource, Default)]
pub struct ExitConfirmState {
    pub showing: bool,
    /// 0 = Confirm (Quit), 1 = Cancel
    pub selected: usize,
}

// ── Plugin ─────────────────────────────────────────────────────────────

pub struct ExitConfirmPlugin;

impl Plugin for ExitConfirmPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ExitConfirmState>()
            .add_systems(Update, check_ctrl_q)
            .add_systems(Update, handle_exit_confirm.run_if(exit_confirm_visible));
    }
}

// ── Systems ────────────────────────────────────────────────────────────

/// Detect Ctrl+Q press. Toggles the exit-confirmation overlay.
fn check_ctrl_q(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<ExitConfirmState>,
    mut commands: Commands,
    overlay_q: Query<Entity, With<ExitConfirmOverlay>>,
) {
    if !keys.just_pressed(KeyCode::KeyQ) || !keys.pressed(KeyCode::ControlLeft) && !keys.pressed(KeyCode::ControlRight) {
        return;
    }

    if state.showing {
        // Already showing → dismiss.
        dismiss_overlay(&mut state, &mut commands, &overlay_q);
    } else {
        // Show overlay.
        state.showing = true;
        state.selected = 1; // Default to "Cancel" to prevent accidental quits.
        spawn_overlay(commands);
    }
}

/// Main update loop for the exit-confirmation overlay.
/// Reads keyboard/gamepad directly (not UiAction) because UiAction
/// generation is suppressed while the overlay is showing.
fn handle_exit_confirm(
    mut state: ResMut<ExitConfirmState>,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut app_exit: MessageWriter<AppExit>,
    mut commands: Commands,
    overlay_q: Query<Entity, With<ExitConfirmOverlay>>,
    mut btn_query: Query<
        (
            Option<&ExitConfirmButton>,
            Option<&ExitCancelButton>,
            &mut BackgroundColor,
            &mut BorderColor,
            &Interaction,
        ),
        Without<ExitConfirmOverlay>,
    >,
) {
    // Keyboard: Left/Right or A/D to navigate between Confirm and Cancel.
    if keys.just_pressed(KeyCode::ArrowLeft)
        || keys.just_pressed(KeyCode::KeyA)
        || keys.just_pressed(KeyCode::ArrowUp)
        || keys.just_pressed(KeyCode::KeyW)
    {
        state.selected = state.selected.saturating_sub(1);
    }
    if keys.just_pressed(KeyCode::ArrowRight)
        || keys.just_pressed(KeyCode::KeyD)
        || keys.just_pressed(KeyCode::ArrowDown)
        || keys.just_pressed(KeyCode::KeyS)
    {
        state.selected = (state.selected + 1).min(1);
    }

    // Enter → confirm selection.
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        if state.selected == 0 {
            app_exit.write(AppExit::Success);
        } else {
            dismiss_overlay(&mut state, &mut commands, &overlay_q);
        }
        return;
    }

    // Escape → cancel.
    if keys.just_pressed(KeyCode::Escape) {
        dismiss_overlay(&mut state, &mut commands, &overlay_q);
        return;
    }

    // Tab key toggles between options.
    if keys.just_pressed(KeyCode::Tab) {
        state.selected = if state.selected == 0 { 1 } else { 0 };
    }

    // Gamepad: D-pad or sticks for navigation, South (A) to confirm, East (B) to cancel.
    for gamepad in gamepads.iter() {
        if gamepad.pressed(GamepadButton::DPadLeft) || gamepad.pressed(GamepadButton::DPadUp) {
            state.selected = state.selected.saturating_sub(1);
        }
        if gamepad.pressed(GamepadButton::DPadRight) || gamepad.pressed(GamepadButton::DPadDown) {
            state.selected = (state.selected + 1).min(1);
        }
        if gamepad.just_pressed(GamepadButton::South) {
            if state.selected == 0 {
                app_exit.write(AppExit::Success);
            } else {
                dismiss_overlay(&mut state, &mut commands, &overlay_q);
            }
            return;
        }
        if gamepad.just_pressed(GamepadButton::East) {
            dismiss_overlay(&mut state, &mut commands, &overlay_q);
            return;
        }
    }

    // Update button visuals.
    let selected = state.selected;
    for (confirm_opt, cancel_opt, mut bg, mut border, interaction) in btn_query.iter_mut() {
        let is_selected = match (confirm_opt, cancel_opt) {
            (Some(_), None) => selected == 0,
            (None, Some(_)) => selected == 1,
            _ => continue,
        };

        let hovered = matches!(interaction, Interaction::Hovered);
        let pressed = matches!(interaction, Interaction::Pressed);

        if pressed {
            *bg = BackgroundColor(BTN_BG_PRESSED);
            *border = BorderColor::all(BTN_BORDER_PRESSED);
        } else if is_selected {
            *bg = BackgroundColor(BTN_BG_SELECTED);
            *border = BorderColor::all(BTN_BORDER_SELECTED);
        } else if hovered {
            *bg = BackgroundColor(BTN_BG_HOVER);
            *border = BorderColor::all(BTN_BORDER_HOVER);
        } else {
            *bg = BackgroundColor(BTN_BG);
            *border = BorderColor::all(BTN_BORDER);
        }

        // Mouse click on button.
        if pressed {
            if confirm_opt.is_some() {
                app_exit.write(AppExit::Success);
            } else {
                dismiss_overlay(&mut state, &mut commands, &overlay_q);
            }
        }
    }
}

// ── Spawn / despawn ───────────────────────────────────────────────────

fn button_node(w: f32, h: f32) -> Node {
    Node {
        width: Val::Px(w),
        height: Val::Px(h),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(2.0)),
        ..default()
    }
}

fn spawn_overlay(mut commands: Commands) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(OVERLAY_BG),
            ExitConfirmOverlay,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        width: Val::Px(420.0),
                        height: Val::Px(220.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(20.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(PANEL_BG),
                    BorderColor::all(PANEL_BORDER),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("Quit game?"),
                        TextFont { font_size: FontSize::Px(28.0), ..default() },
                        TextColor(TEXT_COLOR),
                    ));

                    // Row with two buttons side by side.
                    panel
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(24.0),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn((
                                Button,
                                ExitConfirmButton,
                                button_node(160.0, 50.0),
                                BackgroundColor(BTN_BG),
                                BorderColor::all(BTN_BORDER),
                                children![(
                                    Text::new("Quit"),
                                    TextFont { font_size: FontSize::Px(20.0), ..default() },
                                    TextColor(TEXT_COLOR),
                                )],
                            ));
                            row.spawn((
                                Button,
                                ExitCancelButton,
                                button_node(160.0, 50.0),
                                BackgroundColor(BTN_BG),
                                BorderColor::all(BTN_BORDER),
                                children![(
                                    Text::new("Cancel"),
                                    TextFont { font_size: FontSize::Px(20.0), ..default() },
                                    TextColor(TEXT_COLOR),
                                )],
                            ));
                        });

                    panel.spawn((
                        Text::new("Enter: confirm  Esc: cancel  Ctrl+Q: toggle"),
                        TextFont { font_size: FontSize::Px(12.0), ..default() },
                        TextColor(FOOTER_COLOR),
                    ));
                });
        });
}

fn dismiss_overlay(
    state: &mut ResMut<ExitConfirmState>,
    commands: &mut Commands,
    overlay_q: &Query<Entity, With<ExitConfirmOverlay>>,
) {
    state.showing = false;
    for entity in overlay_q.iter() {
        commands.entity(entity).despawn();
    }
}

/// Run condition: only update when the overlay is visible.
fn exit_confirm_visible(state: Res<ExitConfirmState>) -> bool {
    state.showing
}