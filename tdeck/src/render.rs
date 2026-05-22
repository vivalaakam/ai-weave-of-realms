//! Rendering for the shared embedded app on T-Deck.

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::{RgbColor, Size};
use game::app::EmbeddedApp;
use game::render::{
    draw_app_screen, AppRenderCache, AppTheme, InfoOverlayTheme, ListTheme, MapViewTheme,
    SaveOverlayTheme, SplashTheme,
};

use crate::app;

/// Cached render state used to avoid repainting unchanged tiles.
pub type RenderCache = AppRenderCache;

/// Draws the active shared app screen to the display.
pub fn draw_screen<D>(
    display: &mut D,
    app_state: &EmbeddedApp,
    screen_size: Size,
    render_cache: &mut RenderCache,
) where
    D: DrawTarget<Color=Rgb565>,
{
    draw_app_screen(
        display,
        screen_size,
        app_state.screen(),
        render_cache,
        app::render_config(),
        app_theme(),
        app::selectable_rows(screen_size),
        4,
    );
}

fn tile_color(tile: rpg_engine::map::tile::Tiles) -> Rgb565 {
    let (r, g, b) = tile.as_color();
    Rgb565::new(r >> 3, g >> 2, b >> 3)
}

fn tile_sprite_color(tile: rpg_engine::map::tile::Tiles) -> Rgb565 {
    let (r, g, b) = tile.as_color();
    Rgb565::new(
        r.saturating_add(40) >> 3,
        g.saturating_add(30) >> 2,
        b.saturating_add(10) >> 3,
    )
}

fn team_color(team_id: usize) -> Rgb565 {
    match team_id {
        0 => Rgb565::new(27, 12, 12),
        1 => Rgb565::new(6, 12, 27),
        _ => Rgb565::new(20, 20, 20),
    }
}

fn app_theme() -> AppTheme<Rgb565> {
    AppTheme {
        splash: SplashTheme {
            background: Rgb565::MAGENTA,
            text: Rgb565::WHITE,
        },
        list: ListTheme {
            background: Rgb565::BLACK,
            text: Rgb565::WHITE,
            selected_fill: Rgb565::new(0, 18, 0),
        },
        map_view: MapViewTheme {
            background: Rgb565::BLACK,
            text: Rgb565::WHITE,
            selected_hero: Rgb565::YELLOW,
            hero: Rgb565::WHITE,
            enemy_spawn: Rgb565::RED,
            chest: Rgb565::YELLOW,
            tile_color,
            tile_sprite_color,
            team_color,
            city_marker: Rgb565::WHITE,
        },
        save_overlay: SaveOverlayTheme {
            panel_fill: Rgb565::new(3, 3, 6),
            panel_stroke: Rgb565::CYAN,
            text: Rgb565::WHITE,
            selected_fill: Rgb565::new(0, 18, 0),
        },
        info_overlay: InfoOverlayTheme {
            panel_fill: Rgb565::new(3, 3, 6),
            panel_stroke: Rgb565::YELLOW,
            text: Rgb565::WHITE,
        },
    }
}
