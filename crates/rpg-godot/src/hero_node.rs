//! `HeroNode` GDExtension node — visual representative of a hero on the map.
//!
//! Attach to a `Node2D` (or `Sprite2D`) in the scene.  Wire up the
//! `move_requested` signal to `GameManager.move_hero` from GDScript.
//!
//! ## Usage (GDScript)
//! ```gdscript
//! func _ready():
//!     $HeroNode.hero_id   = 1
//!     $HeroNode.faction   = "player"
//!     $HeroNode.move_requested.connect(func(x, y): gm.move_hero(1, x, y))
//! ```

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
        }
    }
}

#[godot_api]
impl HeroNode {
    #[signal]
    fn move_requested(hero_id: i64, target_x: i64, target_y: i64);

    #[signal]
    fn selected(hero_id: i64);

    // ── State updates from GameManager signals ────────────────────────────────

    /// Called when `GameManager.hero_moved` fires for this hero.
    ///
    /// Updates the tile position and smoothly moves the node to the new
    /// isometric pixel position (basic instant-move; tween in GDScript).
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
        // Caller (GDScript) handles repositioning the node in world space.
    }

    /// Called when `GameManager.hero_defeated` fires for this hero.
    ///
    /// Hides the node.  Destruction / animation is handled in GDScript.
    #[func]
    pub fn on_hero_defeated(&mut self, hero_id: i64) {
        if hero_id == self.hero_id {
            self.base_mut().hide();
        }
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Returns the current tile column.
    #[func]
    pub fn get_tile_x(&self) -> i64 {
        self.tile_x
    }

    /// Returns the current tile row.
    #[func]
    pub fn get_tile_y(&self) -> i64 {
        self.tile_y
    }

    /// Sets tile position without emitting any signal.
    #[func]
    pub fn set_tile_position(&mut self, x: i64, y: i64) {
        self.tile_x = x;
        self.tile_y = y;
    }

    /// Emits `move_requested` toward `(x, y)`.
    ///
    /// Includes `hero_id` so receivers can route without additional lookups.
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
}
