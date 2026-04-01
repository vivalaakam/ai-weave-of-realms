//! # rpg-godot
//!
//! GDExtension bridge between Godot 4 and the rpg-engine / rpg-mapgen crates.
//!
//! This is the **only** crate in the workspace that depends on `godot` (gdext).
//! All game logic lives in `rpg-engine`; this crate is responsible solely for
//! exposing that logic to the Godot scene tree through `GodotClass` nodes.
//!
//! ## Registered classes
//! | Class | Base | Purpose |
//! |-------|------|---------|
//! | [`GameManager`](game_manager::GameManager) | `Node` | Owns `GameState`, drives turn loop |
//! | [`MapNode`](map_node::MapNode) | `Node` | Populates `TileMapLayer` from map data |
//! | [`HeroNode`](hero_node::HeroNode) | `Node2D` | Visual hero unit, emits move/select signals |
//! | [`ScoreUI`](score_ui::ScoreUI) | `Label` | Auto-updates score display |

use godot::prelude::*;

pub mod build_info;
pub mod camera_controller;
pub mod coords;
pub mod error;
pub mod game_manager;
pub mod hero_node;
pub mod main_scene;
pub mod map_node;
pub mod score_ui;
pub mod tile_highlight;

/// GDExtension entry point — registers all Godot classes.
struct RpgGodotExtension;

#[gdextension]
unsafe impl ExtensionLibrary for RpgGodotExtension {}
