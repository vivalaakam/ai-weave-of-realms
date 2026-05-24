use bevy::prelude::*;
use crate::screens::AppState;
use crate::screens::team_setup::LoadedSession;
use engine::map::tile::Tiles;
use game::input::InputEvent;
use game::map_view::{MapViewApp, MapViewOutcome};
use game::session::GameSession;

#[derive(Component)]
pub struct MapViewRoot;

#[derive(Component)]
pub struct EndTurnOverlay;

#[derive(Resource)]
pub struct MapViewState {
    pub map_view: Option<Box<MapViewApp>>,
    pub tile_size: f32,
    pub visible_cols: usize,
    pub visible_rows: usize,
    pub needs_initial_draw: bool,
    pub end_turn_overlay: bool,
}

const TEXT_COLOR: Color = Color::srgb(0.85, 0.85, 0.88);
const CURSOR_COLOR: Color = Color::srgb(0.9, 0.9, 0.3);
const OVERLAY_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.7);
const OVERLAY_PANEL_BG: Color = Color::srgb(0.18, 0.18, 0.24);
const OVERLAY_PANEL_BORDER: Color = Color::srgb(0.5, 0.5, 0.6);

const ATLAS_COLS: u32 = 49;
const ATLAS_ROWS: u32 = 22;
const ATLAS_TILE_W: u32 = 16;
const ATLAS_TILE_H: u32 = 16;

pub struct MapViewPlugin;

impl Plugin for MapViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapViewState>()
            .add_systems(OnEnter(AppState::MapView), enter_map_view)
            .add_systems(OnExit(AppState::MapView), exit_map_view)
            .add_systems(Update, update_map_view.run_if(in_state(AppState::MapView)));
    }
}

impl Default for MapViewState {
    fn default() -> Self {
        Self {
            map_view: None,
            tile_size: 32.0,
            visible_cols: 0,
            visible_rows: 0,
            needs_initial_draw: true,
            end_turn_overlay: false,
        }
    }
}

fn tile_color_for(kind: Tiles) -> Color {
    let (r, g, b) = kind.as_color();
    Color::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

fn tile_atlas_index(kind: Tiles) -> usize {
    kind.atlas_index() as usize
}

#[derive(Component)]
pub struct MapTile;

#[derive(Component)]
pub struct MapTilePos {
    pub col: usize,
    pub row: usize,
}

fn enter_map_view(
    commands: Commands,
    map_view_state: ResMut<MapViewState>,
    loaded: Option<ResMut<LoadedSession>>,
    window: Single<&Window>,
    asset_server: Res<AssetServer>,
    atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    enter_map_view_impl(commands, map_view_state, loaded, window, asset_server, atlas_layouts);
}

fn enter_map_view_impl(
    mut commands: Commands,
    mut map_view_state: ResMut<MapViewState>,
    mut loaded: Option<ResMut<LoadedSession>>,
    window: Single<&Window>,
    asset_server: Res<AssetServer>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let Some(mut loaded) = loaded.take() else {
        return;
    };

    let Some(state) = loaded.state.take() else {
        return;
    };

    let session = match GameSession::from_state(loaded.map_name.clone(), state) {
        Ok(s) => s,
        Err(_e) => {
            return;
        }
    };

    let tile_size = map_view_state.tile_size;
    let map_h = window.height() - 40.0;
    let visible_cols = (window.width() / tile_size).max(1.0) as usize;
    let visible_rows = (map_h / tile_size).max(1.0) as usize;

    let map_view = MapViewApp::new(session, 0, 0, None);
    map_view_state.map_view = Some(Box::new(map_view));
    map_view_state.visible_cols = visible_cols;
    map_view_state.visible_rows = visible_rows;
    map_view_state.needs_initial_draw = true;

    let total_w = visible_cols as f32 * tile_size;
    let total_h = visible_rows as f32 * tile_size;
    let offset_x = -total_w / 2.0 + tile_size / 2.0;
    let offset_y = total_h / 2.0 - tile_size / 2.0;

    let atlas_handle: Handle<Image> = asset_server.load("1_main.png");
    let layout = TextureAtlasLayout::from_grid(
        UVec2::new(ATLAS_TILE_W, ATLAS_TILE_H),
        ATLAS_COLS,
        ATLAS_ROWS,
        None,
        None,
    );
    let layout_handle = atlas_layouts.add(layout);

    for row in 0..visible_rows {
        for col in 0..visible_cols {
            let x = offset_x + col as f32 * tile_size;
            let y = offset_y - row as f32 * tile_size;
            commands.spawn((
                Sprite {
                    image: atlas_handle.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: layout_handle.clone(),
                        index: 0,
                    }),
                    custom_size: Some(Vec2::splat(tile_size)),
                    ..Default::default()
                },
                Transform::from_xyz(x, y, 0.0),
                MapTile,
                MapTilePos { col, row },
            ));
        }
    }

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(40.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.14)),
            MapViewRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(TEXT_COLOR),
            ));
        });
}

