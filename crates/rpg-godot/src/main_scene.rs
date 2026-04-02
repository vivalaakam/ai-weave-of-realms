//! `MainScene` GDExtension node — scene root that wires all subsystems.
//!
//! Set as the root node type in `main.tscn` (`type="MainScene"`).

use godot::classes::{
    Button, Camera2D, ConfirmationDialog, INode, Input, InputEvent, InputEventJoypadButton,
    InputEventKey, InputEventMouseButton, Label, LineEdit, Node, Node2D, ResourceLoader, Sprite2D,
    Texture2D, TileMapLayer, VBoxContainer,
};
use godot::global::{JoyAxis, JoyButton, Key, MouseButton};
use godot::prelude::*;
use tracing::{info, warn};

use crate::build_info;
use crate::camera_controller::CameraController;
use crate::coords::TILE_W;
use crate::game_manager::GameManager;
use crate::hero_node::HeroNode;
use crate::map_node::MapNode;
use crate::tile_highlight::TileHighlight;

// ── Constants ─────────────────────────────────────────────────────────────────

const GENERATOR_PATH: &str = "res://scripts/generators/terrain.lua";
const DEFAULT_SEED: &str = "default-seed";
const MAP_WIDTH: i32 = 96;
const MAP_HEIGHT: i32 = 96;
const GAMEPAD_BTN_CROSS: JoyButton = JoyButton::A;
const GAMEPAD_BTN_CIRCLE: JoyButton = JoyButton::B;
const GAMEPAD_BTN_L1: JoyButton = JoyButton::LEFT_SHOULDER;
const GAMEPAD_BTN_R1: JoyButton = JoyButton::RIGHT_SHOULDER;
const GAMEPAD_BTN_DPAD_UP: JoyButton = JoyButton::DPAD_UP;
const GAMEPAD_BTN_DPAD_RIGHT: JoyButton = JoyButton::DPAD_RIGHT;
const GAMEPAD_BTN_DPAD_DOWN: JoyButton = JoyButton::DPAD_DOWN;
const GAMEPAD_BTN_DPAD_LEFT: JoyButton = JoyButton::DPAD_LEFT;
const LEFT_STICK_DEADZONE: f32 = 0.50;
const LEFT_STICK_REPEAT_INTERVAL: f64 = 0.14;
const CITY_MARKER_SIZE_PIXELS: f32 = 16.0;

// ─── MainScene ────────────────────────────────────────────────────────────────

#[derive(GodotClass)]
#[class(base=Node)]
pub struct MainScene {
    base: Base<Node>,
    end_turn_dialog: Option<Gd<ConfirmationDialog>>,
    hire_hero_dialog: Option<Gd<ConfirmationDialog>>,
    hire_target_tile: Vector2i,
    /// Team name for the city where the pending hire will take place.
    hire_target_team: String,
    left_stick_move_cooldown: f64,
    left_stick_direction_held: bool,
    cross_was_pressed: bool,
    circle_was_pressed: bool,
    gamepad_cursor_tile: Vector2i,
}

