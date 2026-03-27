//! # rpg-tiled
//!
//! TMX (Tiled Map Editor) import and export for [`rpg_engine::map::GameMap`].
//!
//! Supports the isometric staggered diamond layout used by the project.
//! TMX files use GIDs defined in [`rpg_engine::map::TileKind::to_gid`].
//!
//! ## Modules
//! - [`error`] — Crate-level error type

pub mod error;
