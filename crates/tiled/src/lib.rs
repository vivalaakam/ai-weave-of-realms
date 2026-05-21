//! # rpg-tiled
//!
//! TMX (Tiled Map Editor) import and export for [`rpg_engine::map::game_map::GameMap`].
//!
//! Supports the isometric staggered diamond layout used by the project.
//! TMX files reference tile GIDs defined in [`rpg_engine::map::tile::Tiles::to_gid`].
//!
//! ## Modules
//! - [`exporter`] — `GameMap` → `.tmx` XML
//! - [`importer`] — `.tmx` XML → `GameMap`
//! - [`error`] — Crate-level error type
//!
//! ## Quick start
//! ```rust,no_run
//! use rpg_tiled::{write_tmx, read_tmx};
//! use std::path::Path;
//!
//! // Export
//! # fn example(map: rpg_engine::map::game_map::GameMap) -> Result<(), rpg_tiled::error::Error> {
//! write_tmx(&map, Path::new("output/map.tmx"), "../tileset/tileset.tsx")?;
//!
//! // Import
//! let imported = read_tmx(Path::new("output/map.tmx"))?;
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod exporter;
pub mod importer;

pub use exporter::{export_tmx, write_tmx};
pub use importer::{import_tmx, read_tmx};
