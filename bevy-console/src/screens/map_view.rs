use crate::input::UiAction;
use crate::screens::AppState;
use crate::screens::team_setup::LoadedSession;
use bevy::prelude::*;
use engine::map::game_map::MapCoord;
use engine::map::tile::Tiles;
use game::input::InputEvent;
use game::map_view::{MapViewApp, MapViewOutcome};
use game::session::GameSession;

#[derive(Component)]
pub struct MapViewRoot;

#[derive(Component)]
pub struct EndTurnOverlay;

#[derive(Component)]
pub struct PauseOverlay;

#[derive(Component)]
pub struct EndTurnConfirmButton;

#[derive(Component)]
pub struct EndTurnCancelButton;

#[derive(Component)]
pub struct PauseResumeButton;

#[derive(Component)]
pub struct PauseQuitButton;

#[derive(Resource)]
pub struct MapViewState {
    pub map_view: Option<Box<MapViewApp>>,
    pub tile_size: f32,
    pub visible_cols: usize,
    pub visible_rows: usize,
    pub needs_initial_draw: bool,
    pub end_turn_overlay: bool,
    pub end_turn_selected: usize,
    pub pause_overlay: bool,
    pub pause_selected: usize,
    pub last_mouse_tile: Option<(usize, usize)>,
}

const TEXT_COLOR: Color = Color::srgb(0.85, 0.85, 0.88);
const FOOTER_COLOR: Color = Color::srgb(0.5, 0.5, 0.55);
const OVERLAY_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.7);
const OVERLAY_PANEL_BG: Color = Color::srgb(0.18, 0.18, 0.24);
const OVERLAY_PANEL_BORDER: Color = Color::srgb(0.5, 0.5, 0.6);

// Button theme (matches splash.rs)
const BTN_BG: Color = Color::srgb(0.14, 0.14, 0.18);
const BTN_BG_HOVER: Color = Color::srgb(0.22, 0.22, 0.28);
const BTN_BG_SELECTED: Color = Color::srgb(0.28, 0.28, 0.35);
const BTN_BG_PRESSED: Color = Color::srgb(0.35, 0.35, 0.42);
const BTN_BORDER: Color = Color::srgb(0.4, 0.4, 0.48);
const BTN_BORDER_HOVER: Color = Color::srgb(0.55, 0.55, 0.62);
const BTN_BORDER_SELECTED: Color = Color::srgb(0.7, 0.7, 0.78);
const BTN_BORDER_PRESSED: Color = Color::srgb(0.65, 0.65, 0.72);

const ATLAS_COLS: u32 = 49;
const ATLAS_ROWS: u32 = 22;
const ATLAS_TILE_W: u32 = 16;
const ATLAS_TILE_H: u32 = 16;

/// Atlas index for the hero sprite silhouette (matches embedded-graphics `tile_atlas_mask(25)`).
const HERO_ATLAS_INDEX: usize = 25;
/// Atlas index for the selected hero sprite (currently same silhouette, different colour).
const SELECTED_HERO_ATLAS_INDEX: usize = 25;

const HERO_COLOR: Color = Color::srgb(1.0, 1.0, 1.0);
const SELECTED_HERO_COLOR: Color = Color::srgb(1.0, 1.0, 0.47);

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
            end_turn_selected: 0,
            pause_overlay: false,
            pause_selected: 0,
            last_mouse_tile: None,
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

/// Common button node style for overlay buttons.
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

/// Atlas index for the cursor overlay sprite.
const CURSOR_ATLAS_INDEX: usize = 624;
const CURSOR_OVERLAY_COLOR: Color = Color::srgb(1.0, 1.0, 0.47);
const CITY_CURSOR_SCALE: f32 = 3.0;
const CITY_CURSOR_Z: f32 = 0.5;

#[derive(Component)]
pub struct CursorOverlay;

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

fn is_city_core_tile(kind: Tiles) -> bool {
    matches!(kind, Tiles::City)
}

fn mouse_visible_tile(
    window: &Window,
    cursor_position: Vec2,
    tile_size: f32,
    visible_cols: usize,
    visible_rows: usize,
) -> Option<(usize, usize)> {
    let total_w = visible_cols as f32 * tile_size;
    let total_h = visible_rows as f32 * tile_size;
    let left = (window.width() - total_w) * 0.5;
    let top = (window.height() - total_h) * 0.5;
    let local_x = cursor_position.x - left;
    let local_y = cursor_position.y - top;

    if local_x < 0.0 || local_y < 0.0 || local_x >= total_w || local_y >= total_h {
        return None;
    }

    Some(((local_x / tile_size) as usize, (local_y / tile_size) as usize))
}

