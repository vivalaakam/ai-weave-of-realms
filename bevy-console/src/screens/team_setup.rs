use crate::app_host::{AppHost, PendingMapData, TeamConfig, build_state_with_teams};
use crate::atlas::{TeamLogoImages, TileAtlas};
use crate::input::UiAction;
use crate::screens::AppState;
use bevy::prelude::*;
use engine::config::{TeamCatalog, TeamKind, TeamLogo};
use engine::game_state::GameState;
use std::collections::HashSet;

#[derive(Component)]
pub struct TeamSetupRoot;

/// How a team is controlled in the upcoming game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamControl {
    /// Not part of the game.
    Off,
    /// Controlled by a human player (hot-seat allows several).
    Human,
    /// Controlled by the computer.
    Cpu,
}

impl TeamControl {
    /// Cycles to the next control state. `playable` teams may be Human; factions
    /// can only be CPU, so they skip the Human state.
    fn next(self, playable: bool) -> Self {
        match (self, playable) {
            (TeamControl::Off, true) => TeamControl::Human,
            (TeamControl::Human, _) => TeamControl::Cpu,
            (TeamControl::Cpu, _) => TeamControl::Off,
            (TeamControl::Off, false) => TeamControl::Cpu,
        }
    }

    /// Cycles to the previous control state (reverse of [`next`](Self::next)).
    fn prev(self, playable: bool) -> Self {
        match (self, playable) {
            (TeamControl::Off, true) => TeamControl::Cpu,
            (TeamControl::Cpu, true) => TeamControl::Human,
            (TeamControl::Human, _) => TeamControl::Off,
            (TeamControl::Off, false) => TeamControl::Cpu,
            (TeamControl::Cpu, false) => TeamControl::Off,
        }
    }

    fn label(self) -> &'static str {
        match self {
            TeamControl::Off => "—",
            TeamControl::Human => "Human",
            TeamControl::Cpu => "CPU",
        }
    }

    fn color(self) -> Color {
        match self {
            TeamControl::Off => CTRL_OFF,
            TeamControl::Human => CTRL_HUMAN,
            TeamControl::Cpu => CTRL_CPU,
        }
    }
}

#[derive(Clone)]
pub struct TeamRow {
    pub name: String,
    pub color: (u8, u8, u8),
    pub kind: TeamKind,
    pub logo: TeamLogo,
    pub control: TeamControl,
}

#[derive(Resource)]
pub struct TeamSetupState {
    /// Currently highlighted row (0 = races selector, 1..=teams = team rows,
    /// teams+1 = Play button).
    pub selected: usize,
    pub teams: Vec<TeamRow>,
    /// How many hostile races to add to the game.
    pub num_races: usize,
    /// Maximum number of races available in the catalogue.
    pub max_races: usize,
    pub status: Option<String>,
    pub needs_rebuild: bool,
}

impl Default for TeamSetupState {
    fn default() -> Self {
        Self::from_catalog(None)
    }
}

impl TeamSetupState {
    fn from_catalog(catalog: Option<&TeamCatalog>) -> Self {
        let mut teams = Vec::new();
        let mut max_races = 0;

        if let Some(catalog) = catalog {
            // Playable teams first, then factions. The first playable team
            // defaults to Human; everything else starts disabled.
            for (i, def) in catalog.playable().into_iter().enumerate() {
                teams.push(TeamRow {
                    name: def.name.clone(),
                    color: def.color,
                    kind: def.kind,
                    logo: def.logo.clone(),
                    control: if i == 0 { TeamControl::Human } else { TeamControl::Off },
                });
            }
            for def in catalog.factions() {
                teams.push(TeamRow {
                    name: def.name.clone(),
                    color: def.color,
                    kind: def.kind,
                    logo: def.logo.clone(),
                    control: TeamControl::Off,
                });
            }
            max_races = catalog.races().len();
        }

        // Fallback when no catalogue is loaded (e.g. tests): one Human team.
        if teams.is_empty() {
            teams.push(TeamRow {
                name: "Red".to_string(),
                color: (220, 50, 50),
                kind: TeamKind::Playable,
                logo: TeamLogo::Tile(0),
                control: TeamControl::Human,
            });
        }

        Self { selected: 0, teams, num_races: 0, max_races, status: None, needs_rebuild: true }
    }

