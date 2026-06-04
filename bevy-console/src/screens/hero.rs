use crate::input::UiAction;
use crate::screens::AppState;
use crate::screens::map_view::MapViewState;
use bevy::prelude::*;

#[derive(Component)]
pub struct HeroRoot;

pub struct HeroPlugin;

impl Plugin for HeroPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Hero), enter_hero)
            .add_systems(OnExit(AppState::Hero), exit_hero)
            .add_systems(Update, update_hero.run_if(in_state(AppState::Hero)));
    }
}

fn enter_hero(mut commands: Commands, map_view_state: Res<MapViewState>) {
    let Some(coord) = map_view_state.cursor_coord() else {
        return;
    };

    let Some(state) = map_view_state.get_game_state() else {
        return;
    };

    let Some(hero) = state.hero_at(&coord) else {
        return;
    };

    let position = hero.get_position();

    let title = hero.get_name().to_owned();
    let hp = format!("HP: {}/{}", hero.get_hp(), hero.get_max_hp());
    let stats =
        format!("ATK: {}   DEF: {}   SPD: {}", hero.get_atk(), hero.get_def(), hero.get_spd());
    let mov = format!("MOV: {}/{}", hero.get_mov_remaining(), hero.get_mov());
    let pos = format!("Position: {},{}", position.x, position.y);

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.08, 0.12)),
            HeroRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(title),
                TextFont { font_size: FontSize::Px(42.0), ..default() },
                TextColor(Color::srgb(0.92, 0.92, 0.88)),
            ));
            for line in [hp, stats, mov, pos] {
                if line.is_empty() {
                    continue;
                }
                parent.spawn((
                    Text::new(line),
                    TextFont { font_size: FontSize::Px(20.0), ..default() },
                    TextColor(Color::srgb(0.72, 0.72, 0.76)),
                ));
            }
            parent.spawn((
                Text::new("Enter/Cross or Esc/Circle: back"),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::srgb(0.5, 0.5, 0.55)),
            ));
        });
}

fn update_hero(mut next_state: ResMut<NextState<AppState>>, mut reader: MessageReader<UiAction>) {
    let actions: Vec<UiAction> = reader.read().copied().collect();
    if actions.contains(&UiAction::Confirm) || actions.contains(&UiAction::Cancel) {
        next_state.set(AppState::MapView);
    }
}

fn exit_hero(mut commands: Commands, query: Query<Entity, With<HeroRoot>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_children().despawn();
    }
}
