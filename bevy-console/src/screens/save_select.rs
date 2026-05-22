use bevy::prelude::*;
use crate::app_host::AppHost;
use crate::screens::AppState;
use helpers::ListEntry;

#[derive(Component)]
pub struct SaveSelectRoot;

#[derive(Component)]
struct ListEntryIndex(usize);

#[derive(Resource, Default)]
pub struct SaveSelectState {
    pub selected: usize,
    pub entries: Vec<ListEntry>,
    pub status: Option<String>,
}

const TITLE: &str = "Saves";
const FOOTER: &str = "Up/Down: select  Enter: load  Back: splash";

pub struct SaveSelectPlugin;

impl Plugin for SaveSelectPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SaveSelectState>()
            .add_systems(OnEnter(AppState::SaveSelect), enter_save_select)
            .add_systems(OnExit(AppState::SaveSelect), exit_save_select)
            .add_systems(Update, update_save_select.run_if(in_state(AppState::SaveSelect)));
    }
}

fn enter_save_select(
    mut commands: Commands,
    mut host: ResMut<AppHost>,
    mut state: ResMut<SaveSelectState>,
) {
    state.selected = 0;
    state.status = None;
    let entries = match host.discover_saves() {
        Ok(e) => e,
        Err(e) => {
            state.status = Some(e.to_string());
            Vec::new()
        }
    };
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
            SaveSelectRoot,
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
fn update_save_select(
    mut next_state: ResMut<NextState<AppState>>,
    mut host: ResMut<AppHost>,
    mut state: ResMut<SaveSelectState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut buttons: Query<(
        &ListEntryIndex,
        &mut BackgroundColor,
        &mut BorderColor,
        &Interaction,
    )>,
) {
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
                match host.load_save(entry) {
                    Ok(_loaded) => {
                        // TODO: insert LoadedGame resource
                        next_state.set(AppState::MapView);
                    }
                    Err(e) => {
                        state.status = Some(e.to_string());
                    }
                }
            }
        }
    }
}

fn exit_save_select(
    mut commands: Commands,
    query: Query<Entity, With<SaveSelectRoot>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}
