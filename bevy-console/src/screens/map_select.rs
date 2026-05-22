use bevy::prelude::*;
use crate::app_host::{AppHost, PendingMapData};
use crate::screens::AppState;
use helpers::ListEntry;

#[derive(Component)]
pub struct MapSelectRoot;

#[derive(Component)]
struct ListEntryIndex(usize);

#[derive(Resource, Default)]
pub struct MapSelectState {
    pub selected: usize,
    pub entries: Vec<ListEntry>,
    pub status: Option<String>,
}

const TITLE: &str = "Maps";
const FOOTER: &str = "Up/Down: select  Enter: load  Back: splash";

pub struct MapSelectPlugin;

impl Plugin for MapSelectPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapSelectState>()
            .add_systems(OnEnter(AppState::MapSelect), enter_map_select)
            .add_systems(OnExit(AppState::MapSelect), exit_map_select)
            .add_systems(Update, update_map_select.run_if(in_state(AppState::MapSelect)));
    }
}

fn enter_map_select(
    mut commands: Commands,
    mut host: ResMut<AppHost>,
    mut state: ResMut<MapSelectState>,
) {
    state.selected = 0;
    state.status = None;
    let mut entries = Vec::new();

    entries.push(ListEntry {
        id: "__random_map".to_string(),
        label: "Random Map".to_string(),
        meta: 0,
    });

    match host.discover_maps() {
        Ok(mut found) => {
            found.retain(|e| !e.id.starts_with("generated:"));
            entries.append(&mut found);
        }
        Err(e) => {
            state.status = Some(e.to_string());
        }
    }
    state.entries = entries.clone();

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.08, 0.12)),
            MapSelectRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(TITLE),
                TextFont { font_size: FontSize::Px(36.0), ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));

            parent.spawn((Node::default(), BackgroundColor(Color::NONE)));

            for (i, entry) in entries.iter().enumerate() {
                parent.spawn((
                    Button,
                    ListEntryIndex(i),
                    Node {
                        width: Val::Px(400.0),
                        height: Val::Px(40.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.15, 0.15, 0.2)),
                    BorderColor::all(Color::srgb(0.3, 0.3, 0.35)),
                    children![(
                        Text::new(entry.label.clone()),
                        TextFont { font_size: FontSize::Px(18.0), ..default() },
                        TextColor(Color::srgb(0.85, 0.85, 0.85)),
                    )],
                ));
            }

            parent.spawn((Node::default(), BackgroundColor(Color::NONE)));

            parent.spawn((
                Text::new(FOOTER),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::srgb(0.5, 0.5, 0.55)),
            ));
        });
}

#[allow(clippy::type_complexity)]
fn update_map_select(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    mut host: ResMut<AppHost>,
    mut state: ResMut<MapSelectState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut buttons: Query<(
        &ListEntryIndex,
        &mut BackgroundColor,
        &mut BorderColor,
        &Interaction,
    )>,
) {
    let old = state.selected;
    if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) {
        state.selected = state.selected.saturating_sub(1);
    }
    if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) {
        let max = state.entries.len().saturating_sub(1);
        state.selected = (state.selected + 1).min(max);
    }
    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Backspace) {
        next_state.set(AppState::Splash);
        return;
    }

    let selected = state.selected;

    for (idx, mut bg, mut border, interaction) in buttons.iter_mut() {
        let is_sel = idx.0 == selected;
        let pressed = matches!(interaction, Interaction::Pressed);
        if is_sel {
            *bg = BackgroundColor(Color::srgb(0.3, 0.3, 0.4));
            *border = BorderColor::all(Color::srgb(0.6, 0.6, 0.7));
        } else if pressed {
            *bg = BackgroundColor(Color::srgb(0.25, 0.25, 0.3));
            *border = BorderColor::all(Color::srgb(0.45, 0.45, 0.5));
        } else {
            *bg = BackgroundColor(Color::srgb(0.15, 0.15, 0.2));
            *border = BorderColor::all(Color::srgb(0.3, 0.3, 0.35));
        }

        if (is_sel && keys.just_pressed(KeyCode::Enter)) || pressed {
            if let Some(entry) = state.entries.get(idx.0) {
                if entry.id == "__random_map" {
                    next_state.set(AppState::RandomMap);
                } else {
                    match host.load_map_only(entry) {
                        Ok(map) => {
                            commands.insert_resource(PendingMapData {
                                map_name: entry.label.clone(),
                                map: Some(map),
                            });
                            next_state.set(AppState::TeamSetup);
                        }
                        Err(e) => {
                            state.status = Some(host.error_message(e));
                        }
                    }
                }
            }
        }
    }

    if old != selected && state.status.is_some() {
        state.status = None;
    }
}

fn exit_map_select(
    mut commands: Commands,
    query: Query<Entity, With<MapSelectRoot>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}
