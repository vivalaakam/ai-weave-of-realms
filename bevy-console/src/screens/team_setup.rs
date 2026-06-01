use crate::app_host::{PendingMapData, TeamConfig, build_state_with_teams};
use crate::input::UiAction;
use crate::screens::AppState;
use bevy::prelude::*;
use engine::game_state::GameState;
use std::collections::HashSet;

#[derive(Component)]
pub struct TeamSetupRoot;

#[derive(Resource)]
pub struct TeamSetupState {
    pub count: usize,
    pub selected: usize,
    pub teams: Vec<TeamRow>,
    pub status: Option<String>,
    pub needs_rebuild: bool,
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
            teams: (0..count)
                .map(|i| TeamRow {
                    name: generate_team_name(i),
                    color: generate_team_color(i, count),
                    player_controlled: i == 0,
                })
                .collect(),
            status: None,
            needs_rebuild: true,
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
        // Ensure at least one player-controlled team
        let has_player = new.iter().any(|t| t.player_controlled);
        if !has_player && !new.is_empty() {
            new[0].player_controlled = true;
        }
        self.teams = new;
        self.count = new_count;
        if self.selected > self.count {
            self.selected = self.count;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamRow {
    pub name: String,
    pub color: (u8, u8, u8),
    pub player_controlled: bool,
}

const TITLE: &str = "Team Setup";
const FOOTER: &str = "Up/Down: move | Left/Right: adjust | Enter: confirm | Esc: back";

const ADJECTIVES: [&str; 30] = [
    "Ember", "Crimson", "Azure", "Golden", "Shadow", "Iron", "Silver", "Storm", "Frost", "Blood",
    "Dragon", "Phoenix", "Thunder", "Night", "Dawn", "Eternal", "Sacred", "Ancient", "Noble",
    "Savage", "Wild", "Dark", "Bright", "Royal", "Venom", "Crystal", "Flame", "Phantom", "Solar",
    "Void",
];

const NOUNS: [&str; 30] = [
    "Legion",
    "Vanguard",
    "Order",
    "Clan",
    "Guild",
    "Empire",
    "Kingdom",
    "Horde",
    "Tribe",
    "Covenant",
    "Alliance",
    "Fellowship",
    "Brotherhood",
    "Dominion",
    "Republic",
    "Dynasty",
    "Host",
    "Swarm",
    "Collective",
    "Circle",
    "Syndicate",
    "Cult",
    "Squad",
    "Brigade",
    "Regiment",
    "Battalion",
    "Phalanx",
    "Guard",
    "Watch",
    "Sentinels",
];

// Theme
const BG_COLOR: Color = Color::srgb(0.08, 0.08, 0.12);
const TEXT_COLOR: Color = Color::srgb(0.85, 0.85, 0.88);
const TITLE_COLOR: Color = Color::srgb(0.95, 0.95, 0.98);
const FOOTER_COLOR: Color = Color::srgb(0.5, 0.5, 0.55);
const ROW_BG: Color = Color::srgb(0.12, 0.12, 0.16);
const ROW_BG_SELECTED: Color = Color::srgb(0.24, 0.24, 0.32);
const ROW_BORDER: Color = Color::srgb(0.35, 0.35, 0.42);
const ROW_BORDER_SELECTED: Color = Color::srgb(0.6, 0.6, 0.68);
const CTRL_HUMAN: Color = Color::srgb(0.6, 0.85, 0.6);
const CTRL_CPU: Color = Color::srgb(0.55, 0.55, 0.6);
const STATUS_ERROR: Color = Color::srgb(0.9, 0.5, 0.5);
const PLAY_BG: Color = Color::srgb(0.14, 0.14, 0.18);
const PLAY_BORDER: Color = Color::srgb(0.4, 0.4, 0.48);
const PLAY_BG_SELECTED: Color = Color::srgb(0.3, 0.3, 0.38);
const PLAY_BORDER_SELECTED: Color = Color::srgb(0.7, 0.7, 0.78);

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
            .add_systems(
                Update,
                (update_team_setup, rebuild_team_setup_ui).run_if(in_state(AppState::TeamSetup)),
            );
    }
}

fn enter_team_setup(mut state: ResMut<TeamSetupState>) {
    state.needs_rebuild = true;
}

