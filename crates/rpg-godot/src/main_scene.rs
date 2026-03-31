//! `MainScene` GDExtension node — scene root that wires all subsystems.
//!
//! Set as the root node type in `main.tscn` (`type="MainScene"`).

use godot::classes::{
    Button, Camera2D, INode, InputEvent, InputEventMouseButton, LineEdit, Node, TileMapLayer,
};
use godot::global::MouseButton;
use godot::prelude::*;
use tracing::{info, warn};

use crate::coords::{tile_to_world, world_to_tile};
use crate::game_manager::GameManager;
use crate::hero_node::HeroNode;
use crate::map_node::MapNode;

// ── Constants ─────────────────────────────────────────────────────────────────

const GENERATOR_PATH: &str = "res://scripts/generators/terrain.lua";
const DEFAULT_SEED: &str = "default-seed";
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

        if let Some(mut seed_input) = self.seed_input() {
            let cb_seed_submit = self.base().callable("_on_seed_submitted");
            seed_input.connect("text_submitted", &cb_seed_submit);
        }
        if let Some(mut start_button) = self.start_button() {
            let cb_start_pressed = self.base().callable("_on_start_pressed");
            start_button.connect("pressed", &cb_start_pressed);
        }

        // Defer startup so map_ready fires after ready() returns,
        // avoiding a double-borrow of MainScene.
        self.base_mut().call_deferred("_start_game_from_ui", &[]);
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
    fn _start_game_from_ui(&mut self) {
        self.start_game_with_seed(self.current_seed());
    }

    #[func]
    fn _on_start_pressed(&mut self) {
        self.start_game_with_seed(self.current_seed());
    }

    #[func]
    fn _on_seed_submitted(&mut self, seed: GString) {
        self.start_game_with_seed(Self::normalize_seed(seed.to_string()));
    }

    fn start_game_with_seed(&mut self, seed: String) {
        let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        self.selected_hero_id = -1;
        let started = gm.bind_mut().new_game(
            GString::from(seed.as_str()),
            GString::from(GENERATOR_PATH),
            MAP_WIDTH,
            MAP_HEIGHT,
        );
        if started {
            info!("started new game from seed '{seed}'");
        } else {
            warn!("failed to start new game from seed '{seed}'");
        }
    }

    // ── Signal handlers ───────────────────────────────────────────────────────

    #[func]
    fn _on_map_ready(&mut self) {
        let tilemap: Gd<TileMapLayer> = self.base().get_node_as("World/TileMapLayer");
        let mut map_node: Gd<MapNode> = self.base().get_node_as("World/MapNode");
        map_node.bind_mut().populate_tilemap(tilemap.clone());
        self.clear_hero_nodes();
        self.spawn_heroes();
        self.center_camera(tilemap);
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
        info!(
            attacker_id,
            defender_id,
            attacker_damage = att_dmg,
            defender_damage = def_dmg,
            "combat resolved"
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
        info!(turn, "turn advanced");
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

    fn clear_hero_nodes(&mut self) {
        let Some(root) = self.base().get_node_or_null("World/Heroes") else { return };
        for mut child in root.get_children().iter_shared() {
            child.queue_free();
        }
    }

    fn center_camera(&mut self, tilemap: Gd<TileMapLayer>) {
        let center = Self::tilemap_center(tilemap)
            .unwrap_or_else(|| tile_to_world(MAP_WIDTH / 2, MAP_HEIGHT / 2));

        if let Some(cam) = self.base().get_node_or_null("World/Camera2D") {
            if let Ok(mut cam2d) = cam.try_cast::<Camera2D>() {
                cam2d.set_position_smoothing_enabled(false);
                cam2d.set_position(center);
                cam2d.reset_smoothing();
                cam2d.set_position_smoothing_enabled(true);
            }
        }
    }

    fn tilemap_center(mut tilemap: Gd<TileMapLayer>) -> Option<Vector2> {
        let center_cell = Vector2i::new(MAP_WIDTH / 2, MAP_HEIGHT / 2);
        let local = tilemap.call("map_to_local", &[center_cell.to_variant()]);
        local.try_to::<Vector2>().ok()
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

    fn current_seed(&self) -> String {
        self.seed_input()
            .map(|input| Self::normalize_seed(input.get_text().to_string()))
            .unwrap_or_else(|| DEFAULT_SEED.to_string())
    }

    fn normalize_seed(seed: String) -> String {
        let trimmed = seed.trim();
        if trimmed.is_empty() {
            DEFAULT_SEED.to_string()
        } else {
            trimmed.to_string()
        }
    }

    fn seed_input(&self) -> Option<Gd<LineEdit>> {
        self.base()
            .get_node_or_null("UI/SeedPanel/MarginContainer/Controls/SeedInput")
            .and_then(|node| node.try_cast::<LineEdit>().ok())
    }

    fn start_button(&self) -> Option<Gd<Button>> {
        self.base()
            .get_node_or_null("UI/SeedPanel/MarginContainer/Controls/StartButton")
            .and_then(|node| node.try_cast::<Button>().ok())
    }
}
