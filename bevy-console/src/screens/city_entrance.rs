use crate::input::UiAction;
use crate::screens::AppState;
use crate::screens::map_view::MapViewState;
use bevy::prelude::*;
use engine::hero::Hero;

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

#[derive(Component)]
pub struct CityEntranceRoot;

#[derive(Component)]
struct HireHeroButton;

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
    /// A hero is stationed here — show hero name.
    Occupied { name: String },
    /// No hero — and the player's team owns the city, so they can hire.
    CanHire,
    /// No hero — but the player doesn't own the city.
    NoOwnership,
}

fn get_entrance_info(map_view_state: &MapViewState) -> Option<EntranceInfo> {
    let map_view = map_view_state.map_view.as_ref()?;
    let coord = map_view.cursor_coord()?;
    let state = map_view.session().state();

    // Check if a hero is already on the tile.
    if let Some(hero) = state.hero_at(coord) {
        return Some(EntranceInfo::Occupied { name: hero.name.clone() });
    }

    // Check city ownership.
    let active_team_id = state.get_active_team_id().copied().ok()?;
    let owner = state.city_owner(coord);

    if owner == Some(active_team_id) {
        Some(EntranceInfo::CanHire)
    } else {
        Some(EntranceInfo::NoOwnership)
    }
}

fn enter_city_entrance(mut commands: Commands, map_view_state: Res<MapViewState>) {
    let info = get_entrance_info(&map_view_state);

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

#[allow(clippy::type_complexity)]
fn update_city_entrance(
    mut next_state: ResMut<NextState<AppState>>,
    mut map_view_state: ResMut<MapViewState>,
    mut reader: MessageReader<UiAction>,
    mut buttons: Query<
        (&mut BackgroundColor, &mut BorderColor, &Interaction),
        With<HireHeroButton>,
    >,
) {
    let actions: Vec<UiAction> = reader.read().copied().collect();
    let has = |action: UiAction| actions.contains(&action);

    // Esc/Circle always returns to the map.
    if has(UiAction::Cancel) {
        next_state.set(AppState::MapView);
        return;
    }

    // Hire button interaction.
    let mut hire_triggered = false;
    for (mut bg, mut border, interaction) in buttons.iter_mut() {
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
        if let Some(map_view) = map_view_state.map_view.as_mut() {
            let coord = map_view.cursor_coord();
            let team_id = map_view.session().state().get_active_team_id().copied();

            if let (Some(coord), Ok(team_id)) = (coord, team_id) {
                // Check if the team already has a selected hero before mutating.
                let had_no_hero = map_view.session().selected_hero_id().is_none();

                let state = map_view.session_mut().state_mut();
                // Only hire if the tile is unoccupied and the player owns the city.
                let can_hire =
                    state.hero_at(coord).is_none() && state.city_owner(coord) == Some(team_id);

                if can_hire {
                    // Generate a unique name for the hired hero.
                    let hero_count = state.get_team_heroes(team_id).len();
                    let name = format!("Hero {}", hero_count + 1);

                    let hero = Hero::new(
                        0, // placeholder — add_hero assigns real ID
                        name,
                        100, // hp
                        20,  // atk
                        10,  // def
                        15,  // spd → mov = 20 + 15 = 35
                        coord,
                        team_id,
                    );
                    let new_id = state.add_hero(hero);

                    // If this is the team's first hero, select it immediately.
                    if had_no_hero {
                        map_view.session_mut().set_selected_hero_id(new_id);
                        map_view.sync_cursor_to_hero();
                    }

                    map_view.set_status(Some("Hero hired!".to_string()));
                } else {
                    map_view.set_status(Some("Cannot hire here".to_string()));
                }
            }
        }
        next_state.set(AppState::MapView);
        return;
    }

    // With a hero stationed (no button), Enter just closes the window.
    if buttons.is_empty() && has(UiAction::Confirm) {
        next_state.set(AppState::MapView);
    }
}

fn exit_city_entrance(mut commands: Commands, query: Query<Entity, With<CityEntranceRoot>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}