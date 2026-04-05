//! Rendering for all T-Deck screens.

use alloc::{format, string::{String, ToString}, vec::Vec};

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::{FONT_10X20, FONT_6X10};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::{Point, Primitive, RgbColor, Size};
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::{Alignment, Text};
use embedded_graphics::Drawable;
use rpg_engine::hero::HeroId;
use rpg_engine::map::game_map::MapCoord;
use rpg_engine::map::tile::Tiles;

use crate::screens::{InteractionMode, MapSelectScreen, MapViewScreen, Screen, SplashScreen};

const TILE_SIZE: i32 = 16;
const HEADER_HEIGHT: i32 = 22;
const FOOTER_HEIGHT: i32 = 12;
const EMPTY_TILE: u32 = u32::MAX;

/// Cached render state used to avoid repainting unchanged tiles.
#[derive(Default)]
pub struct RenderCache {
    map_view: Option<MapViewCache>,
}

struct MapViewCache {
    map_name: String,
    map_width: usize,
    map_height: usize,
    visible_cols: usize,
    visible_rows: usize,
    visible_cells: Vec<u32>,
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
            render_cache.map_view = None;
            draw_splash(display, screen_size, splash);
        }
        Screen::MapSelect(map_select) => {
            render_cache.map_view = None;
            draw_map_select(display, screen_size, map_select);
        }
        Screen::MapView(map_view) => draw_map_view(display, screen_size, map_view, render_cache),
    }
}

/// Returns the number of visible rows in the map selector.
pub fn selectable_rows(screen_size: Size) -> usize {
    screen_size.height.saturating_sub(32) as usize / 14
}

/// Returns the visible map window dimensions in tiles.
pub fn map_view_tiles(screen_size: Size) -> (usize, usize) {
    let usable_height = screen_size.height as i32 - HEADER_HEIGHT - FOOTER_HEIGHT;
    (
        (screen_size.width as i32 / TILE_SIZE).max(0) as usize,
        (usable_height / TILE_SIZE).max(0) as usize,
    )
}

fn draw_splash<D>(display: &mut D, screen_size: Size, splash: &SplashScreen)
where
    D: DrawTarget<Color = Rgb565>,
{
    halt_on_error(display.clear(Rgb565::MAGENTA));

    let title_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
    let body_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);

    halt_on_error(
        Text::with_alignment(
            "weave of realms",
            Point::new((screen_size.width / 2) as i32, (screen_size.height / 2) as i32 - 24),
            title_style,
            Alignment::Center,
        )
        .draw(display),
    );

    halt_on_error(
        Text::with_alignment(
            "Press Enter for map select",
            Point::new((screen_size.width / 2) as i32, (screen_size.height / 2) as i32 + 8),
            body_style,
            Alignment::Center,
        )
        .draw(display),
    );

    halt_on_error(
        Text::with_alignment(
            "Trackball click also confirms",
            Point::new((screen_size.width / 2) as i32, (screen_size.height / 2) as i32 + 22),
            body_style,
            Alignment::Center,
        )
        .draw(display),
    );

    if let Some(status_text) = splash.status.as_deref() {
        halt_on_error(
            Text::with_alignment(
                status_text,
                Point::new((screen_size.width / 2) as i32, screen_size.height as i32 - 14),
                body_style,
                Alignment::Center,
            )
            .draw(display),
        );
    }
}

fn draw_map_select<D>(display: &mut D, screen_size: Size, map_select: &MapSelectScreen)
where
    D: DrawTarget<Color = Rgb565>,
{
    halt_on_error(display.clear(Rgb565::BLACK));

    let title_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
    let selected_style = PrimitiveStyle::with_fill(Rgb565::new(0, 18, 0));
    let line_height: i32 = 14;
    let start_y: i32 = 18;

    halt_on_error(Text::new("Maps on /maps", Point::new(6, 10), title_style).draw(display));

    let max_rows = selectable_rows(screen_size);
    let end = core::cmp::min(map_select.scroll + max_rows, map_select.maps.len());

    for (row, entry_index) in (map_select.scroll..end).enumerate() {
        let y = start_y + (row as i32 * line_height);
        if entry_index == map_select.selected {
            halt_on_error(
                Rectangle::new(Point::new(2, y - 9), Size::new(screen_size.width - 4, 12))
                    .into_styled(selected_style)
                    .draw(display),
            );
        }

        let prefix = if entry_index == map_select.selected {
            ">"
        } else {
            " "
        };
        let line = format!(
            "{} {} ({}b)",
            prefix,
            map_select.maps[entry_index].display_name,
            map_select.maps[entry_index].size_bytes
        );
        halt_on_error(Text::new(&line, Point::new(6, y), title_style).draw(display));
    }

    let footer = "Up/Down: select  Enter: load  Back: splash";
    halt_on_error(
        Text::new(
            footer,
            Point::new(4, screen_size.height as i32 - 2),
            title_style,
        )
        .draw(display),
    );

    if let Some(status_text) = map_select.status.as_deref() {
        halt_on_error(
            Text::new(
                status_text,
                Point::new(4, screen_size.height as i32 - 14),
                title_style,
            )
            .draw(display),
        );
    }
}

