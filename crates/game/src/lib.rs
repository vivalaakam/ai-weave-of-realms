//! Shared frontend primitives for embedded-graphics-based runtimes.
//!
//! This crate contains the common gameplay map-view model used by both the
//! hardware `tdeck` app and the host-side `sixel-console` launcher.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod app;
#[cfg(feature = "std")]
pub mod app_host;
pub mod info_overlay;
pub mod input;
pub mod list;
pub mod map_view;
pub mod prelude;
pub mod random_map;
pub mod render;
pub mod save_overlay;
pub mod session;
pub mod splash;
pub mod team_setup;
pub mod turn_overlay;
pub mod types;

#[cfg(feature = "std")]
pub use engine::game_state::GameState;
pub use engine::map::tile::Tiles;

#[cfg(feature = "std")]
pub mod io;
