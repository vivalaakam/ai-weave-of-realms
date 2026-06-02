//! Shared tile-atlas handles.
//!
//! The tile atlas (`1_main.png`, a 49x23 grid of 16x16 tiles) is loaded once at
//! start-up into the [`TileAtlas`] resource so every screen — the map view, the
//! team-setup logos, hero selection — can draw sprites from the same texture and
//! layout instead of loading it independently.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use engine::config::TeamLogo;

/// Atlas columns (tiles per row).
pub const ATLAS_COLS: u32 = 49;
/// Atlas rows.
pub const ATLAS_ROWS: u32 = 23;
/// Atlas tile width in pixels.
pub const ATLAS_TILE_W: u32 = 16;
/// Atlas tile height in pixels.
pub const ATLAS_TILE_H: u32 = 16;

/// Handles to the shared tile atlas image and its layout.
#[derive(Resource, Clone)]
pub struct TileAtlas {
    /// Handle to the tile atlas image (`1_main.png`).
    pub image: Handle<Image>,
    /// Handle to the tile atlas layout.
    pub layout: Handle<TextureAtlasLayout>,
}

/// Cache of generated 16x16 images for bitmap team logos, keyed by team name.
///
/// The generated images are white-on-transparent silhouettes; the team colour is
/// applied at draw time via the sprite / image-node tint, so one image per team
/// shape is enough regardless of colour.
#[derive(Resource, Default)]
pub struct TeamLogoImages(pub HashMap<String, Handle<Image>>);

impl TeamLogoImages {
    /// Returns a cached handle to the bitmap logo image for `team_name`, creating
    /// it on first use. Returns `None` for tile-based logos (which are drawn from
    /// the atlas instead).
    pub fn handle(
        &mut self,
        images: &mut Assets<Image>,
        team_name: &str,
        logo: &TeamLogo,
    ) -> Option<Handle<Image>> {
        if !matches!(logo, TeamLogo::Bitmap(_)) {
            return None;
        }
        if let Some(handle) = self.0.get(team_name) {
            return Some(handle.clone());
        }
        let handle = images.add(bitmap_logo_image(logo));
        self.0.insert(team_name.to_string(), handle.clone());
        Some(handle)
    }
}

/// Builds a 16x16 RGBA image from a bitmap logo: set pixels are opaque white,
/// unset pixels are fully transparent.
fn bitmap_logo_image(logo: &TeamLogo) -> Image {
    let mut data = vec![0u8; 16 * 16 * 4];
    for y in 0..16u32 {
        for x in 0..16u32 {
            if logo.pixel(x, y) {
                let i = ((y * 16 + x) * 4) as usize;
                data[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
    }
    Image::new(
        Extent3d { width: 16, height: 16, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
}

/// Loads the atlas image and builds its layout on start-up.
pub struct TileAtlasPlugin;

impl Plugin for TileAtlasPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TeamLogoImages>().add_systems(Startup, load_tile_atlas);
    }
}

fn load_tile_atlas(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let image: Handle<Image> = asset_server.load("1_main.png");
    let layout = TextureAtlasLayout::from_grid(
        UVec2::new(ATLAS_TILE_W, ATLAS_TILE_H),
        ATLAS_COLS,
        ATLAS_ROWS,
        None,
        None,
    );
    let layout = atlas_layouts.add(layout);
    commands.insert_resource(TileAtlas { image, layout });
}
