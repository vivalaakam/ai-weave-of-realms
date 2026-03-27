//! # rpg-mapgen
//!
//! Procedural map generator for the AI RPG project.
//!
//! Maps are assembled from independently generated 32×32 chunks.
//! Each chunk is produced by a Lua script that receives a [`rpg_engine::rng::SeededRng`]
//! seeded from the map seed + chunk coordinates, ensuring full determinism.
//!
//! ## Main API
//! - [`map_assembler::MapAssembler`] — the top-level entry point for generating a complete map
//! - [`map_assembler::MapConfig`]    — generation configuration (seed, dimensions, scripts)
//!
//! ## Modules
//! - [`chunk_generator`] — generates a single 32×32 chunk via Lua
//! - [`stitcher`]        — smooths chunk boundary seams
//! - [`evaluator`]       — scores a generated map via a Lua script
//! - [`validator`]       — validates a generated map via a Lua script
//! - [`rng_userdata`]    — exposes [`SeededRng`](rpg_engine::rng::SeededRng) to Lua
//! - [`map_table`]       — converts [`GameMap`](rpg_engine::map::GameMap) to a Lua table
//! - [`error`]           — crate-level error type

pub mod chunk_generator;
pub mod error;
pub mod evaluator;
pub mod map_assembler;
pub mod map_table;
pub mod rng_userdata;
pub mod stitcher;
pub mod validator;

#[cfg(test)]
pub mod test_utils;