fn enter_map_view_impl(
    mut commands: Commands,
    mut map_view_state: ResMut<MapViewState>,
    mut loaded: Option<ResMut<LoadedSession>>,
    window: Single<&Window>,
    asset_server: Res<AssetServer>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    if map_view_state.map_view.is_none() {
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

        map_view_state.map_view = Some(Box::new(MapViewApp::new(session, 0, 0, None)));
    }

    spawn_map_view_entities(
        &mut commands,
        &mut map_view_state,
        &window,
        &asset_server,
        &mut atlas_layouts,
    );
}

fn spawn_map_view_entities(
    commands: &mut Commands,
    map_view_state: &mut MapViewState,
    window: &Window,
    asset_server: &AssetServer,
    atlas_layouts: &mut Assets<TextureAtlasLayout>,
) {
    let tile_size = map_view_state.tile_size;
    let map_h = window.height() - 40.0;
    let visible_cols = (window.width() / tile_size).max(1.0) as usize;
    let visible_rows = (map_h / tile_size).max(1.0) as usize;

    map_view_state.visible_cols = visible_cols;
    map_view_state.visible_rows = visible_rows;
    map_view_state.needs_initial_draw = true;
    map_view_state.last_mouse_tile = None;

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
                    texture_atlas: Some(TextureAtlas { layout: layout_handle.clone(), index: 0 }),
                    custom_size: Some(Vec2::splat(tile_size)),
                    ..Default::default()
                },
                Transform::from_xyz(x, y, 0.0),
                MapTile,
                MapTilePos { col, row },
            ));
        }
    }

    // Cursor overlay sprite — drawn on top of all tiles at z=1.
    commands.spawn((
        Sprite {
            image: atlas_handle.clone(),
            texture_atlas: Some(TextureAtlas {
                layout: layout_handle.clone(),
                index: CURSOR_ATLAS_INDEX,
            }),
            color: CURSOR_OVERLAY_COLOR,
            custom_size: Some(Vec2::splat(tile_size)),
            ..Default::default()
        },
        Transform::from_xyz(offset_x, offset_y, 1.0),
        CursorOverlay,
    ));

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

fn spawn_end_turn_overlay(commands: &mut Commands, team_name: &str) {
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
                        height: Val::Px(260.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(20.0),
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
                    // Confirm button (selected by default)
                    panel.spawn((
                        Button,
                        EndTurnConfirmButton,
                        button_node(200.0, 50.0),
                        BackgroundColor(BTN_BG_SELECTED),
                        BorderColor::all(BTN_BORDER_SELECTED),
                        children![(
                            Text::new("End Turn"),
                            TextFont { font_size: FontSize::Px(20.0), ..default() },
                            TextColor(TEXT_COLOR),
                        )],
                    ));
                    // Cancel button
                    panel.spawn((
                        Button,
                        EndTurnCancelButton,
                        button_node(200.0, 50.0),
                        BackgroundColor(BTN_BG),
                        BorderColor::all(BTN_BORDER),
                        children![(
                            Text::new("Cancel"),
                            TextFont { font_size: FontSize::Px(20.0), ..default() },
                            TextColor(TEXT_COLOR),
                        )],
                    ));
                    panel.spawn((
                        Text::new("W/S: navigate  Enter: select  Esc: cancel"),
                        TextFont { font_size: FontSize::Px(12.0), ..default() },
                        TextColor(FOOTER_COLOR),
                    ));
                });
        });
}

fn spawn_pause_overlay(commands: &mut Commands) {
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
            PauseOverlay,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        width: Val::Px(400.0),
                        height: Val::Px(300.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(20.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(OVERLAY_PANEL_BG),
                    BorderColor::all(OVERLAY_PANEL_BORDER),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("PAUSED"),
                        TextFont { font_size: FontSize::Px(28.0), ..default() },
                        TextColor(TEXT_COLOR),
                    ));
                    // Resume button (selected by default)
                    panel.spawn((
                        Button,
                        PauseResumeButton,
                        button_node(200.0, 50.0),
                        BackgroundColor(BTN_BG_SELECTED),
                        BorderColor::all(BTN_BORDER_SELECTED),
                        children![(
                            Text::new("Resume"),
                            TextFont { font_size: FontSize::Px(20.0), ..default() },
                            TextColor(TEXT_COLOR),
                        )],
                    ));
                    // Quit button
                    panel.spawn((
                        Button,
                        PauseQuitButton,
                        button_node(200.0, 50.0),
                        BackgroundColor(BTN_BG),
                        BorderColor::all(BTN_BORDER),
                        children![(
                            Text::new("Quit to Menu"),
                            TextFont { font_size: FontSize::Px(20.0), ..default() },
                            TextColor(TEXT_COLOR),
                        )],
                    ));
                    panel.spawn((
                        Text::new("W/S: navigate  Enter: select  Esc: resume"),
                        TextFont { font_size: FontSize::Px(12.0), ..default() },
                        TextColor(FOOTER_COLOR),
                    ));
                });
        });
}