fn update_map_view(
    mut commands: Commands,
    mut map_view_state: ResMut<MapViewState>,
    mut next_state: ResMut<NextState<AppState>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut status_query: Query<&mut Text>,
    mut tile_query: Query<(&MapTilePos, &mut Sprite)>,
    overlay_q: Query<Entity, With<EndTurnOverlay>>,
) {
    let visible_cols = map_view_state.visible_cols;
    let visible_rows = map_view_state.visible_rows;

    let Some(mut map_view_box) = map_view_state.map_view.take() else {
        return;
    };
    let map_view = &mut *map_view_box;

    if map_view_state.end_turn_overlay {
        if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
            if let Some(entity) = overlay_q.iter().next() {
                commands.entity(entity).despawn();
            }
            map_view_state.end_turn_overlay = false;
            match map_view.session_mut().end_turn() {
                Ok(summary) => {
                    map_view.set_status(Some(summary));
                }
                Err(e) => {
                    map_view.set_status(Some(e.to_string()));
                }
            }
            map_view_state.needs_initial_draw = true;
            map_view_state.map_view = Some(map_view_box);
            return;
        }
        if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Backspace) {
            if let Some(entity) = overlay_q.iter().next() {
                commands.entity(entity).despawn();
            }
            map_view_state.end_turn_overlay = false;
            map_view_state.map_view = Some(map_view_box);
            return;
        }
        map_view_state.map_view = Some(map_view_box);
        return;
    }

    let mut events = Vec::new();
    if keys.just_pressed(KeyCode::ArrowUp) {
        events.push(InputEvent::Up);
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        events.push(InputEvent::Down);
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        events.push(InputEvent::Left);
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        events.push(InputEvent::Right);
    }
    if keys.just_pressed(KeyCode::KeyH) {
        events.push(InputEvent::CursorLeft);
    }
    if keys.just_pressed(KeyCode::KeyJ) {
        events.push(InputEvent::CursorDown);
    }
    if keys.just_pressed(KeyCode::KeyK) {
        events.push(InputEvent::CursorUp);
    }
    if keys.just_pressed(KeyCode::KeyL) {
        events.push(InputEvent::CursorRight);
    }
    if keys.just_pressed(KeyCode::KeyW) {
        events.push(InputEvent::PanUp);
    }
    if keys.just_pressed(KeyCode::KeyS) {
        events.push(InputEvent::PanDown);
    }
    if keys.just_pressed(KeyCode::KeyA) {
        events.push(InputEvent::PanLeft);
    }
    if keys.just_pressed(KeyCode::KeyD) {
        events.push(InputEvent::PanRight);
    }
    if keys.just_pressed(KeyCode::Tab) {
        events.push(InputEvent::NextHero);
    }
    if keys.just_pressed(KeyCode::Enter) {
        events.push(InputEvent::Enter);
    }
    if keys.just_pressed(KeyCode::KeyQ) {
        events.push(InputEvent::Key('q'));
    }
    if keys.just_pressed(KeyCode::KeyE) {
        events.push(InputEvent::NextTurn);
    }
    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Backspace) {
        events.push(InputEvent::Back);
    }

    let mut needs_redraw = map_view_state.needs_initial_draw;
    map_view_state.needs_initial_draw = false;
    let mut request_end_turn = false;

    for event in events {
        let outcome = map_view.handle_input(event, visible_cols, visible_rows);
        match outcome {
            MapViewOutcome::NoChange => {}
            MapViewOutcome::Changed | MapViewOutcome::CursorChanged => {
                needs_redraw = true;
            }
            MapViewOutcome::BackRequested => {
                next_state.set(AppState::Splash);
                map_view_state.map_view = Some(map_view_box);
                return;
            }
            MapViewOutcome::RequestEndTurn => {
                request_end_turn = true;
            }
            MapViewOutcome::GameOver { .. } => {
                next_state.set(AppState::Splash);
                map_view_state.map_view = Some(map_view_box);
                return;
            }
            MapViewOutcome::OpenStructureOverlay { name } => {
                map_view.set_status(Some(format!("Structure: {}", name)));
                needs_redraw = true;
            }
        }
    }

    if request_end_turn {
        map_view_state.end_turn_overlay = true;
        let team_name = {
            let session = map_view.session();
            let state = session.state();
            let team = state.get_active_team().ok();
            team.map(|t| t.name.clone()).unwrap_or_else(|| "Unknown".to_string())
        };
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
                EndTurnOverlay,
            ))
            .with_children(|parent| {
                parent
                    .spawn((
                        Node {
                            width: Val::Px(400.0),
                            height: Val::Px(200.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(16.0),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(OVERLAY_PANEL_BG),
                        BorderColor::all(OVERLAY_PANEL_BORDER),
                    ))
                    .with_children(|panel| {
                        panel.spawn((
                            Text::new(format!("End turn for {}?", team_name)),
                            TextFont { font_size: FontSize::Px(22.0), ..default() },
                            TextColor(TEXT_COLOR),
                        ));
                        panel.spawn((
                            Text::new("Press Enter to confirm or Esc to cancel"),
                            TextFont { font_size: FontSize::Px(14.0), ..default() },
                            TextColor(Color::srgb(0.6, 0.6, 0.65)),
                        ));
                    });
            });
        map_view_state.map_view = Some(map_view_box);
        return;
    }

    let status_text = map_view.status().unwrap_or("").to_string();
    for mut text in status_query.iter_mut() {
        text.0 = status_text.clone();
    }

    if needs_redraw {
        let session = map_view.session();
        let map = &session.state().map;
        let view_x = map_view.view_x();
        let view_y = map_view.view_y();
        let cursor_x = map_view.cursor_x();
        let cursor_y = map_view.cursor_y();

        for (tile_pos, mut sprite) in tile_query.iter_mut() {
            let tx = view_x + tile_pos.col;
            let ty = view_y + tile_pos.row;

            let is_cursor = tx as isize == cursor_x && ty as isize == cursor_y;
            sprite.color = if is_cursor {
                CURSOR_COLOR
            } else {
                let coord = engine::map::game_map::MapCoord::new(tx as u32, ty as u32);
                map.get_tile(coord).map(|t| tile_color_for(t.kind)).unwrap_or(Color::BLACK)
            };
            if let Some(atlas) = sprite.texture_atlas.as_mut() {
                let coord = engine::map::game_map::MapCoord::new(tx as u32, ty as u32);
                let idx = map.get_tile(coord).map(|t| tile_atlas_index(t.kind)).unwrap_or(0);
                atlas.index = idx;
            }
        }
    }

    map_view_state.map_view = Some(map_view_box);
}

fn exit_map_view(
    mut commands: Commands,
    query: Query<Entity, With<MapTile>>,
    root_q: Query<Entity, With<MapViewRoot>>,
    overlay_q: Query<Entity, With<EndTurnOverlay>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in root_q.iter() {
        commands.entity(entity).despawn();
    }
    for entity in overlay_q.iter() {
        commands.entity(entity).despawn();
    }
}
