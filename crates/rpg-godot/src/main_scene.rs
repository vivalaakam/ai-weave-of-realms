//! `MainScene` GDExtension node — scene root that wires all subsystems.
//!
//! Set as the root node type in `main.tscn` (`type="MainScene"`).

use godot::classes::{
    Button, Camera2D, ConfirmationDialog, INode, VBoxContainer, InputEvent, InputEventKey,
    InputEventMouseButton, Label, LineEdit, Node, TileMapLayer,
};
use godot::global::{Key, MouseButton};
use godot::prelude::*;
use tracing::{info, warn};

use crate::camera_controller::CameraController;
use crate::coords::TILE_W;
use crate::game_manager::GameManager;
use crate::hero_node::HeroNode;
use crate::build_info;
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
    end_turn_dialog: Option<Gd<ConfirmationDialog>>,
}

#[godot_api]
impl INode for MainScene {
    fn init(base: Base<Node>) -> Self {
        Self { base, selected_hero_id: -1, end_turn_dialog: None }
    }

    fn ready(&mut self) {
        // Log build version
        info!(
            version = %build_info::version_string(),
            build = %build_info::BUILD_NUMBER,
            git = %build_info::GIT_HASH,
            src = %build_info::SRC_HASH,
            profile = %build_info::BUILD_PROFILE,
            "MainScene initialized"
        );

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
        let cb_enemies_spawned = self.base().callable("_on_enemies_spawned");
        gm.connect("enemies_spawned", &cb_enemies_spawned);

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
        if let Some(mut center_button) = self.center_button() {
            let cb_center_pressed = self.base().callable("_on_center_button_pressed");
            center_button.connect("pressed", &cb_center_pressed);
        }
        if let Some(mut reset_zoom_button) = self.reset_zoom_button() {
            let cb_reset_zoom = self.base().callable("_on_reset_zoom_pressed");
            reset_zoom_button.connect("pressed", &cb_reset_zoom);
        }
        if let Some(mut center_x) = self.center_x_input() {
            let cb_center_submit = self.base().callable("_on_center_inputs_submitted");
            center_x.connect("text_submitted", &cb_center_submit);
        }
        if let Some(mut center_y) = self.center_y_input() {
            let cb_center_submit = self.base().callable("_on_center_inputs_submitted");
            center_y.connect("text_submitted", &cb_center_submit);
        }

        // Update version label in UI
        self.update_version_label();

        // Create the end-of-turn confirmation dialog (hidden initially).
        self.base_mut().call_deferred("_create_end_turn_dialog", &[]);

        // Defer startup so map_ready fires after ready() returns,
        // avoiding a double-borrow of MainScene.
        self.base_mut().call_deferred("_start_game_from_ui", &[]);
    }

    fn process(&mut self, _delta: f64) {
        self.update_cursor_debug();
        self.update_zoom_debug();
        self.update_turn_label();
        self.update_hero_mov_label();
    }