#[godot_api]
impl INode for MainScene {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            end_turn_dialog: None,
            hire_hero_dialog: None,
            hire_target_tile: Vector2i::new(-1, -1),
            hire_target_team: String::new(),
            left_stick_move_cooldown: 0.0,
            left_stick_direction_held: false,
            cross_was_pressed: false,
            circle_was_pressed: false,
            gamepad_cursor_tile: Vector2i::new(-1, -1),
        }
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
        let cb_map_ready = self.base().callable("_on_map_ready");
        let cb_hero_moved = self.base().callable("_on_hero_moved");
        let cb_combat_resolved = self.base().callable("_on_combat_resolved");
        let cb_hero_defeated = self.base().callable("_on_hero_defeated");
        let cb_turn_advanced = self.base().callable("_on_turn_advanced");
        gm.connect("map_ready", &cb_map_ready);
        gm.connect("hero_moved", &cb_hero_moved);
        gm.connect("combat_resolved", &cb_combat_resolved);
        gm.connect("hero_defeated", &cb_hero_defeated);
        gm.connect("turn_advanced", &cb_turn_advanced);
        let cb_enemies_spawned = self.base().callable("_on_enemies_spawned");
        gm.connect("enemies_spawned", &cb_enemies_spawned);
        let cb_city_changed = self.base().callable("_on_city_owner_changed");
        gm.connect("city_owner_changed", &cb_city_changed);

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
        self.base_mut()
            .call_deferred("_create_end_turn_dialog", &[]);

        // Create the hire-hero dialog (hidden initially).
        self.base_mut()
            .call_deferred("_create_hire_hero_dialog", &[]);

        // Defer startup so map_ready fires after ready() returns,
        // avoiding a double-borrow of MainScene.
        self.base_mut().call_deferred("_start_game_from_ui", &[]);
    }

    fn process(&mut self, delta: f64) {
        self.process_gamepad_dialog_buttons();
        self.process_gamepad_left_stick_cursor(delta);
        self.update_cursor_debug();
        self.update_zoom_debug();
        self.update_turn_label();
        self.update_hero_mov_label();
    }

    fn input(&mut self, event: Gd<InputEvent>) {
        // Пока открыт любой диалог — не обрабатываем собственные привязки.
        if self.is_any_dialog_visible() {
            return;
        }

        if let Ok(key) = event.clone().try_cast::<InputEventKey>() {
            if key.is_pressed() && !key.is_echo() {
                let physical = key.get_physical_keycode();
                let logical = key.get_keycode();

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
                if let Some(id) = self.get_selected_hero_id() {
                    // Encode Direction as i64: 0=North, 1=East, 2=South, 3=West
                    let direction: Option<i64> = if physical == Key::LEFT || logical == Key::LEFT {
                        Some(3)
                    } else if physical == Key::RIGHT || logical == Key::RIGHT {
                        Some(1)
                    } else if physical == Key::UP || logical == Key::UP {
                        Some(0)
                    } else if physical == Key::DOWN || logical == Key::DOWN {
                        Some(2)
                    } else {
                        None
                    };

                    if let Some(dir) = direction {
                        let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
                        gm.call_deferred("move_hero", &[id.to_variant(), dir.to_variant()]);
                        return;
                    }
                }
            }
        }

        if let Ok(mb) = event.clone().try_cast::<InputEventMouseButton>() {
            if mb.is_pressed() && mb.get_button_index() == MouseButton::LEFT {
                if let Some(tile) = self.mouse_to_tile(&mb) {
                    // Если на тайле стоит управляемый игроком герой — выбираем его.
                    // Если тайл — город/вход и там нет героя — предлагаем нанять.
                    // Иначе — перемещаем текущего выбранного героя.
                    if let Some(hero_id) = self.player_hero_at_tile(tile) {
                        self.set_selected_hero(hero_id, false);
                    } else {
                        let is_city = {
                            let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
                            let v = gm.bind().is_city_tile(tile.x as i64, tile.y as i64);
                            v
                        };
                        if is_city {
                            self.show_hire_hero_dialog(tile);
                        } else if let Some(id) = self.get_selected_hero_id() {
                            // Derive direction from hero position to clicked adjacent tile.
                            // 0=North, 1=East, 2=South, 3=West. Non-adjacent clicks are ignored.
                            let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
                            let pos = gm.bind().get_hero_position(id);
                            drop(gm);

                            let dx = tile.x - pos.x;
                            let dy = tile.y - pos.y;
                            let direction: Option<i64> = match (dx, dy) {
                                (0, -1) => Some(0), // North
                                (1, 0) => Some(1),  // East
                                (0, 1) => Some(2),  // South
                                (-1, 0) => Some(3), // West
                                _ => None,          // not adjacent — ignore
                            };

                            if let Some(dir) = direction {
                                let mut gm: Gd<GameManager> =
                                    self.base().get_node_as("GameManager");
                                gm.call_deferred("move_hero", &[id.to_variant(), dir.to_variant()]);
                            }
                        }
                    }
                }
            }
        }

        if let Ok(btn) = event.try_cast::<InputEventJoypadButton>() {
            if btn.is_pressed() {
                let button = btn.get_button_index();
                if button == GAMEPAD_BTN_R1 {
                    self.select_next_player_hero();
                } else if button == GAMEPAD_BTN_L1 {
                    self.show_end_turn_dialog();
                } else if button == GAMEPAD_BTN_CROSS {
                    // X: показать диалог найма, если курсор стоит на городе без героя игрока.
                    let tile = self.gamepad_cursor_tile;
                    if tile.x >= 0 && tile.y >= 0 {
                        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
                        let is_city = gm.bind().is_city_tile(tile.x as i64, tile.y as i64);
                        drop(gm);
                        if is_city && self.player_hero_at_tile(tile).is_none() {
                            self.show_hire_hero_dialog(tile);
                        }
                    }
                } else if let Some(id) = self.get_selected_hero_id() {
                    let direction: Option<i64> = if button == GAMEPAD_BTN_DPAD_LEFT {
                        Some(3)
                    } else if button == GAMEPAD_BTN_DPAD_RIGHT {
                        Some(1)
                    } else if button == GAMEPAD_BTN_DPAD_UP {
                        Some(0)
                    } else if button == GAMEPAD_BTN_DPAD_DOWN {
                        Some(2)
                    } else {
                        None
                    };
                    if let Some(dir) = direction {
                        let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
                        gm.call_deferred("move_hero", &[id.to_variant(), dir.to_variant()]);
                    }
                }
            }
        }
    }
}

