use crate::input::UiAction;
use crate::screens::AppState;
use bevy::prelude::*;

// ===== Theme constants (embedded-style flat UI) =====
const BG_COLOR: Color = Color::srgb(0.08, 0.08, 0.12);
const TEXT_COLOR: Color = Color::srgb(0.85, 0.85, 0.88);
const TITLE_COLOR: Color = Color::srgb(0.95, 0.95, 0.98);
const FOOTER_COLOR: Color = Color::srgb(0.5, 0.5, 0.55);
const BTN_BG: Color = Color::srgb(0.14, 0.14, 0.18);
const BTN_BG_HOVER: Color = Color::srgb(0.22, 0.22, 0.28);
const BTN_BG_SELECTED: Color = Color::srgb(0.28, 0.28, 0.35);
const BTN_BG_PRESSED: Color = Color::srgb(0.35, 0.35, 0.42);
const BTN_BORDER: Color = Color::srgb(0.4, 0.4, 0.48);
const BTN_BORDER_HOVER: Color = Color::srgb(0.55, 0.55, 0.62);
const BTN_BORDER_SELECTED: Color = Color::srgb(0.7, 0.7, 0.78);
const BTN_BORDER_PRESSED: Color = Color::srgb(0.65, 0.65, 0.72);

#[derive(Component)]
pub struct SplashRoot;

#[derive(Component)]
struct NewGameButton;

#[derive(Component)]
struct LoadGameButton;

#[derive(Resource, Default)]
struct SplashState {
    selected: usize,
}

const SPLASH_OPTIONS: [&str; 2] = ["New Game", "Load Game"];
const SPLASH_FOOTER: &str = "Enter: select  W/S: move";

pub struct SplashPlugin;

impl Plugin for SplashPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SplashState>()
            .add_systems(OnEnter(AppState::Splash), enter_splash)
            .add_systems(OnExit(AppState::Splash), exit_splash)
            .add_systems(Update, update_splash.run_if(in_state(AppState::Splash)));
    }
}

// Common button node style
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

fn enter_splash(mut commands: Commands) {
    commands.insert_resource(SplashState::default());

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(BG_COLOR),
            SplashRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Weave of Realms"),
                TextFont { font_size: FontSize::Px(52.0), ..default() },
                TextColor(TITLE_COLOR),
            ));

            parent
                .spawn((Node { height: Val::Px(32.0), ..default() }, BackgroundColor(Color::NONE)));

            // New Game — framed flat button
            parent.spawn((
                Button,
                NewGameButton,
                button_node(200.0, 50.0),
                BackgroundColor(BTN_BG),
                BorderColor::all(BTN_BORDER),
                children![(
                    Text::new("New Game"),
                    TextFont { font_size: FontSize::Px(22.0), ..default() },
                    TextColor(TEXT_COLOR),
                )],
            ));

            // Load Game — framed flat button
            parent.spawn((
                Button,
                LoadGameButton,
                button_node(200.0, 50.0),
                BackgroundColor(BTN_BG),
                BorderColor::all(BTN_BORDER),
                children![(
                    Text::new("Load Game"),
                    TextFont { font_size: FontSize::Px(22.0), ..default() },
                    TextColor(TEXT_COLOR),
                )],
            ));

            parent
                .spawn((Node { height: Val::Px(24.0), ..default() }, BackgroundColor(Color::NONE)));

            parent.spawn((
                Text::new(SPLASH_FOOTER),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(FOOTER_COLOR),
            ));
        });
}

#[allow(clippy::type_complexity)]
fn update_splash(
    mut next_state: ResMut<NextState<AppState>>,
    mut splash_state: ResMut<SplashState>,
    mut reader: MessageReader<UiAction>,
    mut query: Query<(
        Option<&NewGameButton>,
        Option<&LoadGameButton>,
        &mut BackgroundColor,
        &mut BorderColor,
        &Interaction,
    )>,
) {
    // Collect all actions this frame so we can check multiple times.
    let actions: Vec<UiAction> = reader.read().copied().collect();
    let has = |action: UiAction| actions.contains(&action);

    // Keyboard / gamepad navigation
    if has(UiAction::Up) {
        splash_state.selected = splash_state.selected.saturating_sub(1);
    }
    if has(UiAction::Down) {
        splash_state.selected = (splash_state.selected + 1).min(SPLASH_OPTIONS.len() - 1);
    }

    let selected = splash_state.selected;

    for (new_game_opt, load_game_opt, mut bg, mut border, interaction) in query.iter_mut() {
        let is_selected = match (new_game_opt, load_game_opt) {
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

        if (is_selected && has(UiAction::Confirm)) || pressed {
            if new_game_opt.is_some() {
                next_state.set(AppState::MapSelect);
            } else {
                next_state.set(AppState::SaveSelect);
            }
        }
    }
}

fn exit_splash(mut commands: Commands, query: Query<Entity, With<SplashRoot>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}
