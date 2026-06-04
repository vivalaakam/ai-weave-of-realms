//! Bevy-based console frontend for ai-rpg-v2.
//!
//! Launches a fullscreen Bevy window by default. Pass `--windowed` to run in windowed mode.

use bevy::prelude::*;
use bevy::window::{MonitorSelection, WindowMode};
use clap::Parser;
use std::path::PathBuf;

mod app_host;
mod atlas;
mod input;
mod input_event;
mod screens;

use app_host::AppHost;

/// Command-line arguments for the Bevy console frontend.
#[derive(Parser, Resource, Debug)]
#[command(about = "Bevy console frontend for ai-rpg-v2")]
struct Args {
    /// Load a saved game state from an .rpgs file.
    #[arg(long)]
    save: Option<PathBuf>,

    /// Load a TMX map instead of generating one.
    #[arg(long)]
    tmx: Option<PathBuf>,

    /// Seed phrase for deterministic generation.
    #[arg(long, default_value = "default-seed")]
    seed: String,

    /// Map width in tiles when generating.
    #[arg(long, default_value_t = 96)]
    width: u32,

    /// Map height in tiles when generating.
    #[arg(long, default_value_t = 96)]
    height: u32,

    /// Generator script path (repeatable pipeline).
    #[arg(long = "generator", value_name = "SCRIPT")]
    generator: Option<PathBuf>,

    /// Directory with validation rule scripts.
    #[arg(long)]
    validator_dir: Option<PathBuf>,

    /// Path to a single Lua validator script.
    #[arg(long)]
    validator: Option<PathBuf>,

    /// Path to the Lua evaluator script.
    #[arg(long)]
    evaluator: Option<PathBuf>,

    /// Run in windowed mode instead of fullscreen.
    #[arg(long = "windowed")]
    window_mode: bool,
}

fn main() {
    let args = Args::parse();

    let Ok(tiles) = engine::config::init_tile_config(include_str!("../../assets/tiles.yaml"))
    else {
        eprintln!("failed to load tile config");
        std::process::exit(1);
    };

    let Ok(team_catalog) =
        engine::config::init_team_catalog(include_str!("../../assets/teams.yaml"))
    else {
        eprintln!("failed to load team catalog");
        std::process::exit(1);
    };

    let Ok(hero_catalog) =
        engine::config::init_hero_catalog(include_str!("../../assets/heroes.yaml"))
    else {
        eprintln!("failed to load hero catalog:");
        std::process::exit(1);
    };

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                mode: WindowMode::BorderlessFullscreen(MonitorSelection::Current),
                title: "weave of realms".into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(AppHost {
            maps_dir: PathBuf::from("maps"),
            saves_dir: PathBuf::from("savegame"),
            seed: args.seed,
            width: args.width,
            height: args.height,
            generator: args.generator,
            validator_dir: args.validator_dir,
            validator: args.validator,
            evaluator: args.evaluator,
            tiles,
            team_catalog,
            hero_catalog,
        })
        .init_state::<screens::AppState>()
        .add_plugins(atlas::TileAtlasPlugin)
        .add_plugins(input::InputPlugin)
        .add_plugins(screens::splash::SplashPlugin)
        .add_plugins(screens::map_select::MapSelectPlugin)
        .add_plugins(screens::save_select::SaveSelectPlugin)
        .add_plugins(screens::random_map::RandomMapPlugin)
        .add_plugins(screens::team_setup::TeamSetupPlugin)
        .add_plugins(screens::map_view::MapViewPlugin)
        .add_plugins(screens::city::CityPlugin)
        .add_plugins(screens::city_entrance::CityEntrancePlugin)
        .add_plugins(screens::hero::HeroPlugin)
        .add_plugins(screens::exit_confirm::ExitConfirmPlugin)
        .add_systems(Startup, setup_camera)
        .run();
}

/// Spawns a 2D camera on startup.
fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
