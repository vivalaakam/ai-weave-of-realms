use crate::input::UiAction;
use crate::screens::AppState;
use crate::screens::map_view::MapViewState;
use bevy::prelude::*;
use engine::hero::Hero;
use engine::hero_class::HeroClass;

const TEXT_COLOR: Color = Color::srgb(0.92, 0.92, 0.88);
const SUBTLE_COLOR: Color = Color::srgb(0.72, 0.72, 0.76);
const FOOTER_COLOR: Color = Color::srgb(0.5, 0.5, 0.55);
const BG_COLOR: Color = Color::srgb(0.08, 0.08, 0.12);

const BTN_BG_HOVER: Color = Color::srgb(0.22, 0.22, 0.28);
const BTN_BG_SELECTED: Color = Color::srgb(0.28, 0.28, 0.35);
const BTN_BG_PRESSED: Color = Color::srgb(0.35, 0.35, 0.42);
const BTN_BORDER_HOVER: Color = Color::srgb(0.55, 0.55, 0.62);
const BTN_BORDER_SELECTED: Color = Color::srgb(0.7, 0.7, 0.78);
const BTN_BORDER_PRESSED: Color = Color::srgb(0.65, 0.65, 0.72);

/// How many hero-class cards per row in the selection grid.
const GRID_COLS: usize = 4;
/// Maximum visible rows before scrolling kicks in.
const VISIBLE_ROWS: usize = 4;
const CARD_H: f32 = 110.0;
const GAP: f32 = 8.0;

#[derive(Component)]
pub struct CityEntranceRoot;

#[derive(Component)]
struct HireHeroButton;

/// Marker for the hero-class selection overlay root.
#[derive(Component)]
struct HeroSelectRoot;

/// Marker for the hero-class selection grid container (for scrolling).
#[derive(Component)]
struct HeroSelectGrid;

/// Scrolling/focus state for the hero selection grid.
#[derive(Resource, Default)]
struct HeroSelectScroll {
    /// Currently focused card index (keyboard/gamepad navigation).
    focus: usize,
    /// Vertical scroll offset in pixels (shifts the grid up to reveal lower rows).
    scroll_y: f32,
}

fn enter_city_entrance(mut commands: Commands, map_view_state: Res<MapViewState>) {
    build_city_entrance(&mut commands, &map_view_state);
}

pub struct CityEntrancePlugin;

impl Plugin for CityEntrancePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::CityEntrance), enter_city_entrance)
            .add_systems(OnExit(AppState::CityEntrance), exit_city_entrance)
            .add_systems(Update, update_city_entrance.run_if(in_state(AppState::CityEntrance)));
    }
}

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

/// Decide what to show based on whether the entrance is occupied
/// and whether the player's team owns the city.
enum EntranceInfo {
    Occupied { name: String },
    CanHire,
    NoOwnership,
}

fn get_entrance_info(map_view_state: &MapViewState) -> Option<EntranceInfo> {
    let map_view = map_view_state.map_view.as_ref()?;
    let coord = map_view.cursor_coord()?;
    let state = map_view.session().state();

    if let Some(hero) = state.hero_at(coord) {
        return Some(EntranceInfo::Occupied { name: hero.name.clone() });
    }

    let active_team_id = state.get_active_team_id().copied().ok()?;
    let owner = state.city_owner(coord);

    if owner == Some(active_team_id) {
        Some(EntranceInfo::CanHire)
    } else {
        Some(EntranceInfo::NoOwnership)
    }
}

fn build_city_entrance(commands: &mut Commands, map_view_state: &MapViewState) {
    let info = get_entrance_info(map_view_state);

    let (hero_text, show_hire_button, footer_text) = match info {
        Some(EntranceInfo::Occupied { name }) => {
            (name, false, "Enter/Cross: select   Esc/Circle: back")
        }
        Some(EntranceInfo::CanHire) => (
            "No hero stationed here".to_string(),
            true,
            "Enter/Cross: hire hero   Esc/Circle: back",
        ),
        Some(EntranceInfo::NoOwnership) => (
            "No hero stationed here\n(You don't own this city)".to_string(),
            false,
            "Esc/Circle: back",
        ),
        None => ("Unknown".to_string(), false, "Esc/Circle: back"),
    };

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(18.0),
                ..default()
            },
            BackgroundColor(BG_COLOR),
            CityEntranceRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Hero Squad"),
                TextFont { font_size: FontSize::Px(42.0), ..default() },
                TextColor(TEXT_COLOR),
            ));

            parent.spawn((
                Text::new(hero_text),
                TextFont { font_size: FontSize::Px(20.0), ..default() },
                TextColor(SUBTLE_COLOR),
            ));

            if show_hire_button {
                parent.spawn((
                    Button,
                    HireHeroButton,
                    button_node(220.0, 50.0),
                    BackgroundColor(BTN_BG_SELECTED),
                    BorderColor::all(BTN_BORDER_SELECTED),
                    children![(
                        Text::new("Hire Hero"),
                        TextFont { font_size: FontSize::Px(20.0), ..default() },
                        TextColor(TEXT_COLOR),
                    )],
                ));
            }

            parent.spawn((
                Text::new(footer_text),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(FOOTER_COLOR),
            ));
        });
}