    /// Number of selectable rows below the races selector and above Play.
    fn team_count(&self) -> usize {
        self.teams.len()
    }
}

const TITLE: &str = "Team Setup";
const FOOTER: &str = "Up/Down: move | Left/Right: adjust | Enter: confirm | Esc: back";

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
const CTRL_CPU: Color = Color::srgb(0.55, 0.6, 0.85);
const CTRL_OFF: Color = Color::srgb(0.45, 0.45, 0.5);
const KIND_FACTION: Color = Color::srgb(0.8, 0.65, 0.45);
const STATUS_ERROR: Color = Color::srgb(0.9, 0.5, 0.5);
const PLAY_BG: Color = Color::srgb(0.14, 0.14, 0.18);
const PLAY_BORDER: Color = Color::srgb(0.4, 0.4, 0.48);
const PLAY_BG_SELECTED: Color = Color::srgb(0.3, 0.3, 0.38);
const PLAY_BORDER_SELECTED: Color = Color::srgb(0.7, 0.7, 0.78);

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

fn enter_team_setup(mut state: ResMut<TeamSetupState>, host: Res<AppHost>) {
    *state = TeamSetupState::from_catalog(Some(&host.team_catalog));
    state.needs_rebuild = true;
}

fn rgb(color: (u8, u8, u8)) -> Color {
    Color::srgb(color.0 as f32 / 255.0, color.1 as f32 / 255.0, color.2 as f32 / 255.0)
}

/// Spawns the logo node (atlas tile or generated bitmap image) tinted by the
/// team colour.
fn spawn_logo(
    row: &mut ChildSpawnerCommands,
    team: &TeamRow,
    atlas: &TileAtlas,
    logo_images: &mut TeamLogoImages,
    images: &mut Assets<Image>,
) {
    let tint = rgb(team.color);
    let node = Node { width: Val::Px(22.0), height: Val::Px(22.0), ..default() };
    match &team.logo {
        TeamLogo::Tile(index) => {
            row.spawn((
                ImageNode {
                    image: atlas.image.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: atlas.layout.clone(),
                        index: *index as usize,
                    }),
                    color: tint,
                    ..default()
                },
                node,
            ));
        }
        TeamLogo::Bitmap(_) => {
            if let Some(handle) = logo_images.handle(images, &team.name, &team.logo) {
                row.spawn((ImageNode { image: handle, color: tint, ..default() }, node));
            } else {
                row.spawn((node, BackgroundColor(tint)));
            }
        }
    }
}

