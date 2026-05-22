//! Bevy-based console frontend for ai-rpg-v2.
//!
//! Launches a fullscreen Bevy window by default. Pass `--windowed` to run in windowed mode.

use bevy::prelude::*;
use bevy::window::{MonitorSelection, WindowMode};
use clap::Parser;

mod app_host;
mod screens;

/// Command-line arguments for the Bevy console frontend.
#[derive(Parser, Resource, Debug)]
#[command(about = "Bevy console frontend for ai-rpg-v2")]
struct Args {
    /// Run in windowed mode instead of fullscreen.
    #[arg(long)]
    windowed: bool,
}

fn main() {
    let args = Args::parse();

    tracing_subscriber::fmt::init();

    let window_mode = if args.windowed {
        WindowMode::Windowed
    } else {
        WindowMode::BorderlessFullscreen(MonitorSelection::Current)
    };

    tracing::info!("Starting bevy-console, windowed={}", args.windowed);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                mode: window_mode,
                title: "weave of realms".into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<screens::AppState>()
        .add_plugins(screens::splash::SplashPlugin)
        .add_plugins(screens::map_select::MapSelectPlugin)
        .add_plugins(screens::save_select::SaveSelectPlugin)
        .add_plugins(screens::random_map::RandomMapPlugin)
        .add_plugins(screens::team_setup::TeamSetupPlugin)
        .add_plugins(screens::map_view::MapViewPlugin)
        .add_systems(Startup, setup_camera)
        .run();
}

/// Spawns a 2D camera on startup.
fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