/// Update button visual state based on selection and interaction.
#[allow(clippy::type_complexity)]
fn update_button_style(
    is_selected: bool,
    interaction: &Interaction,
    bg: &mut BackgroundColor,
    border: &mut BorderColor,
) {
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
}

#[allow(clippy::type_complexity)]
fn update_map_view(
    mut commands: Commands,
    mut map_view_state: ResMut<MapViewState>,
    mut next_state: ResMut<NextState<AppState>>,
    mut reader: MessageReader<UiAction>,
    mut status_query: Query<&mut Text>,
    mut tile_query: Query<(&MapTilePos, &mut Sprite), (With<MapTile>, Without<CursorOverlay>)>,
    mut cursor_query: Query<(&mut Transform, &mut Sprite), (With<CursorOverlay>, Without<MapTile>)>,
    window: Single<&Window>,
    end_turn_q: Query<Entity, With<EndTurnOverlay>>,
    pause_q: Query<Entity, With<PauseOverlay>>,
    mut overlay_btns: Query<
        (
            Option<&EndTurnConfirmButton>,
            Option<&EndTurnCancelButton>,
            Option<&PauseResumeButton>,
            Option<&PauseQuitButton>,
            &mut BackgroundColor,
            &mut BorderColor,
            &Interaction,
        ),
        Or<(
            With<EndTurnConfirmButton>,
            With<EndTurnCancelButton>,
            With<PauseResumeButton>,
            With<PauseQuitButton>,
        )>,
    >,
) {
    let visible_cols = map_view_state.visible_cols;
    let visible_rows = map_view_state.visible_rows;

    let Some(mut map_view_box) = map_view_state.map_view.take() else {
        return;
    };
    let map_view = &mut *map_view_box;

    let actions: Vec<UiAction> = reader.read().copied().collect();
    let frame = |action: UiAction| actions.contains(&action);

    // ── Pause overlay input handling ──────────────────────────────────────
    if map_view_state.pause_overlay {
        let selected = map_view_state.pause_selected;

        if frame(UiAction::Up) {
            map_view_state.pause_selected = selected.saturating_sub(1);
        }
        if frame(UiAction::Down) {
            map_view_state.pause_selected = (selected + 1).min(1);
        }

        // Esc always resumes
        if frame(UiAction::Cancel) {
            if let Some(entity) = pause_q.iter().next() {
                commands.entity(entity).despawn();
            }
            map_view_state.pause_overlay = false;
            map_view_state.pause_selected = 0;
            map_view_state.map_view = Some(map_view_box);
            return;
        }

        // Update button styles and handle clicks
        let sel = map_view_state.pause_selected;
        for (et_confirm, et_cancel, resume_opt, quit_opt, mut bg, mut border, interaction) in
            overlay_btns.iter_mut()
        {
            if et_confirm.is_some() || et_cancel.is_some() {
                continue;
            }
            let is_sel = match (resume_opt, quit_opt) {
                (Some(_), None) => sel == 0,
                (None, Some(_)) => sel == 1,
                _ => continue,
            };
            update_button_style(is_sel, &interaction, &mut bg, &mut border);
            let clicked = is_sel && frame(UiAction::Confirm);
            let pressed = matches!(interaction, Interaction::Pressed);
            if clicked || pressed {
                if resume_opt.is_some() {
                    if let Some(entity) = pause_q.iter().next() {
                        commands.entity(entity).despawn();
                    }
                    map_view_state.pause_overlay = false;
                    map_view_state.pause_selected = 0;
                    map_view_state.map_view = Some(map_view_box);
                    return;
                }
                if quit_opt.is_some() {
                    next_state.set(AppState::Splash);
                    map_view_state.map_view = Some(map_view_box);
                    return;
                }
            }
        }

        map_view_state.map_view = Some(map_view_box);
        return;
    }

    // ── End-turn overlay input handling ───────────────────────────────────
    if map_view_state.end_turn_overlay {
        let selected = map_view_state.end_turn_selected;

        if frame(UiAction::Up) {
            map_view_state.end_turn_selected = selected.saturating_sub(1);
        }
        if frame(UiAction::Down) {
            map_view_state.end_turn_selected = (selected + 1).min(1);
        }

        // Esc cancels
        if frame(UiAction::Cancel) {
            if let Some(entity) = end_turn_q.iter().next() {
                commands.entity(entity).despawn();
            }
            map_view_state.end_turn_overlay = false;
            map_view_state.end_turn_selected = 0;
            map_view_state.map_view = Some(map_view_box);
            return;
        }

        // NextTurn action always confirms (fast action)
        if frame(UiAction::NextTurn) {
            if let Some(entity) = end_turn_q.iter().next() {
                commands.entity(entity).despawn();
            }
            map_view_state.end_turn_overlay = false;
            map_view_state.end_turn_selected = 0;
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

        // Update button styles and handle clicks
        let sel = map_view_state.end_turn_selected;
        for (confirm_opt, cancel_opt, p_resume, p_quit, mut bg, mut border, interaction) in
            overlay_btns.iter_mut()
        {
            if p_resume.is_some() || p_quit.is_some() {
                continue;
            }
            let is_sel = match (confirm_opt, cancel_opt) {
                (Some(_), None) => sel == 0,
                (None, Some(_)) => sel == 1,
                _ => continue,
            };
            update_button_style(is_sel, &interaction, &mut bg, &mut border);
            let clicked = is_sel && frame(UiAction::Confirm);
            let button_pressed = matches!(interaction, Interaction::Pressed);
            if clicked || button_pressed {
                if confirm_opt.is_some() {
                    if let Some(entity) = end_turn_q.iter().next() {
                        commands.entity(entity).despawn();
                    }
                    map_view_state.end_turn_overlay = false;
                    map_view_state.end_turn_selected = 0;
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
                if cancel_opt.is_some() {
                    if let Some(entity) = end_turn_q.iter().next() {
                        commands.entity(entity).despawn();
                    }
                    map_view_state.end_turn_overlay = false;
                    map_view_state.end_turn_selected = 0;
                    map_view_state.map_view = Some(map_view_box);
                    return;
                }
            }
        }

        map_view_state.map_view = Some(map_view_box);
        return;
    }

    // ── Normal game input ────────────────────────────────────────────────
    let mut events = Vec::new();
    if frame(UiAction::Up) {
        events.push(InputEvent::Up);
    }
    if frame(UiAction::Down) {
        events.push(InputEvent::Down);
    }
    if frame(UiAction::Left) {
        events.push(InputEvent::Left);
    }
    if frame(UiAction::Right) {
        events.push(InputEvent::Right);
    }
    if frame(UiAction::CursorLeft) {
        events.push(InputEvent::CursorLeft);
    }
    if frame(UiAction::CursorDown) {
        events.push(InputEvent::CursorDown);
    }
    if frame(UiAction::CursorUp) {
        events.push(InputEvent::CursorUp);
    }
    if frame(UiAction::CursorRight) {
        events.push(InputEvent::CursorRight);
    }
    if frame(UiAction::PanUp) {
        events.push(InputEvent::PanUp);
    }
    if frame(UiAction::PanDown) {
        events.push(InputEvent::PanDown);
    }
    if frame(UiAction::PanLeft) {
        events.push(InputEvent::PanLeft);
    }
    if frame(UiAction::PanRight) {
        events.push(InputEvent::PanRight);
    }
    if frame(UiAction::NextHero) {
        events.push(InputEvent::NextHero);
    }
    if frame(UiAction::Confirm) {
        events.push(InputEvent::Enter);
    }

    // KeyQ is not in UiAction, keep raw key check for now
    // (TODO: add Quit/KeyQ to keybindings.toml if desired)
    // No direct KeyCode access here — relies on MessageReader<UiAction>

    // NextTurn triggers end-turn overlay
    if frame(UiAction::NextTurn) {
        events.push(InputEvent::NextTurn);
    }

    // Cancel opens pause overlay instead of going back immediately.
    if frame(UiAction::Cancel) {
        map_view_state.pause_overlay = true;
        map_view_state.pause_selected = 0;
        spawn_pause_overlay(&mut commands);
        map_view_state.map_view = Some(map_view_box);
        return;
    }

    let mut needs_redraw = map_view_state.needs_initial_draw;
    map_view_state.needs_initial_draw = false;
    let mut request_end_turn = false;
    let view_x = map_view.view_x();
    let view_y = map_view.view_y();

    if let Some(cursor_position) = window.cursor_position() {
        if let Some((col, row)) = mouse_visible_tile(
            &window,
            cursor_position,
            map_view_state.tile_size,
            visible_cols,
            visible_rows,
        ) {
            let target_x = view_x + col;
            let target_y = view_y + row;
            let target_tile = (target_x, target_y);
            if map_view_state.last_mouse_tile != Some(target_tile)
                && map_view.set_cursor_from_pointer(target_x, target_y, visible_cols, visible_rows)
            {
                needs_redraw = true;
            }
            map_view_state.last_mouse_tile = Some(target_tile);
        } else {
            map_view_state.last_mouse_tile = None;
        }
    } else {
        map_view_state.last_mouse_tile = None;
    }

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
                if matches!(name.as_str(), "City" | "City Entrance") {
                    next_state.set(AppState::City);
                    map_view_state.map_view = Some(map_view_box);
                    return;
                }
                map_view.set_status(Some(format!("Structure: {}", name)));
                needs_redraw = true;
            }
        }
    }

    if request_end_turn {
        map_view_state.end_turn_overlay = true;
        map_view_state.end_turn_selected = 0;
        let team_name = {
            let session = map_view.session();
            let state = session.state();
            let team = state.get_active_team().ok();
            team.map(|t| t.name.clone()).unwrap_or_else(|| "Unknown".to_string())
        };
        spawn_end_turn_overlay(&mut commands, &team_name);
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
        let selected_hero_id = session.selected_hero_id();

        let city_cursor = if cursor_x >= 0 && cursor_y >= 0 {
            let cursor_coord = MapCoord::new(cursor_x as u32, cursor_y as u32);
            map.get_tile(cursor_coord).map(|tile| is_city_core_tile(tile.kind)).unwrap_or(false)
        } else {
            false
        };

        for (tile_pos, mut sprite) in tile_query.iter_mut() {
            let tx = view_x + tile_pos.col;
            let ty = view_y + tile_pos.row;
            let coord = MapCoord::new(tx as u32, ty as u32);
            sprite.custom_size = Some(Vec2::splat(map_view_state.tile_size));

            // Check for hero on this tile first — hero sprite takes priority.
            if let Some(hero) = session.state().hero_at(coord) {
                let is_selected = hero.get_id() == selected_hero_id;
                sprite.color = if is_selected { SELECTED_HERO_COLOR } else { HERO_COLOR };
                if let Some(atlas) = sprite.texture_atlas.as_mut() {
                    atlas.index =
                        if is_selected { SELECTED_HERO_ATLAS_INDEX } else { HERO_ATLAS_INDEX };
                }
                continue;
            }

            // Tile always shows its own color/atlas — cursor is a separate overlay sprite.
            let tile = map.get_tile(coord).ok();
            sprite.color = tile.map(|t| tile_color_for(t.kind)).unwrap_or(Color::BLACK);
            if let Some(atlas) = sprite.texture_atlas.as_mut() {
                let idx = tile.map(|t| tile_atlas_index(t.kind)).unwrap_or(0);
                atlas.index = idx;
            }
        }

        // Move cursor overlay to the correct tile position.
        let cx = cursor_x - view_x as isize;
        let cy = cursor_y - view_y as isize;
        if cx >= 0 && cy >= 0 && (cx as usize) < visible_cols && (cy as usize) < visible_rows {
            let tile_size = map_view_state.tile_size;
            let total_w = visible_cols as f32 * tile_size;
            let total_h = visible_rows as f32 * tile_size;
            let offset_x = -total_w / 2.0 + tile_size / 2.0;
            let offset_y = total_h / 2.0 - tile_size / 2.0;
            let new_x = offset_x + cx as f32 * tile_size;
            let new_y = offset_y - cy as f32 * tile_size;
            for (mut transform, mut sprite) in cursor_query.iter_mut() {
                transform.translation.x = new_x;
                transform.translation.y = new_y;
                transform.translation.z = CITY_CURSOR_Z;
                sprite.custom_size = Some(Vec2::splat(if city_cursor {
                    tile_size * CITY_CURSOR_SCALE
                } else {
                    tile_size
                }));
            }
        }
    }

    map_view_state.map_view = Some(map_view_box);
}

fn exit_map_view(
    mut commands: Commands,
    query: Query<Entity, With<MapViewRoot>>,
    tile_query: Query<Entity, With<MapTile>>,
    cursor_query: Query<Entity, With<CursorOverlay>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in tile_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in cursor_query.iter() {
        commands.entity(entity).despawn();
    }
}
