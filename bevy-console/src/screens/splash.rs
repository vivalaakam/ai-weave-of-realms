use bevy::prelude::*;
use crate::screens::AppState;

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
            BackgroundColor(Color::srgb(0.08, 0.08, 0.12)),
            SplashRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Weave of Realms"),
                TextFont { font_size: FontSize::Px(52.0), ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));

            parent.spawn((
                Node { height: Val::Px(32.0), ..default() },
                BackgroundColor(Color::NONE),
            ));

            parent.spawn((
                Button,
                NewGameButton,
                Node {
                    width: Val::Px(200.0),
                    height: Val::Px(50.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.2, 0.2, 0.25)),
                BorderColor::all(Color::srgb(0.4, 0.4, 0.5)),
                children![(
                    Text::new("New Game"),
                    TextFont { font_size: FontSize::Px(22.0), ..default() },
                    TextColor(Color::srgb(0.9, 0.9, 0.9)),
                )],
            ));

            parent.spawn((
                Button,
                LoadGameButton,
                Node {
                    width: Val::Px(200.0),
                    height: Val::Px(50.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.2, 0.2, 0.25)),
                BorderColor::all(Color::srgb(0.4, 0.4, 0.5)),
                children![(
                    Text::new("Load Game"),
                    TextFont { font_size: FontSize::Px(22.0), ..default() },
                    TextColor(Color::srgb(0.9, 0.9, 0.9)),
                )],
            ));

            parent.spawn((
                Node { height: Val::Px(24.0), ..default() },
                BackgroundColor(Color::NONE),
            ));

            parent.spawn((
                Text::new(SPLASH_FOOTER),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::srgb(0.5, 0.5, 0.55)),
            ));
        });
}

#[allow(clippy::type_complexity)]
fn update_splash(
    mut next_state: ResMut<NextState<AppState>>,
    mut splash_state: ResMut<SplashState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<(
        Option<&NewGameButton>,
        Option<&LoadGameButton>,
        &mut BackgroundColor,
        &mut BorderColor,
        &Interaction,
    )>,
) {
    // Keyboard navigation
    if keys.just_pressed(KeyCode::ArrowUp)
        || keys.just_pressed(KeyCode::KeyW)
        || keys.just_pressed(KeyCode::KeyK)
    {
        splash_state.selected = splash_state.selected.saturating_sub(1);
    }
    if keys.just_pressed(KeyCode::ArrowDown)
        || keys.just_pressed(KeyCode::KeyS)
        || keys.just_pressed(KeyCode::KeyJ)
    {
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

        if is_selected {
            *bg = BackgroundColor(Color::srgb(0.35, 0.35, 0.45));
            *border = BorderColor::all(Color::srgb(0.7, 0.7, 0.8));
        } else if pressed {
            *bg = BackgroundColor(Color::srgb(0.3, 0.3, 0.35));
            *border = BorderColor::all(Color::srgb(0.5, 0.5, 0.6));
        } else if hovered {
            *bg = BackgroundColor(Color::srgb(0.25, 0.25, 0.3));
            *border = BorderColor::all(Color::srgb(0.5, 0.5, 0.6));
        } else {
            *bg = BackgroundColor(Color::srgb(0.2, 0.2, 0.25));
            *border = BorderColor::all(Color::srgb(0.4, 0.4, 0.5));
        }

        if (is_selected && keys.just_pressed(KeyCode::Enter)) || pressed {
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
