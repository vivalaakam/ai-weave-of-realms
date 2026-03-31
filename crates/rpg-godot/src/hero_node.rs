//! `HeroNode` GDExtension node — visual representative of a hero on the map.
//!
//! Displays a sprite based on faction (player=hero, enemy=enemy).
//! Position is controlled by the parent scene via world coordinates.

use godot::classes::{ResourceLoader, Sprite2D, Texture2D};
use godot::prelude::*;

// ─── HeroNode ─────────────────────────────────────────────────────────────────

#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct HeroNode {
    base: Base<Node2D>,

    /// ID matching the hero in `GameManager`.
    #[var]
    pub hero_id: i64,

    /// `"player"` or `"enemy"`.
    #[var]
    pub faction: GString,

    /// Whether this hero can be selected by the player.
    #[var]
    pub selectable: bool,

    // Tile coordinates on the game map.
    tile_x: i64,
    tile_y: i64,

    // Child sprite node
    sprite_path: GString,
}

#[godot_api]
impl INode2D for HeroNode {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            hero_id: 0,
            faction: GString::from("player"),
            selectable: true,
            tile_x: 0,
            tile_y: 0,
            sprite_path: GString::default(),
        }
    }

    fn ready(&mut self) {
        self.setup_sprite();
    }
}

#[godot_api]
impl HeroNode {
    #[signal]
    fn move_requested(hero_id: i64, target_x: i64, target_y: i64);

    #[signal]
    fn selected(hero_id: i64);

    // ── Sprite setup ──────────────────────────────────────────────────────────

    /// Creates and adds a Sprite2D child based on faction.
    fn setup_sprite(&mut self) {
        let faction_str = self.faction.to_string();
        let texture_path = if faction_str == "enemy" {
            "res://assets/enemy.png"
        } else {
            "res://assets/hero.png"
        };

        // Load texture using the same pattern as MapNode
        let texture = match ResourceLoader::singleton().load(texture_path) {
            Some(res) => res,
            None => {
                tracing::warn!(path = texture_path, "failed to load hero sprite texture");
                return;
            }
        };

        // Cast to Texture2D
        let texture = match texture.try_cast::<Texture2D>() {
            Ok(tex) => tex,
            Err(_) => {
                tracing::warn!(path = texture_path, "failed to cast to Texture2D");
                return;
            }
        };

        // Create Sprite2D using GD type system
        let mut sprite: Gd<Sprite2D> = Sprite2D::new_alloc();
        sprite.set_texture(&texture);
        sprite.set_centered(true);
        
        // Position sprite offset (lift up so base aligns with tile center)
        sprite.set_offset(Vector2::new(0.0, -24.0));
        
        // Set z-index for proper layering (heroes above tiles)
        sprite.set_z_index(100);

        // Set modulate based on faction
        if faction_str == "enemy" {
            sprite.set_modulate(Color::from_rgba8(255, 100, 100, 255));
        }

        let sprite_name = if faction_str == "enemy" { "EnemySprite" } else { "HeroSprite" };
        self.base_mut().add_child(&sprite.clone());
        sprite.set_name(&StringName::from(sprite_name));
        
        self.sprite_path = GString::from(sprite_name);
    }

    // ── State updates from GameManager signals ────────────────────────────────

    /// Called when `GameManager.hero_moved` fires for this hero.
    #[func]
    pub fn on_hero_moved(
        &mut self,
        hero_id: i64,
        _from_x: i64,
        _from_y: i64,
        to_x: i64,
        to_y: i64,
    ) {
        if hero_id != self.hero_id {
            return;
        }
        self.tile_x = to_x;
        self.tile_y = to_y;
    }

    /// Called when `GameManager.hero_defeated` fires for this hero.
    #[func]
    pub fn on_hero_defeated(&mut self, hero_id: i64) {
        if hero_id == self.hero_id {
            self.base_mut().hide();
        }
    }

    // ── Queries ─────────────────────────────────────────────────────────────

    #[func]
    pub fn get_tile_x(&self) -> i64 {
        self.tile_x
    }

    #[func]
    pub fn get_tile_y(&self) -> i64 {
        self.tile_y
    }

    #[func]
    pub fn set_tile_position(&mut self, x: i64, y: i64) {
        self.tile_x = x;
        self.tile_y = y;
    }

    /// Emits `move_requested` toward `(x, y)`.
    #[func]
    pub fn request_move(&mut self, x: i64, y: i64) {
        if self.selectable {
            let id = self.hero_id;
            self.base_mut().emit_signal(
                "move_requested",
                &[id.to_variant(), x.to_variant(), y.to_variant()],
            );
        }
    }

    /// Emits `selected` for this hero.
    #[func]
    pub fn select(&mut self) {
        if self.selectable {
            let id = self.hero_id;
            self.base_mut().emit_signal("selected", &[id.to_variant()]);
        }
    }

    /// Returns whether this hero is an enemy.
    #[func]
    pub fn is_enemy(&self) -> bool {
        self.faction.to_string() == "enemy"
    }
}
