use crate::atlas::{TeamLogoImages, TileAtlas};
use crate::frontend::input::InputEvent;
use crate::frontend::map_view::{MapViewApp, MapViewOutcome};
use crate::frontend::session::GameSession;
use crate::input::{InputCooldown, UiAction};
use crate::screens::AppState;
use crate::screens::team_setup::LoadedSession;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use engine::MapCoord;
use engine::config::{TeamLogo, get_team_catalog, get_tile_config};
use engine::game_state::GameState;
use engine::map::game_map::{RESOURCE_KIND_COUNT, ResourceKind};
use engine::map::tile::Tiles;

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

/// Marker for the bottom status-bar text (scopes the status update query).
#[derive(Component)]
pub struct StatusText;

/// Marker for the top resource/turn HUD bar root.
#[derive(Component)]
pub struct TopBarRoot;

/// Identifies which treasury value a top-bar text entity displays.
#[derive(Component, Clone, Copy)]
pub enum TopBarField {
    /// Current team turn number.
    Turn,
    /// Gold balance.
    Gold,
    /// Stockpile of the resource at this index (0–3).
    Resource(usize),
}

/// Tint for gold values in the HUD.
const GOLD_COLOR: Color = Color::srgb(0.96, 0.82, 0.30);

/// Atlas index of the gold pictogram, sourced from `tiles.yaml` (`gold` tile).
fn gold_icon_index() -> usize {
    get_tile_config().atlas_index("gold").unwrap_or(0) as usize
}

/// Atlas indices of the four resource pictograms, sourced from `tiles.yaml`
/// (`resource` tile variants, in declaration order).
fn resource_icon_indices() -> [usize; RESOURCE_KIND_COUNT] {
    let mut icons = [0usize; RESOURCE_KIND_COUNT];
    if let Some(indexes) = get_tile_config().atlas_indexes("resource") {
        for (slot, index) in icons.iter_mut().zip(indexes) {
            *slot = index as usize;
        }
    }
    icons
}

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
    /// Handle to the tile atlas image (1_main.png).
    pub atlas_image: Handle<Image>,
    /// Handle to the tile atlas layout.
    pub atlas_layout: Handle<TextureAtlasLayout>,
}

impl MapViewState {
    pub fn get_game_state(&self) -> Option<&GameState> {
        self.map_view.as_ref().map(|mv| mv.session().state())
    }

    pub fn get_game_state_mut(&mut self) -> Option<&mut GameState> {
        self.map_view.as_mut().map(|mv| mv.session_mut().state_mut())
    }

    pub fn cursor_coord(&self) -> Option<MapCoord> {
        self.map_view.as_ref().and_then(|mv| mv.cursor_coord())
    }
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

const RESOURCE_ROD_ATLAS_INDEX: usize = 344;

/// Fallback color for heroes whose team is not found.
const HERO_COLOR: Color = Color::srgb(1.0, 1.0, 1.0);
const NEUTRAL_RESOURCE_COLOR: Color = Color::srgb(1.0, 1.0, 1.0);

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
            atlas_image: Handle::default(),
            atlas_layout: Handle::default(),
        }
    }
}

/// Color used for tiles of a neutral (unowned) city.
const NEUTRAL_CITY_COLOR: Color = Color::srgb(1.0, 0.0, 1.0); // magenta

fn tile_color_for(kind: Tiles) -> Color {
    let (r, g, b) = kind.as_color();
    rgb_color(r, g, b)
}