fn rebuild_team_setup_ui(
    mut commands: Commands,
    mut state: ResMut<TeamSetupState>,
    root_q: Query<Entity, With<TeamSetupRoot>>,
    atlas: Res<TileAtlas>,
    mut logo_images: ResMut<TeamLogoImages>,
    mut images: ResMut<Assets<Image>>,
) {
    if !state.needs_rebuild {
        return;
    }
    state.needs_rebuild = false;

    if let Some(root) = root_q.iter().next() {
        commands.entity(root).despawn_children().despawn();
    }

    let sel = state.selected;
    let play_row = state.team_count() + 1;
    let play_selected = sel == play_row;

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
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

            // Races selector (row 0).
            let races_bg = if sel == 0 { ROW_BG_SELECTED } else { ROW_BG };
            let races_border = if sel == 0 { ROW_BORDER_SELECTED } else { ROW_BORDER };
            parent
                .spawn((
                    Node {
                        width: Val::Px(520.0),
                        height: Val::Px(38.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(races_bg),
                    BorderColor::all(races_border),
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new(format!(
                            "Hostile races: {} / {}",
                            state.num_races, state.max_races
                        )),
                        TextFont { font_size: FontSize::Px(16.0), ..default() },
                        TextColor(TEXT_COLOR),
                    ));
                });

            // Team rows.
            for (i, team) in state.teams.iter().enumerate() {
                let row_idx = i + 1;
                let is_sel = row_idx == sel;
                let bg = if is_sel { ROW_BG_SELECTED } else { ROW_BG };
                let border = if is_sel { ROW_BORDER_SELECTED } else { ROW_BORDER };

                parent
                    .spawn((
                        Node {
                            width: Val::Px(520.0),
                            height: Val::Px(38.0),
                            justify_content: JustifyContent::FlexStart,
                            align_items: AlignItems::Center,
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(12.0),
                            padding: UiRect::horizontal(Val::Px(14.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(bg),
                        BorderColor::all(border),
                    ))
                    .with_children(|row| {
                        // Colour swatch.
                        row.spawn((
                            Node { width: Val::Px(18.0), height: Val::Px(18.0), ..default() },
                            BackgroundColor(rgb(team.color)),
                        ));
                        // Logo.
                        spawn_logo(row, team, &atlas, &mut logo_images, &mut images);
                        // Name (grows to push control to the right).
                        row.spawn((
                            Node { flex_grow: 1.0, ..default() },
                            children![(
                                Text::new(team.name.clone()),
                                TextFont { font_size: FontSize::Px(16.0), ..default() },
                                TextColor(TEXT_COLOR),
                            )],
                        ));
                        // Faction tag.
                        if team.kind == TeamKind::Faction {
                            row.spawn((
                                Text::new("Faction"),
                                TextFont { font_size: FontSize::Px(12.0), ..default() },
                                TextColor(KIND_FACTION),
                            ));
                        }
                        // Control state.
                        row.spawn((
                            Text::new(team.control.label().to_string()),
                            TextFont { font_size: FontSize::Px(14.0), ..default() },
                            TextColor(team.control.color()),
                        ));
                    });
            }

            parent.spawn((Node::default(), BackgroundColor(Color::NONE)));

            // Play button.
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

            // Status message.
            let status_text = state.status.clone().unwrap_or_default();
            parent
                .spawn((
                    Node {
                        width: Val::Px(520.0),
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
    host: Res<AppHost>,
    mut reader: MessageReader<UiAction>,
    buttons: Query<&Interaction, With<Button>>,
    pending: Option<Res<PendingMapData>>,
) {
    let actions: HashSet<UiAction> = reader.read().copied().collect();

    let play_row = state.team_count() + 1;
    let max_sel = play_row;

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

    let adjust_left = actions.contains(&UiAction::Left);
    let adjust_right = actions.contains(&UiAction::Right);
    if adjust_left || adjust_right {
        let sel = state.selected;
        if sel == 0 {
            // Races selector.
            if adjust_left {
                state.num_races = state.num_races.saturating_sub(1);
            } else if state.num_races < state.max_races {
                state.num_races += 1;
            }
            changed = true;
        } else if sel >= 1 && sel <= state.team_count() {
            let idx = sel - 1;
            let playable = state.teams[idx].kind == TeamKind::Playable;
            let cur = state.teams[idx].control;
            state.teams[idx].control =
                if adjust_right { cur.next(playable) } else { cur.prev(playable) };
            changed = true;
        }
    }

    let confirm = actions.contains(&UiAction::Confirm)
        || buttons.iter().any(|i| matches!(i, Interaction::Pressed));

    if confirm {
        if let Some(pending) = pending {
            let mut team_cfgs: Vec<TeamConfig> = state
                .teams
                .iter()
                .filter(|t| t.control != TeamControl::Off)
                .map(|t| TeamConfig {
                    name: t.name.clone(),
                    color: t.color,
                    player_controlled: t.control == TeamControl::Human,
                })
                .collect();

            // Append the chosen number of hostile races (always CPU/non-player).
            for def in host.team_catalog.races().into_iter().take(state.num_races) {
                team_cfgs.push(TeamConfig {
                    name: def.name.clone(),
                    color: def.color,
                    player_controlled: false,
                });
            }

            if !team_cfgs.iter().any(|t| t.player_controlled) {
                state.status = Some("Select at least one Human team".to_string());
                state.needs_rebuild = true;
                return;
            }

            if let Some(map) = &pending.map {
                match build_state_with_teams(
                    map.clone(),
                    &pending.map_name,
                    &team_cfgs,
                    host.game_config(),
                ) {
                    Ok(game_state) => {
                        commands.insert_resource(LoadedSession { state: Some(game_state) });
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
    pub state: Option<GameState>,
}

fn exit_team_setup(mut commands: Commands, query: Query<Entity, With<TeamSetupRoot>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_children().despawn();
    }
}
