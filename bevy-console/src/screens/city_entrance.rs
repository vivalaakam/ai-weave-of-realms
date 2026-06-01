use crate::input::UiAction;
use crate::screens::AppState;
use crate::screens::map_view::MapViewState;
use bevy::prelude::*;

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

fn enter_city_entrance(mut commands: Commands, map_view_state: Res<MapViewState>) {
    // The squad cell is the tile under the cursor (the city entrance).
    let hero_name = map_view_state.map_view.as_ref().and_then(|map_view| {
        let coord = map_view.cursor_coord()?;
        map_view.session().state().hero_at(coord).map(|hero| hero.name.clone())
    });

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

            match hero_name {
                // A hero already garrisons the entrance — show its name.
                Some(name) => {
                    parent.spawn((
                        Text::new(name),
                        TextFont { font_size: FontSize::Px(24.0), ..default() },
                        TextColor(SUBTLE_COLOR),
                    ));
                }
                // Empty cell — offer to hire a hero.
                None => {
                    parent.spawn((
                        Text::new("No hero stationed here"),
                        TextFont { font_size: FontSize::Px(20.0), ..default() },
                        TextColor(SUBTLE_COLOR),
                    ));
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
            }

            parent.spawn((
                Text::new("Enter/Cross: select   Esc/Circle: back"),
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

    // When the cell is empty the Hire button is present and always selected.
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
        // TODO: implement actual hero recruitment in the engine.
        if let Some(map_view) = map_view_state.map_view.as_mut() {
            map_view.set_status(Some("Recruitment not implemented yet".to_string()));
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
