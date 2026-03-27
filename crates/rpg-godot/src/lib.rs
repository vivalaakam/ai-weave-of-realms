//! # rpg-godot
//!
//! GDExtension bridge between Godot 4 and the rpg-engine / rpg-mapgen crates.
//!
//! This is the **only** crate in the workspace that depends on `godot` (gdext).
//! All game logic lives in `rpg-engine`; this crate is responsible solely for
//! exposing that logic to the Godot scene tree through `GodotClass` nodes.
//!
//! ## Modules
//! - [`error`] — Crate-level error type

use godot::prelude::*;

pub mod error;

/// GDExtension entry point.
///
/// Registers all Godot classes exported by this library.
struct RpgGodotExtension;

#[gdextension]
unsafe impl ExtensionLibrary for RpgGodotExtension {}
