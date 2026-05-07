//! Rendering for all T-Deck screens.

use alloc::format;

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::{FONT_10X20, FONT_6X10};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::{Point, Primitive, RgbColor, Size};
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::{Alignment, Text};
use embedded_graphics::Drawable;
use rpg_embedded::render::{
    ListTheme, MapViewTheme, RenderCache as SharedRenderCache, RenderConfig, SplashTheme,
    draw_list_screen, draw_map_view as draw_shared_map_view, draw_splash_screen,
    reset_cache as reset_shared_cache, visible_tiles,
};

use crate::screens::{ListScreen, MapViewScreen, SaveOverlay, Screen, SplashScreen};

const MAP_RENDER_CONFIG: RenderConfig = RenderConfig {
    tile_width: 16,
    tile_height: 16,
    header_height: 22,
    footer_height: 12,
};

/// Cached render state used to avoid repainting unchanged tiles.
#[derive(Default)]
pub struct RenderCache {
    map_view: SharedRenderCache,
    overlay_visible: bool,
}

/// Draws the active screen to the display.
pub fn draw_screen<D>(
    display: &mut D,
    screen: &Screen,
    screen_size: Size,
    render_cache: &mut RenderCache,
) where
    D: DrawTarget<Color = Rgb565>,
{
    match screen {
        Screen::Splash(splash) => {
            reset_shared_cache(&mut render_cache.map_view);
            render_cache.overlay_visible = false;
            draw_splash(display, screen_size, splash);
        }
        Screen::MapSelect(map_select) => {
            reset_shared_cache(&mut render_cache.map_view);
            render_cache.overlay_visible = false;
            draw_map_select(display, screen_size, map_select);
        }
        Screen::SaveSelect(save_select) => {
            reset_shared_cache(&mut render_cache.map_view);
            render_cache.overlay_visible = false;
            draw_save_select(display, screen_size, save_select);
        }
        Screen::MapView(map_view) => {
            draw_map_view(display, screen_size, map_view.as_ref(), render_cache)
        }
    }
}

/// Returns the number of visible rows in the map selector.
pub fn selectable_rows(screen_size: Size) -> usize {
    screen_size.height.saturating_sub(32) as usize / 14
}

/// Returns the visible map window dimensions in tiles.
pub fn map_view_tiles(screen_size: Size) -> (usize, usize) {
    visible_tiles(screen_size, MAP_RENDER_CONFIG)
}

fn draw_splash<D>(display: &mut D, screen_size: Size, splash: &SplashScreen)
where
    D: DrawTarget<Color = Rgb565>,
{
    draw_splash_screen(
        display,
        screen_size,
        splash,
        "weave of realms",
        &["New Game", "Load Game"],
        "Enter: select  W/S: move",
        SplashTheme {
            background: Rgb565::MAGENTA,
            text: Rgb565::WHITE,
        },
    );
}

fn draw_map_select<D>(display: &mut D, screen_size: Size, map_select: &ListScreen)
where
    D: DrawTarget<Color = Rgb565>,
{
    draw_list_screen(
        display,
        screen_size,
        map_select,
        "Maps on /maps",
        "Up/Down: select  Enter: load  Back: splash",
        selectable_rows(screen_size),
        list_theme(),
    );
}

fn draw_save_select<D>(display: &mut D, screen_size: Size, save_select: &ListScreen)
where
    D: DrawTarget<Color = Rgb565>,
{
    draw_list_screen(
        display,
        screen_size,
        save_select,
        "Saves in /savegame",
        "Up/Down: select  Enter: load  Back: menu",
        selectable_rows(screen_size),
        list_theme(),
    );
}

