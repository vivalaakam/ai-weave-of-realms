//! `MainScene` GDExtension node — scene root that wires all subsystems.
//!
//! Set as the root node type in `main.tscn` (`type="MainScene"`).

use godot::classes::{Camera2D, INode, InputEvent, InputEventMouseButton, Node, TileMapLayer};
use godot::global::MouseButton;
use godot::prelude::*;

use crate::coords::{tile_to_world, world_to_tile};
use crate::game_manager::GameManager;
use crate::hero_node::HeroNode;
use crate::map_node::MapNode;

// ── Constants ─────────────────────────────────────────────────────────────────

const GENERATOR_PATH: &str = "res://scripts/generators/terrain.lua";
const SEED: &str = "default-seed";
const MAP_WIDTH: i32 = 96;
const MAP_HEIGHT: i32 = 96;

// ─── MainScene ────────────────────────────────────────────────────────────────

#[derive(GodotClass)]
#[class(base=Node)]
pub struct MainScene {
    base: Base<Node>,
    selected_hero_id: i64,
}

#[godot_api]
impl INode for MainScene {
    fn init(base: Base<Node>) -> Self {
        Self { base, selected_hero_id: -1 }
    }

    fn ready(&mut self) {
        let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");

        // Build callables before connecting (connect takes &Callable)
        let cb_map_ready       = self.base().callable("_on_map_ready");
        let cb_hero_moved      = self.base().callable("_on_hero_moved");
        let cb_combat_resolved = self.base().callable("_on_combat_resolved");
        let cb_hero_defeated   = self.base().callable("_on_hero_defeated");
        let cb_turn_advanced   = self.base().callable("_on_turn_advanced");
        gm.connect("map_ready",       &cb_map_ready);
        gm.connect("hero_moved",      &cb_hero_moved);
        gm.connect("combat_resolved", &cb_combat_resolved);
        gm.connect("hero_defeated",   &cb_hero_defeated);
        gm.connect("turn_advanced",   &cb_turn_advanced);

        // score_changed → ScoreUI.on_score_changed
        if let Some(sui) = self.base().get_node_or_null("UI/ScoreUI") {
            let cb_score = sui.callable("on_score_changed");
            gm.connect("score_changed", &cb_score);
        }

        // Defer new_game so map_ready fires after ready() returns,
        // avoiding a double-borrow of MainScene.
        self.base_mut().call_deferred("_start_game", &[]);
    }

    fn input(&mut self, event: Gd<InputEvent>) {
        if let Ok(mb) = event.clone().try_cast::<InputEventMouseButton>() {
            if mb.is_pressed()
                && mb.get_button_index() == MouseButton::LEFT
                && self.selected_hero_id >= 0
            {
                if let Some(tile) = self.mouse_to_tile(&mb) {
                    let id = self.selected_hero_id;
                    let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
                    gm.bind_mut().move_hero(id, tile.x as i64, tile.y as i64);
                }
            }
        }

        if event.is_action_pressed("ui_accept") {
            let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
            gm.bind_mut().advance_turn();
        }
    }
}

#[godot_api]
impl MainScene {
    // ── Game startup ──────────────────────────────────────────────────────────

    /// Called deferred from `ready()` so that `map_ready` fires after `ready()`
    /// returns — prevents a double-borrow of `MainScene`.
    #[func]
    fn _start_game(&mut self) {
        let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        gm.bind_mut().new_game(
            GString::from(SEED),
            GString::from(GENERATOR_PATH),
            MAP_WIDTH,
            MAP_HEIGHT,
        );
    }

    // ── Signal handlers ───────────────────────────────────────────────────────

    #[func]
    fn _on_map_ready(&mut self) {
        let tilemap: Gd<TileMapLayer> = self.base().get_node_as("World/TileMapLayer");
        let mut map_node: Gd<MapNode> = self.base().get_node_as("World/MapNode");
        map_node.bind_mut().populate_tilemap(tilemap);
        self.spawn_heroes();
        self.center_camera();
    }

