//! # rpg-mapgen
//!
//! Procedural map generator for the AI RPG project.
//!
//! Maps are assembled from independently generated 32×32 chunks.
//! Each chunk is produced by a Lua script that receives a [`rpg_engine::rng::SeededRng`]
//! seeded from the map seed + chunk coordinates, ensuring full determinism.
//!
//! ## Modules
//! - [`error`] — Crate-level error type

pub mod error;
