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
    City,
}

pub mod city;
pub mod map_select;
pub mod map_view;
pub mod overlays;
pub mod random_map;
pub mod save_select;
pub mod splash;
pub mod team_setup;