#[godot_api]
impl MainScene {
    fn connected_gamepad_device_id() -> Option<i32> {
        let input = Input::singleton();
        let ids = input.get_connected_joypads();
        if ids.is_empty() {
            None
        } else {
            ids.get(0).map(|id| id as i32)
        }
    }

    fn process_gamepad_dialog_buttons(&mut self) {
        // Handle end-turn dialog first.
        if self.is_end_turn_dialog_visible() {
            let Some(device_id) = Self::connected_gamepad_device_id() else {
                self.cross_was_pressed = false;
                self.circle_was_pressed = false;
                return;
            };
            let input = Input::singleton();
            let cross_pressed = input.is_joy_button_pressed(device_id, GAMEPAD_BTN_CROSS);
            let circle_pressed = input.is_joy_button_pressed(device_id, GAMEPAD_BTN_CIRCLE);

            if cross_pressed && !self.cross_was_pressed {
                self._on_end_turn_confirmed();
                if let Some(mut dialog) = self.end_turn_dialog.clone() {
                    dialog.hide();
                }
            } else if circle_pressed && !self.circle_was_pressed {
                self._on_end_turn_cancelled();
                if let Some(mut dialog) = self.end_turn_dialog.clone() {
                    dialog.hide();
                }
            }

            self.cross_was_pressed = cross_pressed;
            self.circle_was_pressed = circle_pressed;
            return;
        }

        // Handle hire-hero dialog.
        if self.is_hire_hero_dialog_visible() {
            let Some(device_id) = Self::connected_gamepad_device_id() else {
                self.cross_was_pressed = false;
                self.circle_was_pressed = false;
                return;
            };
            let input = Input::singleton();
            let cross_pressed = input.is_joy_button_pressed(device_id, GAMEPAD_BTN_CROSS);
            let circle_pressed = input.is_joy_button_pressed(device_id, GAMEPAD_BTN_CIRCLE);

            if cross_pressed && !self.cross_was_pressed {
                self._on_hire_hero_confirmed();
                if let Some(mut dialog) = self.hire_hero_dialog.clone() {
                    dialog.hide();
                }
            } else if circle_pressed && !self.circle_was_pressed {
                self._on_hire_hero_cancelled();
                if let Some(mut dialog) = self.hire_hero_dialog.clone() {
                    dialog.hide();
                }
            }

            self.cross_was_pressed = cross_pressed;
            self.circle_was_pressed = circle_pressed;
            return;
        }

        self.cross_was_pressed = false;
        self.circle_was_pressed = false;
    }

    fn process_gamepad_left_stick_cursor(&mut self, delta: f64) {
        self.left_stick_move_cooldown = (self.left_stick_move_cooldown - delta).max(0.0);

        let Some(device_id) = Self::connected_gamepad_device_id() else {
            self.left_stick_direction_held = false;
            self.apply_gamepad_cursor_highlight();
            return;
        };
        let input = Input::singleton();
        let axis_x = input.get_joy_axis(device_id, JoyAxis::LEFT_X);
        let axis_y = input.get_joy_axis(device_id, JoyAxis::LEFT_Y);

        let direction = if axis_x <= -LEFT_STICK_DEADZONE && axis_x.abs() >= axis_y.abs() {
            Some(3) // West
        } else if axis_x >= LEFT_STICK_DEADZONE && axis_x.abs() >= axis_y.abs() {
            Some(1) // East
        } else if axis_y <= -LEFT_STICK_DEADZONE {
            Some(0) // North
        } else if axis_y >= LEFT_STICK_DEADZONE {
            Some(2) // South
        } else {
            None
        };

        let Some(direction) = direction else {
            self.left_stick_direction_held = false;
            return;
        };

        if self.left_stick_direction_held && self.left_stick_move_cooldown > 0.0 {
            return;
        }
        self.move_gamepad_cursor(direction);
        self.left_stick_direction_held = true;
        self.left_stick_move_cooldown = LEFT_STICK_REPEAT_INTERVAL;
    }

    fn move_gamepad_cursor(&mut self, direction: i64) {
        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let width = gm.bind().get_map_width() as i32;
        let height = gm.bind().get_map_height() as i32;
        if width <= 0 || height <= 0 {
            return;
        }

        if self.gamepad_cursor_tile.x < 0 || self.gamepad_cursor_tile.y < 0 {
            if let Some(id) = self.get_selected_hero_id() {
                self.gamepad_cursor_tile = gm.bind().get_hero_position(id);
            } else {
                self.gamepad_cursor_tile = Vector2i::new(width / 2, height / 2);
            }
        }

        let mut next = self.gamepad_cursor_tile;
        match direction {
            0 => next.y -= 1, // North
            1 => next.x += 1, // East
            2 => next.y += 1, // South
            3 => next.x -= 1, // West
            _ => return,
        }
        next.x = next.x.clamp(0, width - 1);
        next.y = next.y.clamp(0, height - 1);
        self.gamepad_cursor_tile = next;
        self.apply_gamepad_cursor_highlight();
    }

