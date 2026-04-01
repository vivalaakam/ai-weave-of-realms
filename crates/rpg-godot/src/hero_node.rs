//! `HeroNode` GDExtension node — visual representative of a hero on the map.
//!
//! This node holds only the hero's **identity** (`hero_id`, `team_name`,
//! `player_controlled`).  All mutable game state — position, HP, movement
//! points — is owned exclusively by the engine (`GameManager`) and must be
//! queried from there rather than cached here.

use godot::classes::{ResourceLoader, Sprite2D, Texture2D};
use godot::prelude::*;

// ─── HeroNode ─────────────────────────────────────────────────────────────────

#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct HeroNode {
    base: Base<Node2D>,

    /// ID matching the hero record in `GameManager`.
    #[var]
    pub hero_id: i64,

    /// Human-readable team name (e.g. `"player"`, `"enemy"`).
    /// Set once at spawn; does not change during the hero's lifetime.
    #[var]
    pub team_name: GString,

    /// Whether the human player commands this hero.
    /// Mirrors `Team::player_controlled` from the engine.
    /// Set once at spawn; does not change during the hero's lifetime.
    #[var]
    pub player_controlled: bool,
}

#[godot_api]
impl INode2D for HeroNode {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            hero_id: 0,
            team_name: GString::default(),
            player_controlled: false,
        }
    }

    fn ready(&mut self) {
        self.setup_sprite();
    }
}

#[godot_api]
impl HeroNode {
    #[signal]
    fn move_requested(hero_id: i64, direction: i64);

    #[signal]
    fn selected(hero_id: i64);

    // ── Sprite setup ──────────────────────────────────────────────────────────

    fn setup_sprite(&mut self) {
        let texture_path = if self.player_controlled {
            "res://assets/hero.png"
        } else {
            "res://assets/enemy.png"
        };

        let texture = match ResourceLoader::singleton().load(texture_path) {
            Some(res) => res,
            None => {
                tracing::warn!(path = texture_path, "failed to load hero sprite texture");
                return;
            }
        };

        let texture = match texture.try_cast::<Texture2D>() {
            Ok(tex) => tex,
            Err(_) => {
                tracing::warn!(path = texture_path, "failed to cast to Texture2D");
                return;
            }
        };

        let mut sprite: Gd<Sprite2D> = Sprite2D::new_alloc();
        sprite.set_texture(&texture);
        sprite.set_centered(true);
        sprite.set_offset(Vector2::new(0.0, -24.0));
        sprite.set_z_index(100);

        // Colorise the marker by team name so each faction is visually distinct.
        let modulate = match self.team_name.to_string().as_str() {
            "red"  => Color::from_rgba8(220, 50,  50,  255),
            "blue" => Color::from_rgba8(50,  100, 220, 255),
            _      => Color::from_rgba8(150, 80,  200, 255), // enemy / other
        };
        sprite.set_modulate(modulate);

        let sprite_name = if self.player_controlled {
            "HeroSprite"
        } else {
            "EnemySprite"
        };
        self.base_mut().add_child(&sprite.clone());
        sprite.set_name(&StringName::from(sprite_name));
    }

    // ── State updates ─────────────────────────────────────────────────────────

    /// Called when `GameManager.hero_defeated` fires for this hero.
    #[func]
    pub fn on_hero_defeated(&mut self, hero_id: i64) {
        if hero_id == self.hero_id {
            self.base_mut().hide();
        }
    }

    // ── Input ─────────────────────────────────────────────────────────────────

    /// Emits `move_requested` in the given direction.
    ///
    /// `direction` encoding: 0=North, 1=East, 2=South, 3=West.
    /// Only works if this hero is player-controlled.
    #[func]
    pub fn request_move(&mut self, direction: i64) {
        if self.player_controlled {
            let id = self.hero_id;
            self.base_mut()
                .emit_signal("move_requested", &[id.to_variant(), direction.to_variant()]);
        }
    }

    /// Emits `selected` for this hero.
    /// Only works if this hero is player-controlled.
    #[func]
    pub fn select(&mut self) {
        if self.player_controlled {
            let id = self.hero_id;
            self.base_mut().emit_signal("selected", &[id.to_variant()]);
        }
    }
}
