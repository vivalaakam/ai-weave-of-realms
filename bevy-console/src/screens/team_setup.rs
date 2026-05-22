use bevy::prelude::*;
use crate::app_host::{build_state_with_teams, PendingMapData, TeamConfig};
use crate::screens::AppState;
use engine::game_state::GameState;

#[derive(Component)]
pub struct TeamSetupRoot;

#[derive(Resource)]
pub struct TeamSetupState {
    pub count: usize,
    pub selected: usize,
    pub confirm_focus: bool,
    pub teams: Vec<TeamRow>,
    pub status: Option<String>,
}

impl Default for TeamSetupState {
    fn default() -> Self {
        Self::new(2)
    }
}

impl TeamSetupState {
    pub fn new(count: usize) -> Self {
        let count = count.clamp(1, 8);
        Self {
            count,
            selected: 0,
            confirm_focus: false,
            teams: (0..count)
                .map(|i| TeamRow {
                    name: generate_team_name(i),
                    color: generate_team_color(i, count),
                    player_controlled: i == 0,
                })
                .collect(),
            status: None,
        }
    }

    pub fn rebuild(&mut self, new_count: usize) {
        let new_count = new_count.clamp(1, 8);
        let old = std::mem::take(&mut self.teams);
        let mut new = Vec::with_capacity(new_count);
        for i in 0..new_count {
            if i < old.len() {
                new.push(old[i].clone());
            } else {
                new.push(TeamRow {
                    name: generate_team_name(i),
                    color: generate_team_color(i, new_count),
                    player_controlled: false,
                });
            }
        }
        // Ensure exactly one player team
        let has_player = new.iter().any(|t| t.player_controlled);
        if !has_player && !new.is_empty() {
            new[0].player_controlled = true;
        }
        self.teams = new;
        self.count = new_count;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamRow {
    pub name: String,
    pub color: (u8, u8, u8),
    pub player_controlled: bool,
}

const TITLE: &str = "Team Setup";
const FOOTER: &str = "Up/Down: move  Left/Right: adjust  Enter: confirm  Back: back";

const ADJECTIVES: [&str; 30] = [
    "Ember", "Crimson", "Azure", "Golden", "Shadow", "Iron", "Silver",
    "Storm", "Frost", "Blood", "Dragon", "Phoenix", "Thunder", "Night",
    "Dawn", "Eternal", "Sacred", "Ancient", "Noble", "Savage",
    "Wild", "Dark", "Bright", "Royal", "Venom", "Crystal", "Flame",
    "Phantom", "Solar", "Void",
];

const NOUNS: [&str; 30] = [
    "Legion", "Vanguard", "Order", "Clan", "Guild", "Empire", "Kingdom",
    "Horde", "Tribe", "Covenant", "Alliance", "Fellowship", "Brotherhood",
    "Dominion", "Republic", "Dynasty", "Host", "Swarm", "Collective",
    "Circle", "Syndicate", "Cult", "Squad", "Brigade", "Regiment",
    "Battalion", "Phalanx", "Guard", "Watch", "Sentinels",
];

fn generate_team_name(index: usize) -> String {
    let adj = ADJECTIVES[index % ADJECTIVES.len()];
    let noun = NOUNS[(index / ADJECTIVES.len()) % NOUNS.len()];
    format!("{} {}", adj, noun)
}

fn generate_team_color(index: usize, total: usize) -> (u8, u8, u8) {
    let hue = if total > 0 { (index as f64 / total as f64 * 360.0) as u16 % 360 } else { 0 };
    hsl_to_rgb(hue, 75, 55)
}

fn hsl_to_rgb(h: u16, s: u8, l: u8) -> (u8, u8, u8) {
    let h = h as f64 / 360.0;
    let s = s as f64 / 100.0;
    let l = l as f64 / 100.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r1, g1, b1) = match (h * 6.0) as u8 {
        0 | 1 => (c, x, 0.0),
        2 => (x, c, 0.0),
        3 => (0.0, c, x),
        4 => (0.0, x, c),
        5 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r1 + m) * 255.0).clamp(0.0, 255.0) as u8,
        ((g1 + m) * 255.0).clamp(0.0, 255.0) as u8,
        ((b1 + m) * 255.0).clamp(0.0, 255.0) as u8,
    )
}

pub struct TeamSetupPlugin;

impl Plugin for TeamSetupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TeamSetupState>()
            .add_systems(OnEnter(AppState::TeamSetup), enter_team_setup)
            .add_systems(OnExit(AppState::TeamSetup), exit_team_setup)
            .add_systems(Update, update_team_setup.run_if(in_state(AppState::TeamSetup)));
    }
}