    #[func]
    fn _on_hero_moved(&mut self, hero_id: i64, fx: i64, fy: i64, tx: i64, ty: i64) {
        let Some(root) = self.base().get_node_or_null("World/Heroes") else { return };
        for child in root.get_children().iter_shared() {
            if let Ok(mut hn) = child.try_cast::<HeroNode>() {
                if hn.bind().hero_id == hero_id {
                    hn.bind_mut().on_hero_moved(hero_id, fx, fy, tx, ty);
                    hn.set_position(tile_to_world(tx as i32, ty as i32));
                    break;
                }
            }
        }
    }

    #[func]
    fn _on_combat_resolved(
        &mut self,
        attacker_id: i64,
        defender_id: i64,
        att_dmg: i64,
        def_dmg: i64,
        _att_alive: bool,
        _def_alive: bool,
    ) {
        godot_print!(
            "Combat: hero {} dealt {} dmg, hero {} dealt {} dmg",
            attacker_id, def_dmg, defender_id, att_dmg
        );
    }

    #[func]
    fn _on_hero_defeated(&mut self, hero_id: i64) {
        let Some(root) = self.base().get_node_or_null("World/Heroes") else { return };
        for child in root.get_children().iter_shared() {
            if let Ok(mut hn) = child.try_cast::<HeroNode>() {
                if hn.bind().hero_id == hero_id {
                    hn.bind_mut().on_hero_defeated(hero_id);
                    break;
                }
            }
        }
    }

    #[func]
    fn _on_turn_advanced(&mut self, turn: i64) {
        godot_print!("Turn {}", turn);
    }

    #[func]
    fn _on_move_requested(&mut self, hero_id: i64, x: i64, y: i64) {
        let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        gm.bind_mut().move_hero(hero_id, x, y);
    }

    #[func]
    fn _on_hero_selected(&mut self, hero_id: i64) {
        self.selected_hero_id = hero_id;
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn spawn_heroes(&mut self) {
        self.add_game_hero(1, "Hero",  100, 20, 10, 15, 4,  5,  5, "player");
        self.create_hero_node(1, "player", 5, 5);
        self.add_game_hero(2, "Enemy",  40, 12,  6,  8, 3, 10, 10, "enemy");
        self.create_hero_node(2, "enemy", 10, 10);
    }

    fn add_game_hero(
        &mut self,
        id: i64, name: &str,
        hp: i64, atk: i64, def: i64, spd: i64, mov: i64,
        x: i64, y: i64, faction: &str,
    ) {
        let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        gm.bind_mut().add_hero(
            id, GString::from(name),
            hp, atk, def, spd, mov,
            x, y,
            GString::from(faction),
        );
    }

    fn create_hero_node(&mut self, id: i64, faction: &str, tx: i32, ty: i32) {
        let mut hero = HeroNode::new_alloc();
        let name = StringName::from(&format!("Hero_{}", id));
        hero.set_name(&name);
        {
            let mut b = hero.bind_mut();
            b.hero_id   = id;
            b.faction   = GString::from(faction);
            b.selectable = faction == "player";
            b.set_tile_position(tx as i64, ty as i64);
        }
        hero.set_position(tile_to_world(tx, ty));

        let cb_move = self.base().callable("_on_move_requested");
        let cb_sel  = self.base().callable("_on_hero_selected");
        hero.connect("move_requested", &cb_move);
        hero.connect("selected",       &cb_sel);

        let mut root = self.base().get_node_as::<Node>("World/Heroes");
        let hero_node: Gd<Node> = hero.upcast();
        root.add_child(&hero_node);
    }

    fn center_camera(&mut self) {
        let cx = MAP_WIDTH / 2;
        let cy = MAP_HEIGHT / 2;
        let center = tile_to_world(cx, cy);
        if let Some(cam) = self.base().get_node_or_null("World/Camera2D") {
            if let Ok(mut cam2d) = cam.try_cast::<Camera2D>() {
                cam2d.set_position(center);
            }
        }
    }

    fn mouse_to_tile(&self, mb: &InputEventMouseButton) -> Option<Vector2i> {
        let vp     = self.base().get_viewport()?;
        let camera = vp.get_camera_2d()?;
        let center = camera.get_screen_center_position();
        let half   = vp.get_visible_rect().size * 0.5;
        let world  = center + mb.get_position() - half;
        let tile   = world_to_tile(world);
        (tile.x >= 0 && tile.y >= 0).then_some(tile)
    }
}
