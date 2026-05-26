use game::app::{AppLayout, AppScreen};
use game::prelude::render::{Rgb888, Size};
use game::render::{
    draw_app_screen, visible_tiles, AppRenderCache, AppTheme, InfoOverlayTheme, ListTheme,
    MapViewTheme, RenderConfig, SaveOverlayTheme, SplashTheme,
};
use game::Tiles;

pub const HEADER_HEIGHT: u32 = 28;
pub const FOOTER_HEIGHT: u32 = 16;
const MAP_RENDER_CONFIG: RenderConfig = RenderConfig { header_height: 28, footer_height: 16 };
/// Background color used by the framebuffer.
pub(crate) const BACKGROUND: Rgb888 = Rgb888::new(20, 22, 26);
const SPLASH_BACKGROUND: Rgb888 = Rgb888::new(36, 0, 72);
const TEXT: Rgb888 = Rgb888::new(235, 238, 242);
/// Render one frame into the provided framebuffer.
///
/// # Arguments
/// * `screen_size` — Logical dimensions of the framebuffer.
/// * `screen` — Current app screen state.
/// * `render_cache` — Reusable render cache owned by the caller.
/// * `framebuffer` — Target framebuffer for `embedded-graphics`.
pub fn render_frame(
    screen_size: Size,
    screen: &AppScreen,
    render_cache: &mut AppRenderCache,
    framebuffer: &mut crate::frame_buffer::Framebuffer,
) {
    draw_app_screen(
        framebuffer,
        screen_size,
        screen,
        render_cache,
        MAP_RENDER_CONFIG,
        app_theme(),
        app_layout(screen_size).list_rows,
        app_layout(screen_size).save_rows,
    );
}

/// Build the app layout from a logical screen size.
pub fn app_layout(screen_size: Size) -> AppLayout {
    let (map_visible_cols, map_visible_rows) = visible_tiles(screen_size, MAP_RENDER_CONFIG);
    AppLayout {
        list_rows: screen_size.height.saturating_sub(32) as usize / 14,
        save_rows: 4,
        map_visible_cols,
        map_visible_rows,
    }
}

/// Build the complete color/theme descriptor for the renderer.
fn app_theme() -> AppTheme<Rgb888> {
    AppTheme {
        splash: SplashTheme {
            background: SPLASH_BACKGROUND,
            text: TEXT,
            selected: Rgb888::new(255, 255, 120),
        },
        list: ListTheme {
            background: BACKGROUND,
            text: TEXT,
            selected_fill: Rgb888::new(40, 72, 40),
        },
        map_view: MapViewTheme {
            background: BACKGROUND,
            text: TEXT,
            selected_hero: Rgb888::new(255, 255, 120),
            hero: Rgb888::new(255, 255, 255),
            enemy_spawn: Rgb888::new(255, 120, 120),
            chest: Rgb888::new(248, 198, 66),
            tile_color,
            tile_sprite_color,
            team_color,
            city_marker: Rgb888::new(255, 255, 255),
            cursor: Rgb888::new(255, 255, 0),
        },
        save_overlay: SaveOverlayTheme {
            panel_fill: Rgb888::new(24, 26, 34),
            panel_stroke: Rgb888::new(80, 180, 255),
            text: TEXT,
            selected_fill: Rgb888::new(40, 72, 40),
        },
        info_overlay: InfoOverlayTheme {
            panel_fill: Rgb888::new(24, 26, 34),
            panel_stroke: Rgb888::new(248, 198, 66),
            text: TEXT,
        },
    }
}

/// Map a game tile to its base RGB color.
fn tile_color(tile: Tiles) -> Rgb888 {
    let (r, g, b) = tile.as_color();
    Rgb888::new(r, g, b)
}

/// Map a game tile to a slightly lighter variant for sprite rendering.
fn tile_sprite_color(tile: Tiles) -> Rgb888 {
    let (r, g, b) = tile.as_color();
    Rgb888::new(r.saturating_add(40), g.saturating_add(30), b.saturating_add(10))
}

/// Return a distinct color for each team.
fn team_color(team_id: usize) -> Rgb888 {
    match team_id {
        0 => Rgb888::new(220, 50, 50),
        1 => Rgb888::new(50, 100, 220),
        _ => Rgb888::new(140, 140, 140),
    }
}