// ─── Hero class selection overlay ─────────────────────────────────────────

fn spawn_hero_select(
    commands: &mut Commands,
    atlas_image: &Handle<Image>,
    atlas_layout: &Handle<TextureAtlasLayout>,
) {
    let classes = HeroClass::all();
    let total = classes.len();
    let total_rows = total.div_ceil(GRID_COLS);

    commands.insert_resource(HeroSelectScroll { focus: 0, scroll_y: 0.0 });

    // Card dimensions: sprite on the left, text on the right.
    let sprite_size = 96.0;
    let card_w = 260.0;
    let card_h = 110.0;
    let gap = 8.0;
    let grid_w = card_w * GRID_COLS as f32 + gap * (GRID_COLS - 1) as f32;

    // Modal overlay.
    let overlay = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
            GlobalZIndex(100),
            HeroSelectRoot,
        ))
        .id();

    // Panel.
    let panel = commands
        .spawn((
            Node {
                width: Val::Px(grid_w + 32.0),
                max_height: Val::Percent(85.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.12, 0.18)),
            BorderColor::all(Color::srgb(0.4, 0.4, 0.5)),
        ))
        .id();

    // Title.
    let title = commands
        .spawn((
            Text::new("Choose Your Hero"),
            TextFont { font_size: FontSize::Px(32.0), ..default() },
            TextColor(TEXT_COLOR),
        ))
        .id();

    // Scroll wrapper — clips content and provides a viewport for the grid.
    let scroll_wrapper = commands
        .spawn((Node {
            flex_direction: FlexDirection::Column,
            overflow: Overflow::clip(),
            flex_grow: 1.0,
            width: Val::Percent(100.0),
            ..default()
        },))
        .id();

    // Grid area (scrollable container of rows).
    let grid = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(gap),
                width: Val::Percent(100.0),
                ..default()
            },
            HeroSelectGrid,
        ))
        .id();

    // Spawn rows of cards.
    for row_idx in 0..total_rows {
        let row_entity = commands
            .spawn((Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(gap),
                ..default()
            },))
            .id();
        commands.entity(grid).add_child(row_entity);

        for col in 0..GRID_COLS {
            let idx = row_idx * GRID_COLS + col;
            if idx >= total {
                break;
            }
            let class = classes[idx];

            let stats = format!(
                "HP:{} ATK:{} DEF:{}\nSPD:{} MOV:{}",
                class.base_hp(),
                class.base_atk(),
                class.base_def(),
                class.base_spd(),
                Hero::movement_for_spd(class.base_spd()),
            );

            let is_focused = idx == 0;
            let (bg, border) = if is_focused {
                (BTN_BG_SELECTED, BTN_BORDER_SELECTED)
            } else {
                (BTN_BG_HOVER, BTN_BORDER_HOVER)
            };

            // Card: sprite on the left, name + stats on the right.
            let atlas_index = class.atlas_index();
            let card = commands
                .spawn((
                    Button,
                    HeroClassCard { class },
                    Node {
                        width: Val::Px(card_w),
                        height: Val::Px(card_h),
                        justify_content: JustifyContent::FlexStart,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        padding: UiRect::all(Val::Px(4.0)),
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                    BackgroundColor(bg),
                    BorderColor::all(border),
                ))
                .with_children(|card_node| {
                    // Hero sprite (atlas tile scaled to sprite_size).
                    card_node.spawn((
                        ImageNode {
                            image: atlas_image.clone(),
                            texture_atlas: Some(TextureAtlas {
                                layout: atlas_layout.clone(),
                                index: atlas_index,
                            }),
                            ..default()
                        },
                        Node {
                            width: Val::Px(sprite_size),
                            height: Val::Px(sprite_size),
                            ..default()
                        },
                    ));

                    // Class name + stats stacked vertically.
                    card_node
                        .spawn((Node {
                            flex_direction: FlexDirection::Column,
                            flex_grow: 1.0,
                            ..default()
                        },))
                        .with_children(|text_col| {
                            text_col.spawn((
                                Text::new(class.display_name()),
                                TextFont { font_size: FontSize::Px(14.0), ..default() },
                                TextColor(TEXT_COLOR),
                            ));
                            text_col.spawn((
                                Text::new(stats),
                                TextFont { font_size: FontSize::Px(12.0), ..default() },
                                TextColor(SUBTLE_COLOR),
                            ));
                        });
                })
                .id();

            commands.entity(row_entity).add_child(card);
        }
    }

    // Footer.
    let footer = commands
        .spawn((
            Text::new("Arrows: Navigate   Enter: Confirm   Esc: Back"),
            TextFont { font_size: FontSize::Px(14.0), ..default() },
            TextColor(FOOTER_COLOR),
        ))
        .id();

    commands.entity(panel).add_child(title);
    commands.entity(scroll_wrapper).add_child(grid);
    commands.entity(panel).add_child(scroll_wrapper);
    commands.entity(panel).add_child(footer);
    commands.entity(overlay).add_child(panel);
}

