//! # rpg-engine
//!
//! Core game logic for the AI RPG project.
//!
//! This crate has **zero** Godot dependencies and can be used independently
//! of the rendering layer for testing and tooling.
//!
//! ## Modules
//! - [`rng`]  — Keccak256-based deterministic [`rng::SeededRng`]
//! - [`map`]  — Map types: [`map::TileKind`], [`map::Tile`], [`map::Chunk`], [`map::GameMap`]
//! - [`error`] — Crate-level error type

pub mod error;
pub mod map;
pub mod rng;

#[cfg(test)]
pub mod test_utils;