fn draw_map_view<D>(
    display: &mut D,
    screen_size: Size,
    map_view: &MapViewScreen,
    render_cache: &mut RenderCache,
) where
    D: DrawTarget<Color = Rgb565>,
{
    let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
    let mode_name = match map_view.mode {
        InteractionMode::Pan => "PAN",
        InteractionMode::Hero => "HERO",
    };
    let header = format!(
        "{} {} @{},{}",
        map_view.session.map_name(),
        mode_name,
        map_view.view_x,
        map_view.view_y
    );

    let origin_x = 0;
    let origin_y = HEADER_HEIGHT;
    let map_width_px = screen_size.width as i32;
    let map_height_px = screen_size.height as i32 - HEADER_HEIGHT - FOOTER_HEIGHT;
    let visible_cols = (map_width_px / TILE_SIZE).max(0) as usize;
    let visible_rows = (map_height_px / TILE_SIZE).max(0) as usize;
    let map = &map_view.session.state().map;
    let map_width = map.tile_width() as usize;
    let map_height = map.tile_height() as usize;

    let requires_full_redraw = match render_cache.map_view.as_ref() {
        Some(cache) => {
            cache.map_name != map_view.session.map_name()
                || cache.map_width != map_width
                || cache.map_height != map_height
                || cache.visible_cols != visible_cols
                || cache.visible_rows != visible_rows
                || cache.overlay_visible != map_view.info_overlay.is_some()
        }
        None => true,
    };

    if requires_full_redraw {
        halt_on_error(display.clear(Rgb565::BLACK));
        render_cache.map_view = Some(MapViewCache {
            map_name: map_view.session.map_name().to_string(),
            map_width,
            map_height,
            visible_cols,
            visible_rows,
            visible_cells: alloc::vec![EMPTY_TILE; visible_cols * visible_rows],
            overlay_visible: map_view.info_overlay.is_some(),
        });
    }

    let cache = render_cache
        .map_view
        .as_mut()
        .expect("map view cache must exist before drawing");
    cache.overlay_visible = map_view.info_overlay.is_some();

    clear_band(
        display,
        Rectangle::new(Point::new(0, 0), Size::new(screen_size.width, HEADER_HEIGHT as u32)),
    );
    halt_on_error(Text::new(&header, Point::new(4, 10), text_style).draw(display));

    for row in 0..visible_rows {
        for col in 0..visible_cols {
            let map_x = map_view.view_x + col;
            let map_y = map_view.view_y + row;
            let coord = MapCoord::new(map_x as u32, map_y as u32);
            let cell = if map_x < map_width && map_y < map_height {
                cell_signature(map_view, coord)
            } else {
                EMPTY_TILE
            };
            let cache_index = row * visible_cols + col;
            if cache.visible_cells[cache_index] == cell {
                continue;
            }

            cache.visible_cells[cache_index] = cell;
            let x = origin_x + (col as i32 * TILE_SIZE);
            let y = origin_y + (row as i32 * TILE_SIZE);
            draw_cell(display, map_view, coord, Point::new(x, y));
        }
    }

    let footer = match map_view.mode {
        InteractionMode::Pan => "I: info  Enter: hero mode  WASD: pan  Back: maps",
        InteractionMode::Hero => "I: info  Enter: pan mode  WASD: move hero  Back: maps",
    };
    clear_band(
        display,
        Rectangle::new(
            Point::new(0, screen_size.height as i32 - FOOTER_HEIGHT),
            Size::new(screen_size.width, FOOTER_HEIGHT as u32),
        ),
    );
    halt_on_error(
        Text::new(
            footer,
            Point::new(4, screen_size.height as i32 - 2),
            text_style,
        )
        .draw(display),
    );

    if let Some(status_text) = map_view.status.as_deref() {
        halt_on_error(
            Text::new(
                status_text,
                Point::new(4, screen_size.height as i32 - 14),
                text_style,
            )
            .draw(display),
        );
    }

    let summary = map_view.session.summary();
    halt_on_error(
        Text::new(&summary, Point::new(4, HEADER_HEIGHT - 2), text_style).draw(display),
    );

    if let Some(info_overlay) = &map_view.info_overlay {
        draw_info_overlay(display, screen_size, info_overlay);
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
    let close_line = "Enter or q: close";

    halt_on_error(Text::new("System Info", Point::new(origin_x + 8, origin_y + 14), text_style).draw(display));
    halt_on_error(Text::new(&battery_line, Point::new(origin_x + 8, origin_y + 34), text_style).draw(display));
    halt_on_error(Text::new(&ram_line, Point::new(origin_x + 8, origin_y + 48), text_style).draw(display));
    halt_on_error(Text::new(close_line, Point::new(origin_x + 8, origin_y + 74), text_style).draw(display));
}

fn draw_cell<D>(display: &mut D, map_view: &MapViewScreen, coord: MapCoord, top_left: Point)
where
    D: DrawTarget<Color = Rgb565>,
{
    let map = &map_view.session.state().map;
    let Ok(tile) = map.get_tile(coord) else {
        return;
    };

    halt_on_error(
        Rectangle::new(
            top_left,
            Size::new((TILE_SIZE - 1) as u32, (TILE_SIZE - 1) as u32),
        )
        .into_styled(PrimitiveStyle::with_fill(tile_color(tile.kind)))
        .draw(display),
    );

    if let Some(team_id) = map_view.session.state().city_owner(coord) {
        halt_on_error(
            Rectangle::new(top_left + Point::new(1, 1), Size::new(4, 4))
                .into_styled(PrimitiveStyle::with_fill(team_color(team_id as usize)))
                .draw(display),
        );
    }

    if let Some(hero) = map_view.session.state().hero_at(coord) {
        draw_hero_marker(display, hero.get_id(), map_view.session.selected_hero_id(), top_left);
    }
}

fn draw_hero_marker<D>(
    display: &mut D,
    hero_id: HeroId,
    selected_hero_id: HeroId,
    top_left: Point,
) where
    D: DrawTarget<Color = Rgb565>,
{
    let color = if hero_id == selected_hero_id {
        Rgb565::YELLOW
    } else {
        Rgb565::WHITE
    };
    halt_on_error(
        Rectangle::new(top_left + Point::new(4, 4), Size::new(7, 7))
            .into_styled(PrimitiveStyle::with_fill(color))
            .draw(display),
    );
}

fn cell_signature(map_view: &MapViewScreen, coord: MapCoord) -> u32 {
    let map = &map_view.session.state().map;
    let Ok(tile) = map.get_tile(coord) else {
        return EMPTY_TILE;
    };

    let mut signature = tile.kind.to_gid();
    if let Some(hero) = map_view.session.state().hero_at(coord) {
        signature |= (u32::from(hero.get_id()) + 1) << 16;
    }
    if let Some(team_id) = map_view.session.state().city_owner(coord) {
        signature |= (u32::from(team_id) + 1) << 24;
    }
    signature
}

fn tile_color(tile: Tiles) -> Rgb565 {
    let (r, g, b) = tile.as_color();
    Rgb565::new((r >> 3) as u8, (g >> 2) as u8, (b >> 3) as u8)
}

fn team_color(team_id: usize) -> Rgb565 {
    match team_id {
        0 => Rgb565::new(27, 12, 12),
        1 => Rgb565::new(6, 12, 27),
        _ => Rgb565::new(20, 20, 20),
    }
}

fn clear_band<D>(display: &mut D, rect: Rectangle)
where
    D: DrawTarget<Color = Rgb565>,
{
    halt_on_error(
        rect.into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
            .draw(display),
    );
}

fn halt_on_error<T, E>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(_) => loop {
            core::hint::spin_loop();
        },
    }
}
