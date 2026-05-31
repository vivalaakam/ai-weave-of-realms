use crate::input::UiAction;
use crate::screens::AppState;
use crate::screens::map_view::MapViewState;
use bevy::prelude::*;

#[derive(Component)]
pub struct CityRoot;

pub struct CityPlugin;

impl Plugin for CityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::City), enter_city)
            .add_systems(OnExit(AppState::City), exit_city)
            .add_systems(Update, update_city.run_if(in_state(AppState::City)));
    }
}

fn enter_city(mut commands: Commands, map_view_state: Res<MapViewState>) {
    let info = map_view_state.map_view.as_ref().and_then(|map_view| map_view.cursor_structure());

    let title = info.as_ref().map(|city| city.name.clone()).unwrap_or_else(|| "City".to_string());
    let details = info
        .as_ref()
        .map(|city| {
            format!(
                "Area: {}x{}  Origin: {},{}",
                city.width(),
                city.height(),
                city.min_x,
                city.min_y
            )
        })
        .unwrap_or_else(|| "No city selected".to_string());

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
            BackgroundColor(Color::srgb(0.08, 0.08, 0.12)),
            CityRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(title),
                TextFont { font_size: FontSize::Px(42.0), ..default() },
                TextColor(Color::srgb(0.92, 0.92, 0.88)),
            ));
            parent.spawn((
                Text::new(details),
                TextFont { font_size: FontSize::Px(20.0), ..default() },
                TextColor(Color::srgb(0.72, 0.72, 0.76)),
            ));
            parent.spawn((
                Text::new("Enter/Cross or Esc/Circle: back"),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::srgb(0.5, 0.5, 0.55)),
            ));
        });
}

fn update_city(mut next_state: ResMut<NextState<AppState>>, mut reader: MessageReader<UiAction>) {
    let actions: Vec<UiAction> = reader.read().copied().collect();
    if actions.contains(&UiAction::Confirm) || actions.contains(&UiAction::Cancel) {
        next_state.set(AppState::MapView);
    }
}

fn exit_city(mut commands: Commands, query: Query<Entity, With<CityRoot>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}