    fn input(&mut self, event: Gd<InputEvent>) {
        // Пока открыт диалог завершения хода — не обрабатываем собственные привязки.
        if self.is_end_turn_dialog_visible() {
            return;
        }

        if let Ok(key) = event.clone().try_cast::<InputEventKey>() {
            if key.is_pressed() && !key.is_echo() {
                let physical = key.get_physical_keycode();
                let logical  = key.get_keycode();

                // Tab: переключить героя и пометить событие как обработанное,
                // чтобы Godot не передавал его дальше как ui_focus_next.
                if physical == Key::TAB || logical == Key::TAB {
                    self.select_next_player_hero();
                    if let Some(mut vp) = self.base().get_viewport() {
                        vp.set_input_as_handled();
                    }
                    return;
                }

                // Space: показать диалог подтверждения завершения хода.
                if physical == Key::SPACE || logical == Key::SPACE {
                    self.show_end_turn_dialog();
                    if let Some(mut vp) = self.base().get_viewport() {
                        vp.set_input_as_handled();
                    }
                    return;
                }

                // Стрелки: переместить выбранного героя на одну клетку.
                if self.selected_hero_id >= 0 {
                    let mut dx: i32 = 0;
                    let mut dy: i32 = 0;
                    if physical == Key::LEFT  || logical == Key::LEFT  { dx = -1; }
                    else if physical == Key::RIGHT || logical == Key::RIGHT { dx =  1; }
                    else if physical == Key::UP    || logical == Key::UP    { dy = -1; }
                    else if physical == Key::DOWN  || logical == Key::DOWN  { dy =  1; }

                    if dx != 0 || dy != 0 {
                        let id = self.selected_hero_id;
                        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
                        let (current, width, height) = {
                            let bound = gm.bind();
                            let pos = bound.get_hero_position(id);
                            let w   = bound.get_map_width();
                            let h   = bound.get_map_height();
                            (pos, w, h)
                        };
                        drop(gm);

                        if current.x >= 0 && current.y >= 0 {
                            let new_x = current.x + dx;
                            let new_y = current.y + dy;
                            if new_x >= 0 && new_y >= 0
                                && new_x < width as i32 && new_y < height as i32
                            {
                                let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
                                gm.call_deferred("move_hero", &[
                                    id.to_variant(),
                                    (new_x as i64).to_variant(),
                                    (new_y as i64).to_variant(),
                                ]);
                            }
                        }
                        return;
                    }
                }
            }
        }

        if let Ok(mb) = event.clone().try_cast::<InputEventMouseButton>() {
            if mb.is_pressed() && mb.get_button_index() == MouseButton::LEFT {
                if let Some(tile) = self.mouse_to_tile(&mb) {
                    // Если на тайле стоит управляемый игроком герой — выбираем его.
                    // Иначе — перемещаем текущего выбранного героя.
                    if let Some(hero_id) = self.player_hero_at_tile(tile) {
                        self.set_selected_hero(hero_id, false);
                    } else if self.selected_hero_id >= 0 {
                        let id = self.selected_hero_id;
                        let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
                        gm.call_deferred("move_hero", &[
                            id.to_variant(),
                            (tile.x as i64).to_variant(),
                            (tile.y as i64).to_variant(),
                        ]);
                    }
                }
            }
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

    #[func]
    fn _on_center_button_pressed(&mut self) {
        self.focus_camera_from_debug_inputs();
    }

    #[func]
    fn _on_center_inputs_submitted(&mut self, _value: GString) {
        self.focus_camera_from_debug_inputs();
    }

    #[func]
    fn _on_reset_zoom_pressed(&mut self) {
        if let Some(mut camera) = self.camera_controller() {
            camera.bind_mut().reset_zoom();
        }
        self.update_zoom_debug();
    }

    fn start_game_with_seed(&mut self, seed: String) {
        let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        self.selected_hero_id = -1;
        self.update_seed_debug(&seed);
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
        self.configure_camera_bounds();
        self.clear_hero_nodes();
        self.spawn_heroes();
        self.update_heroes_list();
        self.set_center_debug_inputs(MAP_WIDTH / 2, MAP_HEIGHT / 2);
        self.select_first_player_hero();
        self.center_camera(tilemap);
    }

    #[func]
    fn _on_hero_moved(&mut self, hero_id: i64, fx: i64, fy: i64, tx: i64, ty: i64) {
        info!("_on_hero_moved {hero_id} fx:{fx} fy:{ty}");
        let Some(root) = self.base().get_node_or_null("World/Heroes") else { return };
        for child in root.get_children().iter_shared() {
            if let Ok(mut hn) = child.try_cast::<HeroNode>() {
                if hn.bind().hero_id == hero_id {
                    if let Some(world_pos) = self.map_tile_to_world(Vector2i::new(tx as i32, ty as i32)) {
                        hn.set_position(world_pos);
                    }
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
        if self.selected_hero_id == hero_id {
            self.select_next_player_hero();
        }
        self.update_heroes_list();
    }

    #[func]
    fn _on_turn_advanced(&mut self, turn: i64) {
        info!(turn, "turn advanced");
        self.update_heroes_list();
    }

    #[func]
    fn _on_move_requested(&mut self, hero_id: i64, x: i64, y: i64) {
        let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        gm.call_deferred("move_hero", &[hero_id.to_variant(), x.to_variant(), y.to_variant()]);
    }

    #[func]
    fn _on_hero_selected(&mut self, hero_id: i64) {
        self.set_selected_hero(hero_id, true);
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn spawn_heroes(&mut self) {
        let Some(player_spawn) = self.player_spawn_position() else {
            warn!("skipping hero spawn because no valid start position was found");
            return;
        };

        // Spawn player hero (spd=15 → mov = 35)
        self.add_game_hero(
            1,
            "Hero",
            100,
            20,
            10,
            15,
            i64::from(player_spawn.x),
            i64::from(player_spawn.y),
            "player",
            true,
        );
        self.create_hero_node(1, "player", true, player_spawn.x, player_spawn.y);

        // Spawn enemies via Lua-driven spawner
        let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let enemy_count = gm.bind_mut().spawn_enemies();
        info!(count = enemy_count, "enemies spawned");
    }


    /// Called after enemies are spawned via Lua script.
    /// Creates visual hero nodes for all enemies.
    #[func]
    fn _on_enemies_spawned(&mut self, count: i64) {
        if count <= 0 {
            return;
        }

        // Collect enemy data first (immutable borrow).
        // Query team info from engine — HeroNode does not cache mutable state.
        let mut enemy_data: Vec<(i64, String, bool, i32, i32)> = Vec::new();
        {
            let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
            let enemy_ids = gm.bind().get_living_enemy_hero_ids();
            for id in enemy_ids.iter_shared() {
                let pos = gm.bind().get_hero_position(id);
                if pos.x >= 0 && pos.y >= 0 {
                    let team_name = gm.bind().get_hero_team_name(id).to_string();
                    let player_controlled = gm.bind().is_hero_player_controlled(id);
                    enemy_data.push((id, team_name, player_controlled, pos.x, pos.y));
                }
            }
        }

        // Now create hero nodes (mutable borrow).
        for (hero_id, team_name, player_controlled, x, y) in enemy_data {
            self.create_hero_node(hero_id, &team_name, player_controlled, x, y);
        }

        self.update_heroes_list();
    }

    fn add_game_hero(
        &mut self,
        id: i64, name: &str,
        hp: i64, atk: i64, def: i64, spd: i64,
        x: i64, y: i64, team_name: &str, player_controlled: bool,
    ) {
        let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        gm.bind_mut().add_hero(
            id, GString::from(name),
            hp, atk, def, spd,
            x, y,
            GString::from(team_name), player_controlled,
        );
    }

    fn create_hero_node(&mut self, id: i64, team_name: &str, player_controlled: bool, tx: i32, ty: i32) {
        let mut hero = HeroNode::new_alloc();
        let name = StringName::from(&format!("Hero_{}", id));
        hero.set_name(&name);
        {
            let mut b = hero.bind_mut();
            b.hero_id          = id;
            b.team_name        = GString::from(team_name);
            b.player_controlled = player_controlled;
        }
        if let Some(world_pos) = self.map_tile_to_world(Vector2i::new(tx, ty)) {
            hero.set_position(world_pos);
        }

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
        if self.selected_hero_id >= 0 {
            self.focus_camera_on_hero(self.selected_hero_id);
            return;
        }

        let center = Self::tilemap_center(tilemap)
            .unwrap_or_else(|| Vector2::new(0.0, TILE_W * 0.5));

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
        self.screen_to_tile(mb.get_position())
    }

    fn current_seed(&self) -> String {
        self.seed_input()
            .map(|input| Self::normalize_seed(input.get_text().to_string()))
            .unwrap_or_else(|| DEFAULT_SEED.to_string())
    }

    fn update_seed_debug(&self, seed: &str) {
        if let Some(mut label) = self.seed_value_label() {
            label.set_text(&format!("Seed: {seed}"));
        }
    }

    fn update_cursor_debug(&self) {
        let Some(mut label) = self.cursor_value_label() else { return };
        let Some(viewport) = self.base().get_viewport() else { return };
        let mouse = viewport.get_mouse_position();
        let text = match self.screen_to_tile(mouse) {
            Some(tile) => format!("Cursor: {}, {}", tile.x, tile.y),
            None => "Cursor: -, -".to_string(),
        };
        label.set_text(&text);
    }

    fn update_zoom_debug(&self) {
        let Some(mut label) = self.zoom_value_label() else { return };
        let Some(camera) = self.camera_controller() else { return };
        let zoom = camera.bind().current_zoom();
        label.set_text(&format!("Zoom: {:.2}", zoom));
    }

    fn configure_camera_bounds(&self) {
        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let width = gm.bind().get_map_width() as i32;
        let height = gm.bind().get_map_height() as i32;
        if let Some(mut camera) = self.camera_controller() {
            camera.bind_mut().configure_map_bounds(width, height);
        }
    }

    fn player_spawn_position(&self) -> Option<Vector2i> {
        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let player = gm.bind().get_player_spawn();
        (player.x >= 0 && player.y >= 0).then_some(player)
    }

    fn select_first_player_hero(&mut self) {
        let heroes = self.player_hero_ids();
        let Some(hero_id) = heroes.iter_shared().next() else {
            self.selected_hero_id = -1;
            return;
        };
        self.set_selected_hero(hero_id, false);
    }

    /// Updates the heroes list in the UI panel.
    fn update_heroes_list(&mut self) {
        let Some(list) = self.heroes_list() else {
            return;
        };

        // Clear existing children
        for mut child in list.get_children().iter_shared() {
            child.queue_free();
        }

        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");

        // Add player heroes
        let player_ids = gm.bind().get_living_player_hero_ids();
        for id in player_ids.iter_shared() {
            self.add_hero_to_list(&gm, id, "Hero", list.clone());
        }

        // Add enemy heroes
        let enemy_ids = gm.bind().get_living_enemy_hero_ids();
        for id in enemy_ids.iter_shared() {
            self.add_hero_to_list(&gm, id, "Enemy", list.clone());
        }

        // Highlight selected hero
        self.highlight_selected_hero_in_list();
    }

    fn add_hero_to_list(&self, gm: &Gd<GameManager>, hero_id: i64, name_prefix: &str, mut list: Gd<VBoxContainer>) {
        let pos = gm.bind().get_hero_position(hero_id);
        let is_alive = gm.bind().is_hero_alive(hero_id);
        
        let mut btn: Gd<Button> = Button::new_alloc();
        btn.set_name(&StringName::from(&format!("HeroBtn_{}", hero_id)));
        btn.set_text(&format!("{} {} ({}:{})", name_prefix, hero_id, pos.x, pos.y));
        btn.set_disabled(!is_alive);
        
        // Connect with hero_id as argument
        let cb = self.base().callable("_on_hero_list_clicked").bind(&[hero_id.to_variant()]);
        btn.connect("pressed", &cb);
        
        list.add_child(&btn);
    }

    /// Highlights the selected hero button in the UI list.
    fn highlight_selected_hero_in_list(&self) {
        let Some(list) = self.heroes_list() else { return };
        
        for child in list.get_children().iter_shared() {
            if let Ok(mut btn) = child.try_cast::<Button>() {
                let btn_name = btn.get_name().to_string();
                // Extract hero_id from button name "HeroBtn_{id}"
                let hero_id_str = btn_name.strip_prefix("HeroBtn_");
                let is_selected = hero_id_str
                    .and_then(|s| s.parse::<i64>().ok())
                    .map(|id| id == self.selected_hero_id)
                    .unwrap_or(false);
                
                if is_selected {
                    // Highlight selected hero with yellow color
                    btn.set_modulate(Color::from_rgba8(255, 255, 100, 255));
                } else {
                    // Reset to default color
                    btn.set_modulate(Color::from_rgba8(255, 255, 255, 255));
                }
            }
        }
    }

    #[func]
    fn _on_hero_list_clicked(&mut self, hero_id: i64) {
        self.set_selected_hero(hero_id, true);
    }

    // ── Диалог завершения хода ────────────────────────────────────────────────

    /// Создаёт диалог подтверждения завершения хода и скрывает его.
    /// Вызывается deferred из ready(), чтобы сцена была готова к добавлению дочерних узлов.
    #[func]
    fn _create_end_turn_dialog(&mut self) {
        let mut dialog = ConfirmationDialog::new_alloc();
        dialog.set_title("Завершить ход");
        dialog.set_text("Завершить ход и передать управление следующей команде?");
        if let Some(mut btn) = dialog.get_ok_button() { btn.set_text("Да"); }
        if let Some(mut btn) = dialog.get_cancel_button() { btn.set_text("Нет"); }

        let cb_confirmed = self.base().callable("_on_end_turn_confirmed");
        let cb_cancelled = self.base().callable("_on_end_turn_cancelled");
        dialog.connect("confirmed", &cb_confirmed);
        dialog.connect("canceled",  &cb_cancelled);

        self.base_mut().add_child(&dialog.clone());
        self.end_turn_dialog = Some(dialog);
    }

    /// Показывает диалог подтверждения завершения хода.
    fn show_end_turn_dialog(&mut self) {
        if let Some(mut dialog) = self.end_turn_dialog.clone() {
            dialog.popup_centered();
        }
    }

    /// Возвращает `true`, если диалог завершения хода сейчас виден.
    fn is_end_turn_dialog_visible(&self) -> bool {
        self.end_turn_dialog
            .as_ref()
            .map(|d| d.is_visible())
            .unwrap_or(false)
    }

    /// Вызывается при нажатии «Да» в диалоге.
    /// Завершает ход и возвращает управление первому герою игрока.
    #[func]
    fn _on_end_turn_confirmed(&mut self) {
        let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        gm.call_deferred("advance_turn", &[]);
        // После advance_turn движение сброшено у всех; выбираем первого героя игрока.
        self.select_first_player_hero();
        self.update_heroes_list();
    }

    /// Вызывается при нажатии «Нет» или Esc в диалоге.
    #[func]
    fn _on_end_turn_cancelled(&mut self) {
        // Диалог уже закрылся сам — дополнительных действий не требуется.
    }

    /// Возвращает id управляемого игроком героя, стоящего на тайле `tile`,
    /// или `None` если такого нет.
    fn player_hero_at_tile(&self, tile: Vector2i) -> Option<i64> {
        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let hero_id = gm.bind().get_hero_id_at_position(tile.x as i64, tile.y as i64);
        if hero_id >= 0 && gm.bind().is_hero_player_controlled(hero_id) {
            Some(hero_id)
        } else {
            None
        }
    }

    fn heroes_list(&self) -> Option<Gd<VBoxContainer>> {
        self.base()
            .get_node_or_null("UI/RightPanel/MarginContainer/Content/ScrollContainer/HeroesList")
            .and_then(|node| node.try_cast::<VBoxContainer>().ok())
    }

    fn select_next_player_hero(&mut self) {
        let heroes = self.player_hero_ids();
        if heroes.is_empty() {
            self.selected_hero_id = -1;
            return;
        }

        let current_index = heroes
            .iter_shared()
            .position(|hero_id| hero_id == self.selected_hero_id);
        let next_index = current_index.map(|index| (index + 1) % heroes.len()).unwrap_or(0);
        let Some(next_id) = heroes.get(next_index) else {
            return;
        };
        self.set_selected_hero(next_id, true);
    }

    fn set_selected_hero(&mut self, hero_id: i64, focus_camera: bool) {
        self.selected_hero_id = hero_id;
        self.highlight_selected_hero_in_list();
        if focus_camera {
            self.focus_camera_on_hero(hero_id);
        }
    }

    fn focus_camera_on_hero(&self, hero_id: i64) {
        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let tile = gm.bind().get_hero_position(hero_id);
        if tile.x < 0 || tile.y < 0 {
            return;
        }
        self.set_center_debug_inputs(tile.x, tile.y);
        if let Some(mut camera) = self.camera_controller() {
            if let Some(world_pos) = self.map_tile_to_world(Vector2i::new(tile.x, tile.y)) {
                camera.bind_mut().focus_on_world_position(world_pos);
            }
        }
    }

    fn focus_camera_from_debug_inputs(&self) {
        let Some((x, y)) = self.debug_center_tile() else {
            return;
        };
        if let Some(mut camera) = self.camera_controller() {
            if let Some(world_pos) = self.map_tile_to_world(Vector2i::new(x, y)) {
                camera.bind_mut().focus_on_world_position(world_pos);
            }
        }
    }

    fn player_hero_ids(&self) -> Array<i64> {
        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let ids = gm.bind().get_living_player_hero_ids();
        ids
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

    fn center_button(&self) -> Option<Gd<Button>> {
        self.base()
            .get_node_or_null("UI/RightPanel/MarginContainer/Content/CenterButton")
            .and_then(|node| node.try_cast::<Button>().ok())
    }

    fn reset_zoom_button(&self) -> Option<Gd<Button>> {
        self.base()
            .get_node_or_null("UI/RightPanel/MarginContainer/Content/ResetZoomButton")
            .and_then(|node| node.try_cast::<Button>().ok())
    }

    fn center_x_input(&self) -> Option<Gd<LineEdit>> {
        self.base()
            .get_node_or_null("UI/RightPanel/MarginContainer/Content/CenterInputs/CenterX")
            .and_then(|node| node.try_cast::<LineEdit>().ok())
    }

    fn center_y_input(&self) -> Option<Gd<LineEdit>> {
        self.base()
            .get_node_or_null("UI/RightPanel/MarginContainer/Content/CenterInputs/CenterY")
            .and_then(|node| node.try_cast::<LineEdit>().ok())
    }

    fn seed_value_label(&self) -> Option<Gd<Label>> {
        self.base()
            .get_node_or_null("UI/RightPanel/MarginContainer/Content/SeedValue")
            .and_then(|node| node.try_cast::<Label>().ok())
    }

    fn cursor_value_label(&self) -> Option<Gd<Label>> {
        self.base()
            .get_node_or_null("UI/RightPanel/MarginContainer/Content/CursorValue")
            .and_then(|node| node.try_cast::<Label>().ok())
    }

    fn zoom_value_label(&self) -> Option<Gd<Label>> {
        self.base()
            .get_node_or_null("UI/RightPanel/MarginContainer/Content/ZoomValue")
            .and_then(|node| node.try_cast::<Label>().ok())
    }

    fn camera_controller(&self) -> Option<Gd<CameraController>> {
        self.base()
            .get_node_or_null("World/Camera2D")
            .and_then(|node| node.try_cast::<CameraController>().ok())
    }

    fn set_center_debug_inputs(&self, x: i32, y: i32) {
        if let Some(mut input) = self.center_x_input() {
            input.set_text(&x.to_string());
        }
        if let Some(mut input) = self.center_y_input() {
            input.set_text(&y.to_string());
        }
    }

    fn debug_center_tile(&self) -> Option<(i32, i32)> {
        let x = self
            .center_x_input()
            .and_then(|input| input.get_text().to_string().trim().parse::<i32>().ok())?;
        let y = self
            .center_y_input()
            .and_then(|input| input.get_text().to_string().trim().parse::<i32>().ok())?;
        Some((x, y))
    }

    fn screen_to_tile(&self, screen_pos: Vector2) -> Option<Vector2i> {
        let vp = self.base().get_viewport()?;
        let camera = vp.get_camera_2d()?;
        let center = camera.get_screen_center_position();
        let half = vp.get_visible_rect().size * 0.5;
        let world = center + screen_pos - half;
        let tile = self.world_to_map_tile(world)?;
        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let width = gm.bind().get_map_width() as i32;
        let height = gm.bind().get_map_height() as i32;
        (tile.x >= 0 && tile.y >= 0 && tile.x < width && tile.y < height).then_some(tile)
    }

    fn map_tile_to_world(&self, tile: Vector2i) -> Option<Vector2> {
        let mut tilemap = self.tilemap_layer()?;
        let local = tilemap.call("map_to_local", &[tile.to_variant()]).try_to::<Vector2>().ok()?;
        Some(tilemap.to_global(local))
    }

    fn world_to_map_tile(&self, world: Vector2) -> Option<Vector2i> {
        let mut tilemap = self.tilemap_layer()?;
        let local = tilemap.to_local(world);
        tilemap.call("local_to_map", &[local.to_variant()]).try_to::<Vector2i>().ok()
    }
    fn tilemap_layer(&self) -> Option<Gd<TileMapLayer>> {
        self.base()
            .get_node_or_null("World/TileMapLayer")
            .and_then(|node| node.try_cast::<TileMapLayer>().ok())
    }

    fn turn_label(&self) -> Option<Gd<Label>> {
        self.base()
            .get_node_or_null("UI/RightPanel/MarginContainer/Content/TurnValue")
            .and_then(|node| node.try_cast::<Label>().ok())
    }

    fn hero_mov_label(&self) -> Option<Gd<Label>> {
        self.base()
            .get_node_or_null("UI/RightPanel/MarginContainer/Content/HeroMovValue")
            .and_then(|node| node.try_cast::<Label>().ok())
    }

    fn update_turn_label(&self) {
        let Some(mut label) = self.turn_label() else { return };
        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let turn = gm.bind().get_turn();
        label.set_text(&format!("Ход: {turn}"));
    }

    fn update_hero_mov_label(&self) {
        let Some(mut label) = self.hero_mov_label() else { return };
        if self.selected_hero_id < 0 {
            label.set_text("Движение: —");
            return;
        }
        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let rem = gm.bind().get_hero_mov_remaining(self.selected_hero_id);
        let max = gm.bind().get_hero_mov_max(self.selected_hero_id);
        if rem >= 0 && max >= 0 {
            label.set_text(&format!("Движение: {rem}/{max}"));
        } else {
            label.set_text("Движение: —");
        }
    }

    fn version_label(&self) -> Option<Gd<Label>> {
        self.base()
            .get_node_or_null("UI/RightPanel/MarginContainer/Content/VersionLabel")
            .and_then(|node| node.try_cast::<Label>().ok())
    }

    fn update_version_label(&self) {
        if let Some(mut label) = self.version_label() {
            label.set_text(&build_info::version_string());
        }
    }
}
