//! # rpg-engine
//!
//! Core game logic for the AI RPG project.
//!
//! This crate has **zero** Godot dependencies and can be used independently
//! of the rendering layer for testing and tooling.
//!
//! ## Modules
//! - [`rng`]        — Keccak256-based deterministic [`rng::SeededRng`]
//! - [`map`]        — Map types: tiles, chunks, [`map::game_map::GameMap`]
//! - [`hero`]       — [`hero::Hero`] entity and [`hero::Team`]
//! - [`movement`]   — Reachable tiles and pathfinding (Dijkstra)
//! - [`combat`]     — Auto-resolve combat between heroes
//! - [`score`]      — [`score::ScoreBoard`] and [`score::ScoreEvent`]
//! - [`game_state`] — [`game_state::GameState`] and turn manager
//! - [`spawn`]      — Deterministic hero spawn selection on generated maps
//! - [`error`]      — Crate-level error type

pub mod combat;
pub mod error;
pub mod game_state;
pub mod hero;
pub mod map;
pub mod movement;
pub mod rng;
pub mod score;
pub mod spawn;

#[cfg(test)]
pub mod test_utils;