fn draw_map_view<D>(
    display: &mut D,
    screen_size: Size,
    map_view: &MapViewScreen,
    render_cache: &mut RenderCache,
) where
    D: DrawTarget<Color = Rgb565>,
{
    let overlay_visible = map_view.info_overlay.is_some() || map_view.save_overlay.is_some();
    if render_cache.overlay_visible != overlay_visible {
        reset_shared_cache(&mut render_cache.map_view);
        render_cache.overlay_visible = overlay_visible;
    }

    draw_shared_map_view(
        display,
        screen_size,
        &map_view.app,
        &mut render_cache.map_view,
        MAP_RENDER_CONFIG,
        map_theme(),
    );

    if let Some(save_overlay) = &map_view.save_overlay {
        draw_save_overlay(display, screen_size, save_overlay);
    } else if let Some(info_overlay) = &map_view.info_overlay {
        draw_info_overlay(display, screen_size, info_overlay);
    }
}

fn draw_save_overlay<D>(display: &mut D, screen_size: Size, overlay: &SaveOverlay)
where
    D: DrawTarget<Color = Rgb565>,
{
    let box_width: u32 = 204;
    let box_height: u32 = 120;
    let origin_x = ((screen_size.width - box_width) / 2) as i32;
    let origin_y = ((screen_size.height - box_height) / 2) as i32;
    let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
    let selected_style = PrimitiveStyle::with_fill(Rgb565::new(0, 18, 0));

    halt_on_error(
        Rectangle::new(Point::new(origin_x, origin_y), Size::new(box_width, box_height))
            .into_styled(PrimitiveStyle::with_fill(Rgb565::new(3, 3, 6)))
            .draw(display),
    );
    halt_on_error(
        Rectangle::new(Point::new(origin_x, origin_y), Size::new(box_width, box_height))
            .into_styled(PrimitiveStyle::with_stroke(Rgb565::CYAN, 1))
            .draw(display),
    );

    match overlay {
        SaveOverlay::Menu { selected, status } => {
            halt_on_error(Text::new("Save Menu", Point::new(origin_x + 8, origin_y + 14), text_style).draw(display));
            let entries = ["Save Game", "Load Game", "Cancel"];
            for (idx, label) in entries.iter().enumerate() {
                let y = origin_y + 36 + (idx as i32 * 14);
                if idx == *selected {
                    halt_on_error(
                        Rectangle::new(Point::new(origin_x + 4, y - 9), Size::new(box_width - 8, 12))
                            .into_styled(selected_style)
                            .draw(display),
                    );
                }
                let prefix = if idx == *selected { ">" } else { " " };
                let line = format!("{prefix} {label}");
                halt_on_error(Text::new(&line, Point::new(origin_x + 8, y), text_style).draw(display));
            }
            halt_on_error(
                Text::new(
                    "Enter: select  Back: close",
                    Point::new(origin_x + 8, origin_y + 102),
                    text_style,
                )
                .draw(display),
            );
            if let Some(status) = status.as_deref() {
                halt_on_error(Text::new(status, Point::new(origin_x + 8, origin_y + 90), text_style).draw(display));
            }
        }
        SaveOverlay::SaveName { name, status } => {
            halt_on_error(Text::new("Save Name", Point::new(origin_x + 8, origin_y + 14), text_style).draw(display));
            let name_line = format!("> {name}_");
            halt_on_error(Text::new(&name_line, Point::new(origin_x + 8, origin_y + 38), text_style).draw(display));
            halt_on_error(
                Text::new(
                    "Type name  Enter: save  Back: delete/close",
                    Point::new(origin_x + 8, origin_y + 102),
                    text_style,
                )
                .draw(display),
            );
            if let Some(status) = status.as_deref() {
                halt_on_error(Text::new(status, Point::new(origin_x + 8, origin_y + 74), text_style).draw(display));
            }
        }
        SaveOverlay::LoadList {
            saves,
            selected,
            scroll,
            status,
        } => {
            halt_on_error(Text::new("Load Game", Point::new(origin_x + 8, origin_y + 14), text_style).draw(display));
            let max_rows = ((box_height - 56) / 14) as usize;
            let end = core::cmp::min(*scroll + max_rows, saves.len());
            for (row, entry_index) in ((*scroll)..end).enumerate() {
                let y = origin_y + 34 + (row as i32 * 14);
                if entry_index == *selected {
                    halt_on_error(
                        Rectangle::new(Point::new(origin_x + 4, y - 9), Size::new(box_width - 8, 12))
                            .into_styled(selected_style)
                            .draw(display),
                    );
                }
                let prefix = if entry_index == *selected { ">" } else { " " };
                let line = format!("{prefix} {}", saves[entry_index].display_name);
                halt_on_error(Text::new(&line, Point::new(origin_x + 8, y), text_style).draw(display));
            }
            halt_on_error(
                Text::new(
                    "Up/Down: select  Enter: load  Back: close",
                    Point::new(origin_x + 8, origin_y + 102),
                    text_style,
                )
                .draw(display),
            );
            if let Some(status) = status.as_deref() {
                halt_on_error(Text::new(status, Point::new(origin_x + 8, origin_y + 90), text_style).draw(display));
            }
        }
    }
}

