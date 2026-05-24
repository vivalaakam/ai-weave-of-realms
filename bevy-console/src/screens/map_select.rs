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

// Theme
const BG_COLOR: Color = Color::srgb(0.08, 0.08, 0.12);
const TEXT_COLOR: Color = Color::srgb(0.85, 0.85, 0.88);
const TITLE_COLOR: Color = Color::srgb(0.95, 0.95, 0.98);
const FOOTER_COLOR: Color = Color::srgb(0.5, 0.5, 0.55);
const ROW_BG: Color = Color::srgb(0.14, 0.14, 0.18);
const ROW_BG_SELECTED: Color = Color::srgb(0.28, 0.28, 0.35);
const ROW_BG_HOVER: Color = Color::srgb(0.22, 0.22, 0.28);
const ROW_BG_PRESSED: Color = Color::srgb(0.35, 0.35, 0.42);
const ROW_BORDER: Color = Color::srgb(0.35, 0.35, 0.42);
const ROW_BORDER_SELECTED: Color = Color::srgb(0.65, 0.65, 0.72);
const ROW_BORDER_HOVER: Color = Color::srgb(0.5, 0.5, 0.58);
const ROW_BORDER_PRESSED: Color = Color::srgb(0.6, 0.6, 0.68);
const STATUS_ERROR: Color = Color::srgb(0.9, 0.5, 0.5);

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
            BackgroundColor(BG_COLOR),
            MapSelectRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(TITLE),
                TextFont { font_size: FontSize::Px(36.0), ..default() },
                TextColor(TITLE_COLOR),
            ));

            parent.spawn((Node::default(), BackgroundColor(Color::NONE)));

            for (i, entry) in entries.iter().enumerate() {
                let is_rand = entry.id == "__random_map";
                let bg = if i == 0 { ROW_BG_SELECTED } else { ROW_BG };
                let border = if i == 0 { ROW_BORDER_SELECTED } else { ROW_BORDER };
                parent.spawn((
                    Button,
                    ListEntryIndex(i),
                    Node {
                        width: Val::Px(400.0),
                        height: Val::Px(40.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(bg),
                    BorderColor::all(border),
                    children![(
                        Text::new(if is_rand { format!("> {}", entry.label) } else { entry.label.clone() }),
                        TextFont { font_size: FontSize::Px(18.0), ..default() },
                        TextColor(TEXT_COLOR),
                    )],
                ));
            }

            if let Some(ref status) = state.status {
                parent.spawn((
                    Text::new(status.clone()),
                    TextFont { font_size: FontSize::Px(14.0), ..default() },
                    TextColor(STATUS_ERROR),
                ));
            }

            parent.spawn((Node::default(), BackgroundColor(Color::NONE)));

            parent.spawn((
                Text::new(FOOTER),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(FOOTER_COLOR),
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
        let hovered = matches!(interaction, Interaction::Hovered);
        if pressed {
            *bg = BackgroundColor(ROW_BG_PRESSED);
            *border = BorderColor::all(ROW_BORDER_PRESSED);
        } else if is_sel {
            *bg = BackgroundColor(ROW_BG_SELECTED);
            *border = BorderColor::all(ROW_BORDER_SELECTED);
        } else if hovered {
            *bg = BackgroundColor(ROW_BG_HOVER);
            *border = BorderColor::all(ROW_BORDER_HOVER);
        } else {
            *bg = BackgroundColor(ROW_BG);
            *border = BorderColor::all(ROW_BORDER);
        }

        if ((is_sel && keys.just_pressed(KeyCode::Enter)) || pressed)
            && let Some(entry) = state.entries.get(idx.0) {
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
                            state.status = Some(e.to_string());
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