fn enter_team_setup(mut commands: Commands, state: Res<TeamSetupState>) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.08, 0.12)),
            TeamSetupRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(TITLE),
                TextFont { font_size: FontSize::Px(36.0), ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));

            parent.spawn((Node::default(), BackgroundColor(Color::NONE)));

            // Team count selector
            parent.spawn((
                Text::new(format!("Teams: {}", state.count)),
                TextFont { font_size: FontSize::Px(20.0), ..default() },
                TextColor(Color::srgb(0.7, 0.7, 0.75)),
            ));

            // Team rows
            for team in state.teams.iter() {
                let color = Color::srgb(
                    team.color.0 as f32 / 255.0,
                    team.color.1 as f32 / 255.0,
                    team.color.2 as f32 / 255.0,
                );
                let ctrl = if team.player_controlled { "Human" } else { "CPU" };

                parent
                    .spawn((
                        Node {
                            width: Val::Px(480.0),
                            height: Val::Px(36.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(12.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.12, 0.12, 0.16)),
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Node {
                                width: Val::Px(20.0),
                                height: Val::Px(20.0),
                                ..default()
                            },
                            BackgroundColor(color),
                        ));
                        row.spawn((
                            Text::new(team.name.clone()),
                            TextFont { font_size: FontSize::Px(16.0), ..default() },
                            TextColor(Color::srgb(0.85, 0.85, 0.85)),
                        ));
                        row.spawn((
                            Text::new(ctrl.to_string()),
                            TextFont { font_size: FontSize::Px(14.0), ..default() },
                            TextColor(Color::srgb(0.6, 0.6, 0.65)),
                        ));
                    });
            }

            parent.spawn((Node::default(), BackgroundColor(Color::NONE)));

            parent.spawn((
                Button,
                Node {
                    width: Val::Px(160.0),
                    height: Val::Px(44.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.2, 0.2, 0.25)),
                BorderColor::all(Color::srgb(0.4, 0.4, 0.5)),
                children![(
                    Text::new("Play"),
                    TextFont { font_size: FontSize::Px(20.0), ..default() },
                    TextColor(Color::srgb(0.9, 0.9, 0.9)),
                )],
            ));

            parent.spawn((Node::default(), BackgroundColor(Color::NONE)));

            parent.spawn((
                Text::new(FOOTER),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::srgb(0.5, 0.5, 0.55)),
            ));
        });
}

fn update_team_setup(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    mut state: ResMut<TeamSetupState>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Query<&Interaction, With<Button>>,
    pending: Option<Res<PendingMapData>>,
) {
    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Backspace) {
        commands.remove_resource::<PendingMapData>();
        next_state.set(AppState::MapSelect);
        return;
    }

    let sel = state.selected;
    if keys.just_pressed(KeyCode::ArrowUp) {
        state.selected = sel.saturating_sub(1);
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        state.selected = (sel + 1).min(state.teams.len().saturating_sub(1));
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        if sel == 0 {
            let new_count = state.count.saturating_sub(1).max(1);
            if new_count != state.count {
                state.rebuild(new_count);
            }
        } else if sel > 0 {
            let idx = sel - 1;
            if let Some(team) = state.teams.get_mut(idx) {
                team.player_controlled = !team.player_controlled;
            }
        }
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        if sel == 0 {
            let new_count = (state.count + 1).min(8);
            if new_count != state.count {
                state.rebuild(new_count);
            }
        } else if sel > 0 {
            let idx = sel - 1;
            if let Some(team) = state.teams.get_mut(idx) {
                team.player_controlled = !team.player_controlled;
            }
        }
    }
    if keys.just_pressed(KeyCode::Enter)
        || buttons.iter().any(|i| matches!(i, Interaction::Pressed))
    {
        if let Some(pending) = pending {
            let team_cfgs: Vec<TeamConfig> = state
                .teams
                .iter()
                .map(|t| TeamConfig {
                    name: t.name.clone(),
                    color: t.color,
                    player_controlled: t.player_controlled,
                })
                .collect();

            if let Some(map) = &pending.map {
                match build_state_with_teams(map.clone(), &pending.map_name, &team_cfgs) {
                    Ok(game_state) => {
                        commands.insert_resource(LoadedSession {
                            map_name: pending.map_name.clone(),
                            state: game_state,
                        });
                        commands.remove_resource::<PendingMapData>();
                        next_state.set(AppState::MapView);
                    }
                    Err(e) => {
                        state.status = Some(e.to_string());
                    }
                }
            } else {
                state.status = Some("No map loaded".to_string());
            }
        } else {
            state.status = Some("No pending map".to_string());
        }
    }
}

#[derive(Resource)]
pub struct LoadedSession {
    pub map_name: String,
    pub state: GameState,
}

fn exit_team_setup(
    mut commands: Commands,
    query: Query<Entity, With<TeamSetupRoot>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}