fn draw_info_overlay<D>(
    display: &mut D,
    screen_size: Size,
    info_overlay: &crate::system_info::SystemInfoSnapshot,
) where
    D: DrawTarget<Color = Rgb565>,
{
    let box_width: u32 = 188;
    let box_height: u32 = 92;
    let origin_x = ((screen_size.width - box_width) / 2) as i32;
    let origin_y = ((screen_size.height - box_height) / 2) as i32;
    let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);

    halt_on_error(
        Rectangle::new(Point::new(origin_x, origin_y), Size::new(box_width, box_height))
            .into_styled(PrimitiveStyle::with_fill(Rgb565::new(3, 3, 6)))
            .draw(display),
    );
    halt_on_error(
        Rectangle::new(Point::new(origin_x, origin_y), Size::new(box_width, box_height))
            .into_styled(PrimitiveStyle::with_stroke(Rgb565::YELLOW, 1))
            .draw(display),
    );

    let battery_line = format!(
        "Battery: {}% ({} mV)",
        info_overlay.battery_percent, info_overlay.battery_mv
    );
    let ram_line = format!(
        "RAM: {}/{} KB",
        info_overlay.ram_used_bytes / 1024,
        info_overlay.ram_total_bytes / 1024
    );

    halt_on_error(Text::new("System Info", Point::new(origin_x + 8, origin_y + 14), text_style).draw(display));
    halt_on_error(Text::new(&battery_line, Point::new(origin_x + 8, origin_y + 34), text_style).draw(display));
    halt_on_error(Text::new(&ram_line, Point::new(origin_x + 8, origin_y + 48), text_style).draw(display));
    halt_on_error(Text::new("Enter or q: close", Point::new(origin_x + 8, origin_y + 74), text_style).draw(display));
}

fn tile_color(tile: rpg_engine::map::tile::Tiles) -> Rgb565 {
    let (r, g, b) = tile.as_color();
    Rgb565::new(r >> 3, g >> 2, b >> 3)
}

fn team_color(team_id: usize) -> Rgb565 {
    match team_id {
        0 => Rgb565::new(27, 12, 12),
        1 => Rgb565::new(6, 12, 27),
        _ => Rgb565::new(20, 20, 20),
    }
}

fn map_theme() -> MapViewTheme<Rgb565> {
    MapViewTheme {
        background: Rgb565::BLACK,
        text: Rgb565::WHITE,
        selected_hero: Rgb565::YELLOW,
        hero: Rgb565::WHITE,
        enemy_spawn: Rgb565::RED,
        chest: Rgb565::YELLOW,
        tile_color,
        team_color,
    }
}

fn list_theme() -> ListTheme<Rgb565> {
    ListTheme {
        background: Rgb565::BLACK,
        text: Rgb565::WHITE,
        selected_fill: Rgb565::new(0, 18, 0),
    }
}

fn halt_on_error<T, E>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(_) => loop {
            core::hint::spin_loop();
        },
    }
}