fn rebuild_team_setup_ui(
    mut commands: Commands,
    mut state: ResMut<TeamSetupState>,
    root_q: Query<Entity, With<TeamSetupRoot>>,
) {
    if !state.needs_rebuild {
        return;
    }
    state.needs_rebuild = false;

    // Despawn existing root (and its children via hierarchy)
    if let Some(root) = root_q.iter().next() {
        commands.entity(root).despawn_children().despawn();
    }

    let sel = state.selected;
    let row_count = state.count + 1; // +1 for team count selector
    let play_selected = sel == row_count;

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
            BackgroundColor(BG_COLOR),
            TeamSetupRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(TITLE),
                TextFont { font_size: FontSize::Px(36.0), ..default() },
                TextColor(TITLE_COLOR),
            ));

            parent.spawn((Node::default(), BackgroundColor(Color::NONE)));

            // Team count selector (row 0)
            let count_bg = if sel == 0 { ROW_BG_SELECTED } else { ROW_BG };
            let count_border = if sel == 0 { ROW_BORDER_SELECTED } else { ROW_BORDER };
            parent
                .spawn((
                    Node {
                        width: Val::Px(480.0),
                        height: Val::Px(40.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(12.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(count_bg),
                    BorderColor::all(count_border),
                ))
                .with_children(|row| {
                    let label = if sel == 0 {
                        format!("Teams: {}", state.count)
                    } else {
                        format!("  Teams: {}", state.count)
                    };
                    row.spawn((
                        Text::new(label),
                        TextFont { font_size: FontSize::Px(18.0), ..default() },
                        TextColor(TEXT_COLOR),
                    ));
                });

            // Team rows
            for (i, team) in state.teams.iter().enumerate() {
                let row_idx = i + 1;
                let color = Color::srgb(
                    team.color.0 as f32 / 255.0,
                    team.color.1 as f32 / 255.0,
                    team.color.2 as f32 / 255.0,
                );
                let is_sel = row_idx == sel;
                let bg = if is_sel { ROW_BG_SELECTED } else { ROW_BG };
                let border = if is_sel { ROW_BORDER_SELECTED } else { ROW_BORDER };
                let ctrl_label = if team.player_controlled { "Human" } else { "CPU" };
                let ctrl_color = if team.player_controlled { CTRL_HUMAN } else { CTRL_CPU };

                parent
                    .spawn((
                        Node {
                            width: Val::Px(480.0),
                            height: Val::Px(40.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(12.0),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(bg),
                        BorderColor::all(border),
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Node { width: Val::Px(20.0), height: Val::Px(20.0), ..default() },
                            BackgroundColor(color),
                        ));
                        row.spawn((
                            Text::new(team.name.clone()),
                            TextFont { font_size: FontSize::Px(16.0), ..default() },
                            TextColor(TEXT_COLOR),
                        ));
                        row.spawn((
                            Text::new(ctrl_label.to_string()),
                            TextFont { font_size: FontSize::Px(14.0), ..default() },
                            TextColor(ctrl_color),
                        ));
                    });
            }

            parent.spawn((Node::default(), BackgroundColor(Color::NONE)));

            // Play button
            let play_bg = if play_selected { PLAY_BG_SELECTED } else { PLAY_BG };
            let play_border = if play_selected { PLAY_BORDER_SELECTED } else { PLAY_BORDER };
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(160.0),
                        height: Val::Px(44.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(play_bg),
                    BorderColor::all(play_border),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text::new("Play"),
                        TextFont { font_size: FontSize::Px(20.0), ..default() },
                        TextColor(TEXT_COLOR),
                    ));
                });

            // Status message
            let status_text = state.status.clone().unwrap_or_default();
            parent
                .spawn((
                    Node {
                        width: Val::Px(480.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text::new(status_text),
                        TextFont { font_size: FontSize::Px(14.0), ..default() },
                        TextColor(if state.status.is_some() { STATUS_ERROR } else { FOOTER_COLOR }),
                    ));
                });

            parent.spawn((Node::default(), BackgroundColor(Color::NONE)));

            parent.spawn((
                Text::new(FOOTER),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(FOOTER_COLOR),
            ));
        });
}

fn update_team_setup(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    mut state: ResMut<TeamSetupState>,
    mut reader: MessageReader<UiAction>,
    buttons: Query<&Interaction, With<Button>>,
    pending: Option<Res<PendingMapData>>,
) {
    // Collect all UiAction messages for this frame into a set for O(1) lookups.
    let actions: HashSet<UiAction> = reader.read().copied().collect();

    let row_count = state.count + 1; // +1 for count selector
    let max_sel = row_count; // last row is play button

    let mut changed = false;

    if actions.contains(&UiAction::Cancel) {
        commands.remove_resource::<PendingMapData>();
        next_state.set(AppState::MapSelect);
        return;
    }

    if actions.contains(&UiAction::Up) || actions.contains(&UiAction::CursorUp) {
        state.selected = state.selected.saturating_sub(1);
        changed = true;
    }
    if actions.contains(&UiAction::Down) || actions.contains(&UiAction::CursorDown) {
        state.selected = (state.selected + 1).min(max_sel);
        changed = true;
    }

    if actions.contains(&UiAction::Left) {
        if state.selected == 0 {
            let new_count = state.count.saturating_sub(1).max(1);
            if new_count != state.count {
                state.rebuild(new_count);
                changed = true;
            }
        } else if state.selected > 0 && state.selected <= state.count {
            let idx = state.selected - 1;
            if let Some(team) = state.teams.get_mut(idx) {
                team.player_controlled = !team.player_controlled;
                changed = true;
            }
        }
    }

    if actions.contains(&UiAction::Right) {
        if state.selected == 0 {
            let new_count = (state.count + 1).min(8);
            if new_count != state.count {
                state.rebuild(new_count);
                changed = true;
            }
        } else if state.selected > 0 && state.selected <= state.count {
            let idx = state.selected - 1;
            if let Some(team) = state.teams.get_mut(idx) {
                team.player_controlled = !team.player_controlled;
                changed = true;
            }
        }
    }

    let confirm = actions.contains(&UiAction::Confirm)
        || buttons.iter().any(|i| matches!(i, Interaction::Pressed));

    if confirm {
        if let Some(pending) = pending {
            // Ensure at least one team is player-controlled.
            let has_player = state.teams.iter().any(|t| t.player_controlled);
            if !has_player && !state.teams.is_empty() {
                state.teams[0].player_controlled = true;
            }

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
                            state: Some(game_state),
                        });
                        commands.remove_resource::<PendingMapData>();
                        next_state.set(AppState::MapView);
                        return;
                    }
                    Err(e) => {
                        state.status = Some(e.to_string());
                        changed = true;
                    }
                }
            } else {
                state.status = Some("No map loaded".to_string());
                changed = true;
            }
        } else {
            state.status = Some("No pending map".to_string());
            changed = true;
        }
    }

    if changed {
        state.needs_rebuild = true;
    }
}

#[derive(Resource)]
pub struct LoadedSession {
    pub map_name: String,
    pub state: Option<GameState>,
}

fn exit_team_setup(mut commands: Commands, query: Query<Entity, With<TeamSetupRoot>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_children().despawn();
    }
}