    fn apply_gamepad_cursor_highlight(&mut self) {
        if let Some(mut hl) = self.tile_highlight() {
            hl.bind_mut()
                .set_forced_active_tile(self.gamepad_cursor_tile);
        }
    }

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
        gm.bind_mut().clear_active_heroes();
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
        self.setup_city_markers();
        self.update_heroes_list();
        self.set_center_debug_inputs(MAP_WIDTH / 2, MAP_HEIGHT / 2);
        // Start with the first player team and begin their first turn.
        {
            let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
            gm.bind_mut().reset_active_team();
            gm.bind_mut().on_turn();
        }
        self.select_first_active_team_hero();
        self.gamepad_cursor_tile = Vector2i::new(MAP_WIDTH / 2, MAP_HEIGHT / 2);
        self.apply_gamepad_cursor_highlight();
        self.center_camera(tilemap);
    }

    #[func]
    fn _on_hero_moved(&mut self, hero_id: i64, tx: i64, ty: i64) {
        let Some(root) = self.base().get_node_or_null("World/Heroes") else {
            return;
        };
        for child in root.get_children().iter_shared() {
            if let Ok(mut hn) = child.try_cast::<HeroNode>() {
                if hn.bind().hero_id == hero_id {
                    if let Some(world_pos) =
                        self.map_tile_to_world(Vector2i::new(tx as i32, ty as i32))
                    {
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
        let Some(root) = self.base().get_node_or_null("World/Heroes") else {
            return;
        };
        for child in root.get_children().iter_shared() {
            if let Ok(mut hn) = child.try_cast::<HeroNode>() {
                if hn.bind().hero_id == hero_id {
                    hn.bind_mut().on_hero_defeated(hero_id);
                    break;
                }
            }
        }
        if self.get_selected_hero_id() == Some(hero_id) {
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
    fn _on_move_requested(&mut self, hero_id: i64, direction: i64) {
        let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        gm.call_deferred("move_hero", &[hero_id.to_variant(), direction.to_variant()]);
    }

    #[func]
    fn _on_hero_selected(&mut self, hero_id: i64) {
        self.set_selected_hero(hero_id, true);
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn spawn_heroes(&mut self) {
        // Find two maximally-distant city entrance tiles for the two player teams.
        let city_spawns: Vec<Vector2i> = {
            let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
            let arr = gm.bind().find_city_entrance_spawns(2);
            arr.iter_shared().collect()
        };

        // Fallback: use the legacy player-spawn helper when city data is missing.
        let fallback = self.player_spawn_position().unwrap_or(Vector2i::new(0, 0));
        let red_spawn = city_spawns.first().copied().unwrap_or(fallback);
        let blue_spawn = city_spawns.get(1).copied().unwrap_or_else(|| {
            // No second city: pick the point farthest from red_spawn on the map.
            let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
            let ep = gm.bind().get_enemy_spawn();
            if ep.x >= 0 {
                ep
            } else {
                fallback
            }
        });

        // Spawn Red hero (id = 1, team_id = 0).
        self.add_game_hero(1, "Красный", 100, 20, 10, 15, red_spawn, "red", true);
        self.create_hero_node(1, "red", true, red_spawn);
        {
            let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
            gm.call_deferred(
                "set_city_owner",
                &[
                    i64::from(red_spawn.x).to_variant(),
                    i64::from(red_spawn.y).to_variant(),
                    0i64.to_variant(), // TeamId 0 = Red
                ],
            );
        }

        // Spawn Blue hero (id = 2, team_id = 1).
        self.add_game_hero(2, "Синий", 100, 20, 10, 15, blue_spawn, "blue", true);
        self.create_hero_node(2, "blue", true, blue_spawn);
        {
            let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
            gm.call_deferred(
                "set_city_owner",
                &[
                    i64::from(blue_spawn.x).to_variant(),
                    i64::from(blue_spawn.y).to_variant(),
                    1i64.to_variant(), // TeamId 1 = Blue
                ],
            );
        }

        // Spawn enemies via Lua-driven spawner.
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
        let mut enemy_data: Vec<(i64, String, bool, Vector2i)> = Vec::new();
        {
            let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
            let enemy_ids = gm.bind().get_living_enemy_hero_ids();
            for id in enemy_ids.iter_shared() {
                let pos = gm.bind().get_hero_position(id);
                if pos.x >= 0 && pos.y >= 0 {
                    let team_name = gm.bind().get_hero_team_name(id).to_string();
                    let player_controlled = gm.bind().is_hero_player_controlled(id);
                    enemy_data.push((id, team_name, player_controlled, pos));
                }
            }
        }

        // Now create hero nodes (mutable borrow).
        for (hero_id, team_name, player_controlled, pos) in enemy_data {
            self.create_hero_node(hero_id, &team_name, player_controlled, pos);
        }

        self.update_heroes_list();
    }

    // ── City ownership markers ────────────────────────────────────────────────

    /// Creates or recreates `Sprite2D` ownership markers for every `City` (center)
    /// tile on the map.  Markers are parented under `World/CityMarkers`.
    fn setup_city_markers(&mut self) {
        // Ensure the CityMarkers container exists.
        if self.base().get_node_or_null("World/CityMarkers").is_none() {
            let mut container = Node2D::new_alloc();
            container.set_name(&StringName::from("CityMarkers"));
            let mut world = self.base().get_node_as::<Node>("World");
            world.add_child(&container.upcast::<Node>());
        }

        // Clear existing markers.
        let children: Vec<Gd<Node>> = self
            .base()
            .get_node_as::<Node>("World/CityMarkers")
            .get_children()
            .iter_shared()
            .collect();
        for mut child in children {
            let mut root = self.base().get_node_as::<Node>("World/CityMarkers");
            root.remove_child(&child);
            child.queue_free();
        }

        // Load the ownership icon texture.
        let texture = match ResourceLoader::singleton()
            .load("res://assets/owner.svg")
            .and_then(|r| r.try_cast::<Texture2D>().ok())
        {
            Some(t) => t,
            None => {
                warn!("setup_city_markers: failed to load res://assets/owner.svg");
                return;
            }
        };
        let texture_size = texture.get_size();
        if texture_size.x <= 0.0 || texture_size.y <= 0.0 {
            warn!("setup_city_markers: owner texture has invalid size");
            return;
        }
        let marker_scale = Vector2::new(
            CITY_MARKER_SIZE_PIXELS / texture_size.x,
            CITY_MARKER_SIZE_PIXELS / texture_size.y,
        );

        // Query city center coordinates.
        let city_coords: Vec<Vector2i> = {
            let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
            let v = gm.bind().get_city_center_coords().iter_shared().collect();
            v
        };

        for coord in city_coords {
            let owner = {
                let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
                let v = gm
                    .bind()
                    .get_city_owner(coord.x as i64, coord.y as i64)
                    .to_string();
                v
            };

            let mut sprite = Sprite2D::new_alloc();
            sprite.set_texture(&texture);
            sprite.set_centered(true);
            sprite.set_scale(marker_scale);
            // Float above the tile surface in isometric space.
            sprite.set_offset(Vector2::new(0.0, -20.0));
            sprite.set_z_index(50);
            sprite.set_modulate(team_marker_color(&owner));

            if let Some(world_pos) = self.map_tile_to_world(coord) {
                sprite.set_position(world_pos);
            }

            sprite.set_name(&StringName::from(&format!(
                "CityMarker_{}_{}",
                coord.x, coord.y
            )));

            let mut markers = self.base().get_node_as::<Node>("World/CityMarkers");
            markers.add_child(&sprite.upcast::<Node>());
        }
    }

    /// Fired when `GameManager` emits `city_owner_changed`.
    ///
    /// Updates the colour of the ownership marker at `(x, y)` to reflect the new
    /// `team_name`.  If the tile has no marker (e.g. it is a `CityEntrance`) the
    /// call is a silent no-op.
    #[func]
    fn _on_city_owner_changed(&mut self, x: i64, y: i64, team_name: GString) {
        let path = format!("World/CityMarkers/CityMarker_{}_{}", x, y);
        let Some(node) = self.base().get_node_or_null(&path) else {
            return; // entrance tile — no marker
        };
        let Ok(mut sprite) = node.try_cast::<Sprite2D>() else {
            return;
        };
        sprite.set_modulate(team_marker_color(&team_name.to_string()));
    }

    #[allow(clippy::too_many_arguments)]
    fn add_game_hero(
        &mut self,
        id: i64,
        name: &str,
        hp: i64,
        atk: i64,
        def: i64,
        spd: i64,
        pos: Vector2i,
        team_name: &str,
        player_controlled: bool,
    ) {
        let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        gm.bind_mut().add_hero(
            id,
            GString::from(name),
            hp,
            atk,
            def,
            spd,
            pos,
            GString::from(team_name),
            player_controlled,
        );
    }

    fn create_hero_node(
        &mut self,
        id: i64,
        team_name: &str,
        player_controlled: bool,
        pos: Vector2i,
    ) {
        let mut hero = HeroNode::new_alloc();
        let name = StringName::from(&format!("Hero_{}", id));
        hero.set_name(&name);
        {
            let mut b = hero.bind_mut();
            b.hero_id = id;
            b.team_name = GString::from(team_name);
            b.player_controlled = player_controlled;
        }
        if let Some(world_pos) = self.map_tile_to_world(pos) {
            hero.set_position(world_pos);
        }

        let cb_move = self.base().callable("_on_move_requested");
        let cb_sel = self.base().callable("_on_hero_selected");
        hero.connect("move_requested", &cb_move);
        hero.connect("selected", &cb_sel);

        let mut root = self.base().get_node_as::<Node>("World/Heroes");
        let hero_node: Gd<Node> = hero.upcast();
        root.add_child(&hero_node);
    }

    fn clear_hero_nodes(&mut self) {
        let Some(root) = self.base().get_node_or_null("World/Heroes") else {
            return;
        };
        // Collect first to avoid modifying the parent while iterating.
        let children: Vec<Gd<Node>> = root.get_children().iter_shared().collect();
        for mut child in children {
            // Detach immediately so _on_hero_moved cannot find stale nodes on this
            // frame before queue_free actually destroys them.
            let mut r = self.base().get_node_as::<Node>("World/Heroes");
            r.remove_child(&child);
            child.queue_free();
        }
    }

    fn center_camera(&mut self, tilemap: Gd<TileMapLayer>) {
        if let Some(id) = self.get_selected_hero_id() {
            self.focus_camera_on_hero(id);
            return;
        }

        let center =
            Self::tilemap_center(tilemap).unwrap_or_else(|| Vector2::new(0.0, TILE_W * 0.5));

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
        let Some(mut label) = self.cursor_value_label() else {
            return;
        };
        let Some(viewport) = self.base().get_viewport() else {
            return;
        };
        let text = if self.gamepad_cursor_tile.x >= 0 && self.gamepad_cursor_tile.y >= 0 {
            format!(
                "Cursor: {}, {} (gamepad)",
                self.gamepad_cursor_tile.x, self.gamepad_cursor_tile.y
            )
        } else {
            let mouse = viewport.get_mouse_position();
            match self.screen_to_tile(mouse) {
                Some(tile) => format!("Cursor: {}, {}", tile.x, tile.y),
                None => "Cursor: -, -".to_string(),
            }
        };
        label.set_text(&text);
    }

    fn update_zoom_debug(&self) {
        let Some(mut label) = self.zoom_value_label() else {
            return;
        };
        let Some(camera) = self.camera_controller() else {
            return;
        };
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

    fn add_hero_to_list(
        &self,
        gm: &Gd<GameManager>,
        hero_id: i64,
        name_prefix: &str,
        mut list: Gd<VBoxContainer>,
    ) {
        let pos = gm.bind().get_hero_position(hero_id);
        let is_alive = gm.bind().is_hero_alive(hero_id);

        let mut btn: Gd<Button> = Button::new_alloc();
        btn.set_name(&StringName::from(&format!("HeroBtn_{}", hero_id)));
        btn.set_text(&format!(
            "{} {} ({}:{})",
            name_prefix, hero_id, pos.x, pos.y
        ));
        btn.set_disabled(!is_alive);

        // Connect with hero_id as argument
        let cb = self
            .base()
            .callable("_on_hero_list_clicked")
            .bind(&[hero_id.to_variant()]);
        btn.connect("pressed", &cb);

        list.add_child(&btn);
    }

    /// Highlights the selected hero button in the UI list.
    fn highlight_selected_hero_in_list(&self) {
        let Some(list) = self.heroes_list() else {
            return;
        };
        let selected_id = self.get_selected_hero_id();

        for child in list.get_children().iter_shared() {
            if let Ok(mut btn) = child.try_cast::<Button>() {
                let btn_name = btn.get_name().to_string();
                // Extract hero_id from button name "HeroBtn_{id}"
                let hero_id_str = btn_name.strip_prefix("HeroBtn_");
                let is_selected = hero_id_str
                    .and_then(|s| s.parse::<i64>().ok())
                    .map(|id| Some(id) == selected_id)
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
        if let Some(mut btn) = dialog.get_ok_button() {
            btn.set_text("Да");
        }
        if let Some(mut btn) = dialog.get_cancel_button() {
            btn.set_text("Нет");
        }

        let cb_confirmed = self.base().callable("_on_end_turn_confirmed");
        let cb_cancelled = self.base().callable("_on_end_turn_cancelled");
        dialog.connect("confirmed", &cb_confirmed);
        dialog.connect("canceled", &cb_cancelled);

        self.base_mut().add_child(&dialog.clone());
        self.end_turn_dialog = Some(dialog);
    }

    /// Показывает диалог подтверждения завершения хода текущей команды.
    fn show_end_turn_dialog(&mut self) {
        if let Some(mut dialog) = self.end_turn_dialog.clone() {
            let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
            let team = team_display_name(&gm.bind().get_active_team().to_string());
            dialog.set_text(&format!(
                "Завершить ход команды {}?\nПосле этого ход перейдёт к следующей команде.",
                team
            ));
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

    /// Вызывается при нажатии «Да» в диалоге завершения хода.
    ///
    /// Переключает активную команду на следующую.  После того как все команды
    /// игрока сходили, вызывает `advance_turn` (сброс AI-движения, счётчик глобального хода)
    /// и возвращает ход первой команде.  Сброс движения игровых команд выполняется в `on_turn`.
    #[func]
    fn _on_end_turn_confirmed(&mut self) {
        self.advance_active_team();
        self.update_heroes_list();
    }

    /// Вызывается при нажатии «Нет» или Esc в диалоге.
    #[func]
    fn _on_end_turn_cancelled(&mut self) {
        // Диалог уже закрылся сам — дополнительных действий не требуется.
    }

    // ── Диалог найма героя ────────────────────────────────────────────────────

    /// Создаёт диалог найма нового героя и скрывает его.
    ///
    /// Вызывается deferred из `ready()`.
    #[func]
    fn _create_hire_hero_dialog(&mut self) {
        let mut dialog = ConfirmationDialog::new_alloc();
        dialog.set_title("Нанять героя");
        dialog.set_text("В городе нет героя. Нанять нового героя здесь?");
        if let Some(mut btn) = dialog.get_ok_button() {
            btn.set_text("Нанять");
        }
        if let Some(mut btn) = dialog.get_cancel_button() {
            btn.set_text("Отмена");
        }

        let cb_confirmed = self.base().callable("_on_hire_hero_confirmed");
        let cb_cancelled = self.base().callable("_on_hire_hero_cancelled");
        dialog.connect("confirmed", &cb_confirmed);
        dialog.connect("canceled", &cb_cancelled);

        self.base_mut().add_child(&dialog.clone());
        self.hire_hero_dialog = Some(dialog);
    }

    /// Показывает диалог найма героя для тайла `tile`.
    ///
    /// Найм разрешён только в городах, принадлежащих команде игрока (Red/Blue).
    /// Нейтральные и вражеские города игнорируются.
    fn show_hire_hero_dialog(&mut self, tile: Vector2i) {
        // Determine which team owns this city.
        let owner = {
            let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
            let v = gm
                .bind()
                .get_city_owner(tile.x as i64, tile.y as i64)
                .to_string();
            v
        };

        // Hiring is only allowed at cities owned by the currently active player team.
        let active_team: String = {
            let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
            let x = gm.bind().get_active_team().to_string();
            x
        };
        if owner != active_team {
            return;
        }

        self.hire_target_tile = tile;
        self.hire_target_team = owner.clone();

        // Update dialog text to show which team will gain the new hero.
        if let Some(mut dialog) = self.hire_hero_dialog.clone() {
            let team_display = team_display_name(&owner);
            dialog.set_text(&format!(
                "В городе нет героя.\nНанять нового героя для команды {team_display}?"
            ));
            dialog.popup_centered();
        }
    }

    /// Возвращает `true`, если диалог найма героя сейчас виден.
    fn is_hire_hero_dialog_visible(&self) -> bool {
        self.hire_hero_dialog
            .as_ref()
            .map(|d| d.is_visible())
            .unwrap_or(false)
    }

    /// Возвращает `true`, если любой модальный диалог сейчас виден.
    fn is_any_dialog_visible(&self) -> bool {
        self.is_end_turn_dialog_visible() || self.is_hire_hero_dialog_visible()
    }

    /// Вызывается при нажатии «Нанять» в диалоге.
    ///
    /// Создаёт нового героя для команды, которой принадлежит город,
    /// и добавляет его в сцену.
    #[func]
    fn _on_hire_hero_confirmed(&mut self) {
        let tile = self.hire_target_tile;
        let team = self.hire_target_team.clone();
        if tile.x < 0 || tile.y < 0 || team.is_empty() {
            return;
        }
        let new_id = {
            let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
            let v = gm.bind().get_next_hero_id();
            v
        };
        let name = format!("Герой {new_id}");
        self.add_game_hero(new_id, &name, 100, 20, 10, 15, tile, &team, true);
        self.create_hero_node(new_id, &team, true, tile);
        self.update_heroes_list();
        info!(hero_id = new_id, team = %team, x = tile.x, y = tile.y, "hired new hero at city");
    }

    /// Вызывается при нажатии «Отмена» или Esc в диалоге найма.
    #[func]
    fn _on_hire_hero_cancelled(&mut self) {
        // Диалог уже закрылся сам — дополнительных действий не требуется.
    }

    /// Возвращает id управляемого активной командой героя, стоящего на тайле `tile`,
    /// или `None` если такого нет.
    fn player_hero_at_tile(&self, tile: Vector2i) -> Option<i64> {
        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let hero_id = gm
            .bind()
            .get_hero_id_at_position(tile.x as i64, tile.y as i64);
        if hero_id < 0 {
            return None;
        }
        let controlled = gm.bind().is_hero_player_controlled(hero_id);
        let team = gm.bind().get_hero_team_name(hero_id).to_string();
        let active_team = gm.bind().get_active_team().to_string();
        if controlled && team == active_team {
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
        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let team_id = gm.bind().get_active_team_id();
        let next_id = gm.bind().get_next_hero(team_id);
        if next_id >= 0 {
            self.set_selected_hero(next_id, true);
        }
    }

    fn set_selected_hero(&mut self, hero_id: i64, focus_camera: bool) {
        let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let team_id = gm.bind().get_active_team_id();
        gm.bind_mut().set_active_hero(team_id, hero_id);
        self.highlight_selected_hero_in_list();
        if focus_camera {
            self.focus_camera_on_hero(hero_id);
        }
    }

    /// Returns the currently selected hero ID for the active team, or None if none selected.
    fn get_selected_hero_id(&self) -> Option<i64> {
        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let team_id = gm.bind().get_active_team_id();
        let id = gm.bind().get_active_hero(team_id);
        if id >= 0 { Some(id) } else { None }
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

    /// Selects the first living hero of the active team, if any.
    fn select_first_active_team_hero(&mut self) {
        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let team_id = gm.bind().get_active_team_id();
        let next_id = gm.bind().get_next_hero(team_id);
        if next_id >= 0 {
            self.set_selected_hero(next_id, true);
        }
    }

    /// Advances to the next player team's phase, or — when all player teams have
    /// moved — calls `advance_turn` (global turn counter + AI movement reset) and
    /// returns to the first player team.  Player movement is reset via `on_turn`.
    fn advance_active_team(&mut self) {
        let mut gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let should_advance_turn = gm.bind_mut().get_next_active_team();
        if should_advance_turn {
            gm.call_deferred("advance_turn", &[]);
        }
        gm.bind_mut().on_turn();
        drop(gm);

        // Try to restore the last active hero for the new team.
        // If none set or hero is dead, select the first living hero.
        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let team_id = gm.bind().get_active_team_id();
        let last_id = gm.bind().get_active_hero(team_id);
        if last_id >= 0 && gm.bind().is_hero_alive(last_id) {
            self.set_selected_hero(last_id, true);
        } else {
            self.select_first_active_team_hero();
        }
        info!(active_team = %gm.bind().get_active_team().to_string(), "team turn started");
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

    fn tile_highlight(&self) -> Option<Gd<TileHighlight>> {
        self.base()
            .get_node_or_null("World/TileHighlight")
            .and_then(|node| node.try_cast::<TileHighlight>().ok())
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
        let local = tilemap
            .call("map_to_local", &[tile.to_variant()])
            .try_to::<Vector2>()
            .ok()?;
        Some(tilemap.to_global(local))
    }

    fn world_to_map_tile(&self, world: Vector2) -> Option<Vector2i> {
        let mut tilemap = self.tilemap_layer()?;
        let local = tilemap.to_local(world);
        tilemap
            .call("local_to_map", &[local.to_variant()])
            .try_to::<Vector2i>()
            .ok()
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
        let Some(mut label) = self.turn_label() else {
            return;
        };
        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let turn = gm.bind().get_turn();
        let team = team_display_name(&gm.bind().get_active_team().to_string());
        label.set_text(&format!("Ход: {turn} · {team}"));
    }

    fn update_hero_mov_label(&self) {
        let Some(mut label) = self.hero_mov_label() else {
            return;
        };
        let Some(hero_id) = self.get_selected_hero_id() else {
            label.set_text("Движение: —");
            return;
        };
        let gm: Gd<GameManager> = self.base().get_node_as("GameManager");
        let rem = gm.bind().get_hero_mov_remaining(hero_id);
        let max = gm.bind().get_hero_mov_max(hero_id);
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

// ── Module-level helpers ──────────────────────────────────────────────────────

/// Returns a human-readable Russian display name for a team identifier.
fn team_display_name(team: &str) -> &'static str {
    match team {
        "red" => "Красных",
        "blue" => "Синих",
        _ => "Врагов",
    }
}

/// Returns the `Color` used for city ownership markers of a given team.
///
/// Neutral (empty `team`) returns a dim grey; player teams use their signature
/// colours with slight transparency so the underlying tile remains visible.
fn team_marker_color(team: &str) -> Color {
    match team {
        "red" => Color::from_rgba8(220, 50, 50, 210),
        "blue" => Color::from_rgba8(50, 100, 220, 210),
        _ => Color::from_rgba8(180, 180, 180, 120), // neutral / unknown
    }
}