#[derive(Component)]
struct HeroClassCard {
    class: HeroClass,
}

#[allow(clippy::type_complexity)]
fn update_city_entrance(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    mut map_view_state: ResMut<MapViewState>,
    mut reader: MessageReader<UiAction>,
    // "Hire Hero" button — only present on the entrance screen (no overlay).
    mut hire_btns: Query<
        (&mut BackgroundColor, &mut BorderColor, &Interaction),
        (With<HireHeroButton>, Without<HeroClassCard>),
    >,
    select_root: Query<Entity, With<HeroSelectRoot>>,
    // Hero class cards — only present in the selection overlay.
    mut cards: Query<
        (&HeroClassCard, &mut BackgroundColor, &mut BorderColor, &Interaction),
        Without<HireHeroButton>,
    >,
    mut scroll: Option<ResMut<HeroSelectScroll>>,
    mut grid_node: Query<&mut Node, With<HeroSelectGrid>>,
    entrance_roots: Query<Entity, With<CityEntranceRoot>>,
) {
    // If no hero-selection overlay is active and scroll resource is missing,
    // we're in the entrance screen mode. Early-out if scroll is None and
    // there's no overlay either.
    let actions: Vec<UiAction> = reader.read().copied().collect();
    let has = |action: UiAction| actions.contains(&action);

    // ── Hero selection overlay mode ────────────────────────────────
    if !select_root.is_empty() {
        let Some(scroll) = scroll.as_mut() else {
            return;
        };
        let total = HeroClass::all().len();

        // Navigation within the selection grid.
        if (has(UiAction::CursorUp) || has(UiAction::Up)) && scroll.focus >= GRID_COLS {
            scroll.focus -= GRID_COLS;
        }
        if (has(UiAction::CursorDown) || has(UiAction::Down)) && scroll.focus + GRID_COLS < total {
            scroll.focus += GRID_COLS;
        }
        if (has(UiAction::CursorLeft) || has(UiAction::Left))
            && !scroll.focus.is_multiple_of(GRID_COLS)
        {
            scroll.focus -= 1;
        }
        if (has(UiAction::CursorRight) || has(UiAction::Right))
            && scroll.focus % GRID_COLS < GRID_COLS - 1
            && scroll.focus + 1 < total
        {
            scroll.focus += 1;
        }

        // Update card highlights based on focus.
        for (card, mut bg, mut border, _interaction) in cards.iter_mut() {
            let is_focused = scroll.focus == card.class as usize;
            if is_focused {
                *bg = BackgroundColor(BTN_BG_SELECTED);
                *border = BorderColor::all(BTN_BORDER_SELECTED);
            } else {
                *bg = BackgroundColor(BTN_BG_HOVER);
                *border = BorderColor::all(BTN_BORDER_HOVER);
            }
        }

        // Scroll the grid so the focused card is visible.
        // Each row = CARD_H + GAP pixels. Show up to VISIBLE_ROWS rows at a time.
        let row_height = CARD_H + GAP;
        let focus_row = scroll.focus / GRID_COLS;
        let max_visible_y = scroll.scroll_y + VISIBLE_ROWS as f32 * row_height;
        let focus_y = focus_row as f32 * row_height;
        if focus_y < scroll.scroll_y {
            scroll.scroll_y = focus_y;
        } else if focus_y + row_height > max_visible_y {
            scroll.scroll_y = focus_y + row_height - VISIBLE_ROWS as f32 * row_height;
        }
        // Clamp scroll_y >= 0.
        scroll.scroll_y = scroll.scroll_y.max(0.0);

        // Apply scroll offset to the grid container.
        if let Ok(mut node) = grid_node.single_mut() {
            node.margin = UiRect { top: Val::Px(-scroll.scroll_y), ..default() };
        }

        // Confirm selection — hire the focused class.
        if has(UiAction::Confirm) {
            let class = HeroClass::all()[scroll.focus];
            hire_hero_of_class(&mut map_view_state, class);
            for entity in select_root.iter() {
                commands.entity(entity).despawn_children().despawn();
            }
            next_state.set(AppState::MapView);
            return;
        }

        // Cancel — close overlay, refresh and return to entrance screen.
        if has(UiAction::Cancel) {
            for entity in select_root.iter() {
                commands.entity(entity).despawn_children().despawn();
            }
            // Despawn old entrance UI and rebuild to reflect current state.
            for entity in entrance_roots.iter() {
                commands.entity(entity).despawn_children().despawn();
            }
            build_city_entrance(&mut commands, &map_view_state);
            return;
        }

        // Mouse click on a card.
        for (card, _bg, _border, interaction) in cards.iter_mut() {
            if matches!(interaction, Interaction::Pressed) {
                hire_hero_of_class(&mut map_view_state, card.class);
                for entity in select_root.iter() {
                    commands.entity(entity).despawn_children().despawn();
                }
                next_state.set(AppState::MapView);
                return;
            }
        }

        return;
    }

    // ── Normal entrance screen mode ────────────────────────────────

    if has(UiAction::Cancel) {
        next_state.set(AppState::MapView);
        return;
    }

    let mut hire_triggered = false;
    for (mut bg, mut border, interaction) in hire_btns.iter_mut() {
        let pressed = matches!(interaction, Interaction::Pressed);
        let hovered = matches!(interaction, Interaction::Hovered);
        if pressed {
            *bg = BackgroundColor(BTN_BG_PRESSED);
            *border = BorderColor::all(BTN_BORDER_PRESSED);
        } else if hovered {
            *bg = BackgroundColor(BTN_BG_HOVER);
            *border = BorderColor::all(BTN_BORDER_HOVER);
        } else {
            *bg = BackgroundColor(BTN_BG_SELECTED);
            *border = BorderColor::all(BTN_BORDER_SELECTED);
        }
        if pressed || has(UiAction::Confirm) {
            hire_triggered = true;
        }
    }

    if hire_triggered {
        // Open hero class selection — pass the atlas handles from MapViewState.
        spawn_hero_select(&mut commands, &map_view_state.atlas_image, &map_view_state.atlas_layout);
        return;
    }

    if hire_btns.is_empty() && has(UiAction::Confirm) {
        next_state.set(AppState::MapView);
    }
}