fn rgb_color(r: u8, g: u8, b: u8) -> Color {
    Color::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

/// City tiles (the city body and its entrance) are tinted by ownership:
/// the owning team's color, or magenta when neutral.
fn is_city_tile(kind: Tiles) -> bool {
    matches!(kind, Tiles::City | Tiles::CityEntrance)
}

fn tile_atlas_index(kind: Tiles) -> usize {
    kind.atlas_index() as usize
}

fn resource_atlas_index(kind: ResourceKind) -> usize {
    match kind {
        ResourceKind::Resource1 => 1089,
        ResourceKind::Resource2 => 1092,
        ResourceKind::Resource3 => 1093,
        ResourceKind::Resource4 => 1094,
        ResourceKind::GoldMine => 1091,
    }
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
pub struct LandOwnerTile;

#[derive(Component)]
pub struct ResourceRodTile;

/// Overlay sprite that draws the owning team's logo on a city core tile.
#[derive(Component)]
pub struct CityLogoTile;

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
    atlas: Res<TileAtlas>,
) {
    enter_map_view_impl(commands, map_view_state, loaded, window, atlas);
}

fn is_city_core_tile(kind: Tiles) -> bool {
    matches!(kind, Tiles::City)
}

/// Returns the single centre cell of every owned city.
///
/// `set_city_owner` floods the whole connected city block, so ownership covers
/// every city tile. To draw a team logo on just one cell, we flood each city
/// once and pick the core (`City`) cell nearest the block's centroid.
fn owned_city_centers(
    state: &engine::game_state::GameState,
) -> std::collections::HashSet<MapCoord> {
    use std::collections::HashSet;
    let map = &state.map;
    let mut visited: HashSet<MapCoord> = HashSet::new();
    let mut centers: HashSet<MapCoord> = HashSet::new();
    for &coord in state.city_owners.keys() {
        if !visited.insert(coord) {
            continue;
        }
        let cells = engine::state_flood::flood_city(map, coord);
        for &c in &cells {
            visited.insert(c);
        }
        let cores: Vec<MapCoord> = cells
            .iter()
            .copied()
            .filter(|c| map.get_tile(*c).map(|t| is_city_core_tile(t.kind)).unwrap_or(false))
            .collect();
        if cores.is_empty() {
            continue;
        }
        let cx = cores.iter().map(|c| c.x as u64).sum::<u64>() / cores.len() as u64;
        let cy = cores.iter().map(|c| c.y as u64).sum::<u64>() / cores.len() as u64;
        if let Some(center) = cores.iter().copied().min_by_key(|c| {
            let dx = c.x as i64 - cx as i64;
            let dy = c.y as i64 - cy as i64;
            dx * dx + dy * dy
        }) {
            // Drop the logo one cell below the centroid when that cell is still
            // part of the city — it reads better than sitting at the top.
            let below = MapCoord::new(center.x, center.y + 1);
            let center = if cores.contains(&below) { below } else { center };
            centers.insert(center);
        }
    }
    centers
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
    atlas: Res<TileAtlas>,
) {
    if map_view_state.map_view.is_none() {
        let Some(mut loaded) = loaded.take() else {
            return;
        };

        let Some(state) = loaded.state.take() else {
            return;
        };

        let session = GameSession::from_state(state);

        map_view_state.map_view = Some(Box::new(MapViewApp::new(session, 0, 0, None)));
    }

    spawn_map_view_entities(&mut commands, &mut map_view_state, &window, &atlas);

    // Center camera on the selected hero on first entry.
    // Must run after spawn_map_view_entities which computes visible_cols/rows.
    let vc = map_view_state.visible_cols;
    let vr = map_view_state.visible_rows;
    if let Some(ref mut mv) = map_view_state.map_view {
        mv.sync_cursor_to_hero();
        mv.center_on_hero(vc, vr);
    }
}

fn spawn_map_view_entities(
    commands: &mut Commands,
    map_view_state: &mut MapViewState,
    window: &Window,
    atlas: &TileAtlas,
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

    let atlas_handle = atlas.image.clone();
    let layout_handle = atlas.layout.clone();

    // Store handles in MapViewState so other systems (e.g. hero selection)
    // can reuse the same atlas.
    map_view_state.atlas_image = atlas_handle.clone();
    map_view_state.atlas_layout = layout_handle.clone();

    for row in 0..visible_rows {
        for col in 0..visible_cols {
            let x = offset_x + col as f32 * tile_size;
            let y = offset_y - row as f32 * tile_size;
            commands.spawn((
                Sprite {
                    color: Color::NONE,
                    custom_size: Some(Vec2::splat(tile_size)),
                    ..Default::default()
                },
                Transform::from_xyz(x, y, -0.1),
                LandOwnerTile,
                MapTilePos { col, row },
            ));

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

            commands.spawn((
                Sprite {
                    image: atlas_handle.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: layout_handle.clone(),
                        index: RESOURCE_ROD_ATLAS_INDEX,
                    }),
                    color: Color::NONE,
                    custom_size: Some(Vec2::splat(tile_size)),
                    ..Default::default()
                },
                Transform::from_xyz(x, y, 0.2),
                ResourceRodTile,
                MapTilePos { col, row },
            ));

            commands.spawn((
                Sprite {
                    image: atlas_handle.clone(),
                    texture_atlas: Some(TextureAtlas { layout: layout_handle.clone(), index: 0 }),
                    color: Color::NONE,
                    custom_size: Some(Vec2::splat(tile_size)),
                    ..Default::default()
                },
                Transform::from_xyz(x, y, 0.25),
                CityLogoTile,
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
                StatusText,
            ));
        });

    spawn_top_bar(commands, atlas_handle, layout_handle);
}

