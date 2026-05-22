use bevy::prelude::*;
use crate::screens::AppState;

#[derive(Component)]
pub struct MapViewRoot;

pub struct MapViewPlugin;

impl Plugin for MapViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::MapView), enter_map_view)
           .add_systems(OnExit(AppState::MapView), exit_map_view);
    }
}

fn enter_map_view(mut commands: Commands) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.15)),
            MapViewRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Map View"),
                TextFont { font_size: FontSize::Px(36.0), ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));
        });
}

fn exit_map_view(mut commands: Commands, query: Query<Entity, With<MapViewRoot>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}