/// Hires a hero of the given class at the current cursor position.
fn hire_hero_of_class(map_view_state: &mut ResMut<MapViewState>, class: HeroClass) {
    if let Some(map_view) = map_view_state.map_view.as_mut() {
        let coord = map_view.cursor_coord();
        let team_id = map_view.session().state().get_active_team_id().copied();

        if let (Some(coord), Ok(team_id)) = (coord, team_id) {
            let state = map_view.session_mut().state_mut();
            let can_hire =
                state.hero_at(coord).is_none() && state.city_owner(coord) == Some(team_id);
            if can_hire {
                let hero_count = state.get_team_heroes(team_id).len();
                let name = format!("{} {}", class.display_name(), hero_count + 1);

                let hero = Hero::new(0, class, name, coord, team_id);
                let new_id = state.add_hero(hero);

                // Always make the newly hired hero the active hero.
                map_view.session_mut().set_selected_hero_id(new_id);
                map_view.sync_cursor_to_hero();

                map_view.set_status(Some("Hero hired!".to_string()));
            } else {
                map_view.set_status(Some("Cannot hire here".to_string()));
            }
        }
    }
}

fn exit_city_entrance(
    mut commands: Commands,
    query: Query<Entity, With<CityEntranceRoot>>,
    select: Query<Entity, With<HeroSelectRoot>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn_children().despawn();
    }
    for entity in select.iter() {
        commands.entity(entity).despawn_children().despawn();
    }
    commands.remove_resource::<HeroSelectScroll>();
}