/// Spawns the top HUD bar: current turn number, gold balance, and the four
/// resource stockpiles, each labelled with its atlas pictogram. The text
/// entities are tagged with [`TopBarField`] so [`update_map_view`] can refresh
/// their values every redraw.
fn spawn_top_bar(
    commands: &mut Commands,
    atlas_image: Handle<Image>,
    atlas_layout: Handle<TextureAtlasLayout>,
) {
    // One labelled cell: optional pictogram + value text tagged with `field`.
    fn cell(
        parent: &mut ChildSpawnerCommands,
        atlas_image: &Handle<Image>,
        atlas_layout: &Handle<TextureAtlasLayout>,
        icon: Option<usize>,
        field: TopBarField,
        color: Color,
    ) {
        parent
            .spawn((Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },))
            .with_children(|cell| {
                if let Some(index) = icon {
                    cell.spawn((
                        ImageNode {
                            image: atlas_image.clone(),
                            texture_atlas: Some(TextureAtlas {
                                layout: atlas_layout.clone(),
                                index,
                            }),
                            ..default()
                        },
                        Node { width: Val::Px(18.0), height: Val::Px(18.0), ..default() },
                    ));
                }
                cell.spawn((
                    Text::new("0"),
                    TextFont { font_size: FontSize::Px(15.0), ..default() },
                    TextColor(color),
                    field,
                ));
            });
    }

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(32.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                column_gap: Val::Px(18.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.14)),
            TopBarRoot,
        ))
        .with_children(|parent| {
            let resource_icons = resource_icon_indices();
            // Turn number (no icon, plain text label baked into the value).
            cell(parent, &atlas_image, &atlas_layout, None, TopBarField::Turn, TEXT_COLOR);
            // Gold balance.
            cell(
                parent,
                &atlas_image,
                &atlas_layout,
                Some(gold_icon_index()),
                TopBarField::Gold,
                GOLD_COLOR,
            );
            // Four resource stockpiles.
            for (idx, &icon) in resource_icons.iter().enumerate() {
                cell(
                    parent,
                    &atlas_image,
                    &atlas_layout,
                    Some(icon),
                    TopBarField::Resource(idx),
                    TEXT_COLOR,
                );
            }
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

/// Per-tile sprite layers plus the assets needed to draw team logos, bundled to
/// keep [`update_map_view`] under the system parameter limit.
#[derive(SystemParam)]
struct TileLayers<'w, 's> {
    tiles: Query<
        'w,
        's,
        (&'static MapTilePos, &'static mut Sprite),
        (
            With<MapTile>,
            Without<LandOwnerTile>,
            Without<ResourceRodTile>,
            Without<CityLogoTile>,
            Without<CursorOverlay>,
        ),
    >,
    land: Query<
        'w,
        's,
        (&'static MapTilePos, &'static mut Sprite),
        (
            With<LandOwnerTile>,
            Without<MapTile>,
            Without<ResourceRodTile>,
            Without<CityLogoTile>,
            Without<CursorOverlay>,
        ),
    >,
    rod: Query<
        'w,
        's,
        (&'static MapTilePos, &'static mut Sprite),
        (
            With<ResourceRodTile>,
            Without<MapTile>,
            Without<LandOwnerTile>,
            Without<CityLogoTile>,
            Without<CursorOverlay>,
        ),
    >,
    logo: Query<
        'w,
        's,
        (&'static MapTilePos, &'static mut Sprite),
        (
            With<CityLogoTile>,
            Without<MapTile>,
            Without<LandOwnerTile>,
            Without<ResourceRodTile>,
            Without<CursorOverlay>,
        ),
    >,
    atlas: Res<'w, TileAtlas>,
    logo_images: ResMut<'w, TeamLogoImages>,
    images: ResMut<'w, Assets<Image>>,
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
fn update_map_view(
    mut commands: Commands,
    mut map_view_state: ResMut<MapViewState>,
    mut next_state: ResMut<NextState<AppState>>,
    mut reader: MessageReader<UiAction>,
    mut status_query: Query<&mut Text, (With<StatusText>, Without<TopBarField>)>,
    mut top_bar_query: Query<(&mut Text, &TopBarField), Without<StatusText>>,
    mut layers: TileLayers,
    mut cursor_query: Query<
        (&mut Transform, &mut Sprite),
        (
            With<CursorOverlay>,
            Without<MapTile>,
            Without<LandOwnerTile>,
            Without<ResourceRodTile>,
            Without<CityLogoTile>,
        ),
    >,
    cooldown: Res<InputCooldown>,
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
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

        if frame(UiAction::Up) || frame(UiAction::CursorUp) {
            map_view_state.pause_selected = selected.saturating_sub(1);
        }
        if frame(UiAction::Down) || frame(UiAction::CursorDown) {
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
            update_button_style(is_sel, interaction, &mut bg, &mut border);
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
                    if let Some(entity) = pause_q.iter().next() {
                        commands.entity(entity).despawn();
                    }
                    map_view_state.pause_overlay = false;
                    map_view_state.pause_selected = 0;
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

        if frame(UiAction::Up) || frame(UiAction::CursorUp) {
            map_view_state.end_turn_selected = selected.saturating_sub(1);
        }
        if frame(UiAction::Down) || frame(UiAction::CursorDown) {
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
            map_view.sync_cursor_to_hero();
            map_view.center_on_hero(visible_cols, visible_rows);
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
            update_button_style(is_sel, interaction, &mut bg, &mut border);
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
                    map_view.sync_cursor_to_hero();
                    map_view.center_on_hero(visible_cols, visible_rows);
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
    if frame(UiAction::PlaceRod) {
        events.push(InputEvent::PlaceRod);
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

            // Mouse click on the tile under the cursor acts as Enter,
            // but only if the input cooldown has elapsed (prevents stale clicks
            // after a state transition from bleeding through).
            if mouse_buttons.just_pressed(MouseButton::Left)
                && !cooldown.is_cooling_down(time.elapsed_secs_f64())
            {
                events.push(InputEvent::Enter);
            }
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
            MapViewOutcome::RequestEndTurn => {
                request_end_turn = true;
            }
            MapViewOutcome::OpenStructureOverlay { name } => match name.as_str() {
                "City" | "City Entrance" => {
                    // Find the nearest CityEntrance tile within the city structure
                    // so we can show the hire-hero screen.
                    if let Some(entrance) = map_view.find_city_entrance_at_cursor() {
                        map_view.set_cursor_pos(entrance.x, entrance.y);
                    }
                    next_state.set(AppState::CityEntrance);
                    map_view_state.map_view = Some(map_view_box);
                    return;
                }
                _ => {
                    map_view.set_status(Some(format!("Structure: {}", name)));
                    needs_redraw = true;
                }
            },
            MapViewOutcome::OpenHeroInfo => {
                next_state.set(AppState::Hero);
                map_view_state.map_view = Some(map_view_box);
                return;
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
        let city_centers = owned_city_centers(session.state());
        let view_x = map_view.view_x();
        let view_y = map_view.view_y();
        let cursor_x = map_view.cursor_x();
        let cursor_y = map_view.cursor_y();
        let selected_hero_id = session.selected_hero_id();

        // Refresh the top HUD bar (turn number, gold, resources) for the active team.
        if let Ok(team) = session.state().get_active_team() {
            let turn = team.get_turn();
            let gold = team.gold();
            let resources = team.resources();
            for (mut text, field) in top_bar_query.iter_mut() {
                text.0 = match *field {
                    TopBarField::Turn => format!("Turn {turn}"),
                    TopBarField::Gold => format!("{gold}"),
                    TopBarField::Resource(idx) => {
                        format!("{}", resources.get(idx).copied().unwrap_or(0))
                    }
                };
            }
        }

        let city_cursor = if cursor_x >= 0 && cursor_y >= 0 {
            let cursor_coord = MapCoord::new(cursor_x as u32, cursor_y as u32);
            map.get_tile(cursor_coord).map(|tile| is_city_core_tile(tile.kind)).unwrap_or(false)
        } else {
            false
        };

        for (tile_pos, mut sprite) in layers.tiles.iter_mut() {
            let tx = view_x + tile_pos.col;
            let ty = view_y + tile_pos.row;
            let coord = MapCoord::new(tx as u32, ty as u32);
            sprite.custom_size = Some(Vec2::splat(map_view_state.tile_size));

            // Check for hero on this tile first — hero sprite takes priority.
            if let Some(hero) = session.state().hero_at(&coord) {
                let is_selected = selected_hero_id == Some(hero.get_id());
                // Color: team color for selected, 50% alpha team color for others.
                let team_color = session
                    .state()
                    .get_team(hero.get_team_id())
                    .map(|t| {
                        let (r, g, b) = t.color;
                        Color::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
                    })
                    .unwrap_or(HERO_COLOR);
                sprite.color = if is_selected { team_color } else { team_color.with_alpha(0.5) };
                if let Some(atlas) = sprite.texture_atlas.as_mut() {
                    atlas.index = hero.get_atlas_index();
                }
                continue;
            }

            // Tile always shows its own color/atlas — cursor is a separate overlay sprite.
            let tile = map.get_tile(coord).ok();
            let resource_node = map.resource_node_at(coord);
            // A resource point is anything the engine treats as ownable via a
            // rod: an explicit resource node, or a bare Gold/Resource tile (e.g.
            // mines loaded from a Tiled map that carries no resource nodes).
            // Tint it by its owner so a captured mine matches its claimed land.
            let is_resource_point = resource_node.is_some()
                || tile.map(|t| matches!(t.kind, Tiles::Gold | Tiles::Resource)).unwrap_or(false);
            sprite.color = match (tile, is_resource_point) {
                (_, true) => match session.state().resource_owner(coord) {
                    Some(team_id) => session
                        .state()
                        .get_team(team_id)
                        .map(|team| {
                            let (r, g, b) = team.color;
                            rgb_color(r, g, b)
                        })
                        .unwrap_or(NEUTRAL_RESOURCE_COLOR),
                    None => NEUTRAL_RESOURCE_COLOR,
                },
                (Some(t), false) if is_city_tile(t.kind) => {
                    match session.state().city_owner(&coord) {
                        // The owned city centre cell is hidden so the team logo
                        // overlay (CityLogoTile layer) takes its place; the rest of
                        // the city keeps its owner-tinted castle tiles.
                        Some(_) if city_centers.contains(&coord) => Color::NONE,
                        Some(team_id) => session
                            .state()
                            .get_team(team_id)
                            .map(|team| {
                                let (r, g, b) = team.color;
                                rgb_color(r, g, b)
                            })
                            .unwrap_or(NEUTRAL_CITY_COLOR),
                        None => NEUTRAL_CITY_COLOR,
                    }
                }
                (Some(t), false) => tile_color_for(t.kind),
                (None, false) => Color::BLACK,
            };
            if let Some(atlas) = sprite.texture_atlas.as_mut() {
                let idx = resource_node
                    .map(|node| resource_atlas_index(node.kind))
                    .or_else(|| tile.map(|t| tile_atlas_index(t.kind)))
                    .unwrap_or(0);
                atlas.index = idx;
            }
        }

        for (tile_pos, mut sprite) in layers.land.iter_mut() {
            let tx = view_x + tile_pos.col;
            let ty = view_y + tile_pos.row;
            let coord = MapCoord::new(tx as u32, ty as u32);
            sprite.custom_size = Some(Vec2::splat(map_view_state.tile_size));
            sprite.color = session
                .state()
                .land_owner(coord)
                .and_then(|team_id| session.state().get_team(team_id))
                .map(|team| {
                    let (r, g, b) = team.color;
                    rgb_color(r, g, b).with_alpha(0.35)
                })
                .unwrap_or(Color::NONE);
        }

        for (tile_pos, mut sprite) in layers.rod.iter_mut() {
            let tx = view_x + tile_pos.col;
            let ty = view_y + tile_pos.row;
            let coord = MapCoord::new(tx as u32, ty as u32);
            sprite.custom_size = Some(Vec2::splat(map_view_state.tile_size));
            sprite.color = session
                .state()
                .resource_rod_owner(coord)
                .and_then(|team_id| session.state().get_team(team_id))
                .map(|team| {
                    let (r, g, b) = team.color;
                    rgb_color(r, g, b)
                })
                .unwrap_or(Color::NONE);
            if let Some(atlas) = sprite.texture_atlas.as_mut() {
                atlas.index = RESOURCE_ROD_ATLAS_INDEX;
            }
        }

        // City logo overlay: draw the owning team's logo on its city core tile.
        let catalog = get_team_catalog();
        for (tile_pos, mut sprite) in layers.logo.iter_mut() {
            let tx = view_x + tile_pos.col;
            let ty = view_y + tile_pos.row;
            let coord = MapCoord::new(tx as u32, ty as u32);
            sprite.custom_size = Some(Vec2::splat(map_view_state.tile_size));

            // Resolve a logo only for a city centre cell with no hero on it.
            let is_center = city_centers.contains(&coord);
            let resolved = if is_center && session.state().hero_at(&coord).is_none() {
                session
                    .state()
                    .city_owner(&coord)
                    .and_then(|team_id| session.state().get_team(team_id))
                    .and_then(|team| {
                        catalog
                            .as_ref()
                            .and_then(|cat| cat.by_name(&team.name))
                            .map(|def| (def.logo.clone(), team.name.clone(), team.color))
                    })
            } else {
                None
            };

            match resolved {
                Some((logo, name, (r, g, b))) => {
                    let tint = rgb_color(r, g, b);
                    match &logo {
                        TeamLogo::Tile(index) => {
                            sprite.image = layers.atlas.image.clone();
                            sprite.texture_atlas = Some(TextureAtlas {
                                layout: layers.atlas.layout.clone(),
                                index: *index as usize,
                            });
                            sprite.color = tint;
                        }
                        TeamLogo::Bitmap(_) => {
                            match layers.logo_images.handle(&mut layers.images, &name, &logo) {
                                Some(handle) => {
                                    sprite.image = handle;
                                    sprite.texture_atlas = None;
                                    sprite.color = tint;
                                }
                                None => sprite.color = Color::NONE,
                            }
                        }
                    }
                }
                None => sprite.color = Color::NONE,
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
    top_bar_query: Query<Entity, With<TopBarRoot>>,
    tile_query: Query<Entity, With<MapTile>>,
    land_query: Query<Entity, With<LandOwnerTile>>,
    rod_query: Query<Entity, With<ResourceRodTile>>,
    logo_query: Query<Entity, With<CityLogoTile>>,
    cursor_query: Query<Entity, With<CursorOverlay>>,
    pause_query: Query<Entity, With<PauseOverlay>>,
    end_turn_query: Query<Entity, With<EndTurnOverlay>>,
    mut map_view_state: ResMut<MapViewState>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in top_bar_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in tile_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in land_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in rod_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in logo_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in cursor_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in pause_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in end_turn_query.iter() {
        commands.entity(entity).despawn();
    }
    map_view_state.pause_overlay = false;
    map_view_state.end_turn_overlay = false;
}
