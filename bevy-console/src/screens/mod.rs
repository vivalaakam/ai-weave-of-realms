use bevy::prelude::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, States)]
pub enum AppState {
    #[default]
    Splash,
    MapSelect,
    SaveSelect,
    RandomMap,
    TeamSetup,
    MapView,
}

pub mod splash;
pub mod map_select;
pub mod save_select;
pub mod random_map;
pub mod team_setup;
pub mod map_view;
pub mod overlays;
