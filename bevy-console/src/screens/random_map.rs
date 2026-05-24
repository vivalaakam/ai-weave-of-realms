use crate::app_host::{AppHost, PendingMapData};
use crate::screens::AppState;
use bevy::prelude::*;
use tracing::error;

#[derive(Component)]
pub struct RandomMapRoot;

#[derive(Component)]
struct SeedText;

#[derive(Component)]
struct StatusText;

#[derive(Component)]
struct RandomButton;

#[derive(Component)]
struct PlayButton;

#[derive(Resource, Default)]
pub struct RandomMapState {
    pub seed: Option<String>,
}

const TITLE: &str = "Random Map";
const FOOTER: &str = "Up/Down: select  Enter: action  Back: map list";

pub struct RandomMapPlugin;

impl Plugin for RandomMapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RandomMapState>()
            .add_systems(OnEnter(AppState::RandomMap), enter_random_map)
            .add_systems(OnExit(AppState::RandomMap), exit_random_map)
            .add_systems(Update, update_random_map.run_if(in_state(AppState::RandomMap)));
    }
}

fn enter_random_map(mut commands: Commands) {
    commands.insert_resource(RandomMapState::default());
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.08, 0.12)),
            RandomMapRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(TITLE),
                TextFont { font_size: FontSize::Px(36.0), ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));

            parent.spawn((Node::default(), BackgroundColor(Color::NONE)));

            parent.spawn((
                Text::new("Seed: -"),
                SeedText,
                TextFont { font_size: FontSize::Px(20.0), ..default() },
                TextColor(Color::srgb(0.7, 0.7, 0.75)),
            ));

            parent.spawn((
                Text::new("Press Random to generate seed"),
                StatusText,
                TextFont { font_size: FontSize::Px(16.0), ..default() },
                TextColor(Color::srgb(0.5, 0.5, 0.55)),
            ));

            parent.spawn((Node::default(), BackgroundColor(Color::NONE)));

            parent.spawn((
                Button,
                RandomButton,
                Node {
                    width: Val::Px(160.0),
                    height: Val::Px(44.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.2, 0.2, 0.25)),
                BorderColor::all(Color::srgb(0.4, 0.4, 0.5)),
                children![(
                    Text::new("Random"),
                    TextFont { font_size: FontSize::Px(18.0), ..default() },
                    TextColor(Color::srgb(0.9, 0.9, 0.9)),
                )],
            ));

            parent.spawn((
                Button,
                PlayButton,
                Node {
                    width: Val::Px(160.0),
                    height: Val::Px(44.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.2, 0.2, 0.25)),
                BorderColor::all(Color::srgb(0.4, 0.4, 0.5)),
                children![(
                    Text::new("Play"),
                    TextFont { font_size: FontSize::Px(18.0), ..default() },
                    TextColor(Color::srgb(0.9, 0.9, 0.9)),
                )],
            ));

            parent.spawn((Node::default(), BackgroundColor(Color::NONE)));
            parent.spawn((
                Text::new(FOOTER),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::srgb(0.5, 0.5, 0.55)),
            ));
        });
}

#[allow(clippy::type_complexity)]
fn update_random_map(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    mut host: ResMut<AppHost>,
    mut state: ResMut<RandomMapState>,
    keys: Res<ButtonInput<KeyCode>>,
    random_btns: Query<&Interaction, With<RandomButton>>,
    play_btns: Query<&Interaction, With<PlayButton>>,
    mut text_query: Query<&mut Text>,
    seed_texts: Query<Entity, With<SeedText>>,
    status_texts: Query<Entity, With<StatusText>>,
) {
    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Backspace) {
        next_state.set(AppState::MapSelect);
        return;
    }

    let random_pressed = random_btns.iter().any(|i| matches!(i, Interaction::Pressed));
    let play_pressed = play_btns.iter().any(|i| matches!(i, Interaction::Pressed));

    if random_pressed {
        let seed = format!(
            "{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        state.seed = Some(seed.clone());
        for entity in seed_texts.iter() {
            if let Ok(mut txt) = text_query.get_mut(entity) {
                txt.0 = format!("Seed: {}", seed);
            }
        }
        for entity in status_texts.iter() {
            if let Ok(mut txt) = text_query.get_mut(entity) {
                txt.0 = "Press Play to generate the map".to_string();
            }
        }
    }

    if play_pressed {
        if let Some(ref seed) = state.seed {
            match host.generate_and_save_map(seed) {
                Ok(entry) => match host.load_map_only(&entry) {
                    Ok(map) => {
                        commands.insert_resource(PendingMapData {
                            map_name: entry.label.clone(),
                            map: Some(map),
                        });
                        next_state.set(AppState::TeamSetup);
                    }
                    Err(e) => {
                        error!(%e, "failed load to generated map");
                        for entity in status_texts.iter() {
                            if let Ok(mut txt) = text_query.get_mut(entity) {
                                txt.0 = e.to_string();
                            }
                        }
                    }
                },
                Err(e) => {
                    error!(%e, "failed to generate map");
                    for entity in status_texts.iter() {
                        if let Ok(mut txt) = text_query.get_mut(entity) {
                            txt.0 = e.to_string();
                        }
                    }
                }
            }
        } else {
            for entity in status_texts.iter() {
                if let Ok(mut txt) = text_query.get_mut(entity) {
                    txt.0 = "Generate a seed first".to_string();
                }
            }
        }
    }
}

fn exit_random_map(mut commands: Commands, query: Query<Entity, With<RandomMapRoot>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}
