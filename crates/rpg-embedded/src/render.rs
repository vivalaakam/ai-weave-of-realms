//! Shared `embedded-graphics` rendering for the gameplay map view.

use alloc::{format, string::ToString, vec, vec::Vec};

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::PixelColor;
use embedded_graphics::prelude::{Point, Primitive, Size};
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;
use embedded_graphics::Drawable;
use rpg_engine::hero::HeroId;
use rpg_engine::map::game_map::MapCoord;
use rpg_engine::map::tile::Tiles;

use crate::map_view::{InteractionMode, MapViewApp};
use crate::list::ListScreen;
use crate::splash::SplashScreen;

const EMPTY_TILE: u32 = u32::MAX;

/// Shared rendering geometry for the gameplay map view.
#[derive(Clone, Copy)]
pub struct RenderConfig {
    /// Width of one tile in pixels.
    pub tile_width: u32,
    /// Height of one tile in pixels.
    pub tile_height: u32,
    /// Header band height in pixels.
    pub header_height: u32,
    /// Footer band height in pixels.
    pub footer_height: u32,
}

/// Shared theme colors for gameplay rendering.
#[derive(Clone, Copy)]
pub struct MapViewTheme<C>
where
    C: PixelColor + Copy,
{
    /// Background fill color.
    pub background: C,
    /// HUD text color.
    pub text: C,
    /// Selected hero marker color.
    pub selected_hero: C,
    /// Unselected hero marker color.
    pub hero: C,
    /// Enemy spawn marker color.
    pub enemy_spawn: C,
    /// Chest marker color.
    pub chest: C,
    /// Converts tile terrain into a device color.
    pub tile_color: fn(Tiles) -> C,
    /// Converts team id into a device color.
    pub team_color: fn(usize) -> C,
}

/// Shared theme colors for splash rendering.
#[derive(Clone, Copy)]
pub struct SplashTheme<C>
where
    C: PixelColor + Copy,
{
    /// Full-screen background color.
    pub background: C,
    /// Main title/body text color.
    pub text: C,
}

/// Shared theme colors for list-screen rendering.
#[derive(Clone, Copy)]
pub struct ListTheme<C>
where
    C: PixelColor + Copy,
{
    /// Background fill color.
    pub background: C,
    /// Main text color.
    pub text: C,
    /// Highlight fill for the selected row.
    pub selected_fill: C,
}

/// Cached state used to avoid redrawing unchanged map cells.
#[derive(Default)]
pub struct RenderCache {
    map_view: Option<MapViewCache>,
}

struct MapViewCache {
    map_name: alloc::string::String,
    map_width: usize,
    map_height: usize,
    visible_cols: usize,
    visible_rows: usize,
    visible_cells: Vec<u32>,
}

/// Returns the visible map window dimensions in tiles for the given screen.
///
/// # Arguments
/// * `screen_size` - Full drawable screen size in pixels.
/// * `config` - Map-view rendering geometry.
pub fn visible_tiles(screen_size: Size, config: RenderConfig) -> (usize, usize) {
    let usable_height = screen_size
        .height
        .saturating_sub(config.header_height + config.footer_height);
    (
        screen_size.width.saturating_div(config.tile_width.max(1)) as usize,
        usable_height.saturating_div(config.tile_height.max(1)) as usize,
    )
}

/// Clears the gameplay render cache so the next frame repaints everything.
pub fn reset_cache(render_cache: &mut RenderCache) {
    render_cache.map_view = None;
}

/// Draws a shared splash screen with a centered title and menu.
///
/// # Arguments
/// * `display` - Platform draw target.
/// * `screen_size` - Full drawable screen size in pixels.
/// * `splash` - Splash-screen state.
/// * `title` - Main centered title.
/// * `menu` - Menu labels to render under the title.
/// * `footer` - Footer hint line.
/// * `theme` - Device-specific splash colors.
pub fn draw_splash_screen<D, C>(
    display: &mut D,
    screen_size: Size,
    splash: &SplashScreen,
    title: &str,
    menu: &[&str],
    footer: &str,
    theme: SplashTheme<C>,
) where
    D: DrawTarget<Color = C>,
    C: PixelColor + Copy,
{
    halt_on_error(display.clear(theme.background));

    let title_style = MonoTextStyle::new(&embedded_graphics::mono_font::ascii::FONT_10X20, theme.text);
    let body_style = MonoTextStyle::new(&FONT_6X10, theme.text);

    let center_x = (screen_size.width / 2) as i32;
    let center_y = (screen_size.height / 2) as i32;

    halt_on_error(
        embedded_graphics::text::Text::with_alignment(
            title,
            Point::new(center_x, center_y - 24),
            title_style,
            embedded_graphics::text::Alignment::Center,
        )
        .draw(display),
    );

    let menu_top = center_y + 2;
    for (idx, label) in menu.iter().enumerate() {
        let prefix = if idx == splash.selected { ">" } else { " " };
        let line = format!("{prefix} {label}");
        halt_on_error(
            embedded_graphics::text::Text::with_alignment(
                &line,
                Point::new(center_x, menu_top + (idx as i32 * 14)),
                body_style,
                embedded_graphics::text::Alignment::Center,
            )
            .draw(display),
        );
    }

    halt_on_error(
        embedded_graphics::text::Text::with_alignment(
            footer,
            Point::new(center_x, menu_top + (menu.len() as i32 * 14) + 18),
            body_style,
            embedded_graphics::text::Alignment::Center,
        )
        .draw(display),
    );

    if let Some(status_text) = splash.status.as_deref() {
        halt_on_error(
            embedded_graphics::text::Text::with_alignment(
                status_text,
                Point::new(center_x, screen_size.height as i32 - 14),
                body_style,
                embedded_graphics::text::Alignment::Center,
            )
            .draw(display),
        );
    }
}

/// Draws a shared selectable list screen.
///
/// # Arguments
/// * `display` - Platform draw target.
/// * `screen_size` - Full drawable screen size in pixels.
/// * `list` - Shared list state.
/// * `title` - Title shown at the top.
/// * `footer` - Footer hint line.
/// * `visible_rows` - Number of list rows that fit on screen.
/// * `theme` - Device-specific list colors.
pub fn draw_list_screen<D, C>(
    display: &mut D,
    screen_size: Size,
    list: &ListScreen,
    title: &str,
    footer: &str,
    visible_rows: usize,
    theme: ListTheme<C>,
) where
    D: DrawTarget<Color = C>,
    C: PixelColor + Copy,
{
    halt_on_error(display.clear(theme.background));

    let text_style = MonoTextStyle::new(&FONT_6X10, theme.text);
    let selected_style = PrimitiveStyle::with_fill(theme.selected_fill);
    let line_height: i32 = 14;
    let start_y: i32 = 18;

    halt_on_error(Text::new(title, Point::new(6, 10), text_style).draw(display));

    let end = core::cmp::min(list.scroll + visible_rows, list.entries.len());
    for (row, entry_index) in (list.scroll..end).enumerate() {
        let y = start_y + (row as i32 * line_height);
        if entry_index == list.selected {
            halt_on_error(
                Rectangle::new(Point::new(2, y - 9), Size::new(screen_size.width - 4, 12))
                    .into_styled(selected_style)
                    .draw(display),
            );
        }

        let prefix = if entry_index == list.selected { ">" } else { " " };
        let entry = &list.entries[entry_index];
        let line = format!("{prefix} {} ({}b)", entry.label, entry.meta);
        halt_on_error(Text::new(&line, Point::new(6, y), text_style).draw(display));
    }

    halt_on_error(
        Text::new(footer, Point::new(4, screen_size.height as i32 - 2), text_style).draw(display),
    );

    if let Some(status_text) = list.status.as_deref() {
        halt_on_error(
            Text::new(
                status_text,
                Point::new(4, screen_size.height as i32 - 14),
                text_style,
            )
            .draw(display),
        );
    }
}

/// Draws the shared gameplay map view.
///
/// # Arguments
/// * `display` - Platform draw target.
/// * `screen_size` - Full drawable screen size in pixels.
/// * `map_view` - Shared gameplay state.
/// * `render_cache` - Incremental render cache.
/// * `config` - Geometry settings for the device.
/// * `theme` - Device-specific colors.
pub fn draw_map_view<D, C>(
    display: &mut D,
    screen_size: Size,
    map_view: &MapViewApp,
    render_cache: &mut RenderCache,
    config: RenderConfig,
    theme: MapViewTheme<C>,
) where
    D: DrawTarget<Color = C>,
    C: PixelColor + Copy,
{
    let text_style = MonoTextStyle::new(&FONT_6X10, theme.text);
    let mode_name = match map_view.mode() {
        InteractionMode::Pan => "PAN",
        InteractionMode::Hero => "HERO",
    };
    let header = format!(
        "{} {} @{},{}",
        map_view.session().map_name(),
        mode_name,
        map_view.view_x(),
        map_view.view_y()
    );

    let origin_x = 0i32;
    let origin_y = config.header_height as i32;
    let (visible_cols, visible_rows) = visible_tiles(screen_size, config);
    let map = &map_view.session().state().map;
    let map_width = map.tile_width() as usize;
    let map_height = map.tile_height() as usize;

    let requires_full_redraw = match render_cache.map_view.as_ref() {
        Some(cache) => {
            cache.map_name != map_view.session().map_name()
                || cache.map_width != map_width
                || cache.map_height != map_height
                || cache.visible_cols != visible_cols
                || cache.visible_rows != visible_rows
        }
        None => true,
    };

    if requires_full_redraw {
        halt_on_error(display.clear(theme.background));
        render_cache.map_view = Some(MapViewCache {
            map_name: map_view.session().map_name().to_string(),
            map_width,
            map_height,
            visible_cols,
            visible_rows,
            visible_cells: vec![EMPTY_TILE; visible_cols * visible_rows],
        });
    }

    let cache = render_cache
        .map_view
        .as_mut()
        .expect("map view cache must exist before drawing");

    clear_band(
        display,
        Rectangle::new(
            Point::new(0, 0),
            Size::new(screen_size.width, config.header_height),
        ),
        theme.background,
    );
    halt_on_error(Text::new(&header, Point::new(4, 10), text_style).draw(display));

    for row in 0..visible_rows {
        for col in 0..visible_cols {
            let map_x = map_view.view_x() + col;
            let map_y = map_view.view_y() + row;
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
            let x = origin_x + (col as i32 * config.tile_width as i32);
            let y = origin_y + (row as i32 * config.tile_height as i32);
            draw_cell(display, map_view, coord, Point::new(x, y), config, theme);
        }
    }

    let footer = match map_view.mode() {
        InteractionMode::Pan => "Enter: hero mode  WASD/HJKL: pan  Q/Back: back",
        InteractionMode::Hero => "Enter: pan mode  WASD/HJKL: move hero  Q/Back: back",
    };
    clear_band(
        display,
        Rectangle::new(
            Point::new(0, screen_size.height as i32 - config.footer_height as i32),
            Size::new(screen_size.width, config.footer_height),
        ),
        theme.background,
    );
    halt_on_error(
        Text::new(
            footer,
            Point::new(4, screen_size.height as i32 - 2),
            text_style,
        )
        .draw(display),
    );

    if let Some(status_text) = map_view.status() {
        halt_on_error(
            Text::new(
                status_text,
                Point::new(4, screen_size.height as i32 - 14),
                text_style,
            )
            .draw(display),
        );
    }

    let summary = map_view.session().summary();
    halt_on_error(
        Text::new(
            &summary,
            Point::new(4, config.header_height as i32 - 2),
            text_style,
        )
        .draw(display),
    );
}

fn draw_cell<D, C>(
    display: &mut D,
    map_view: &MapViewApp,
    coord: MapCoord,
    top_left: Point,
    config: RenderConfig,
    theme: MapViewTheme<C>,
) where
    D: DrawTarget<Color = C>,
    C: PixelColor + Copy,
{
    let map = &map_view.session().state().map;
    let Ok(tile) = map.get_tile(coord) else {
        return;
    };

    halt_on_error(
        Rectangle::new(
            top_left,
            Size::new(
                config.tile_width.saturating_sub(1),
                config.tile_height.saturating_sub(1),
            ),
        )
        .into_styled(PrimitiveStyle::with_fill((theme.tile_color)(tile.kind)))
        .draw(display),
    );

    if let Some(team_id) = map_view.session().state().city_owner(coord) {
        halt_on_error(
            Rectangle::new(top_left + Point::new(1, 1), Size::new(4, 4))
                .into_styled(PrimitiveStyle::with_fill((theme.team_color)(
                    team_id as usize,
                )))
                .draw(display),
        );
    }

    if map.has_enemy_spawn(coord) {
        halt_on_error(
            Rectangle::new(top_left + Point::new(1, 1), Size::new(3, 3))
                .into_styled(PrimitiveStyle::with_fill(theme.enemy_spawn))
                .draw(display),
        );
    }

    if map.has_chest_spawn(coord) {
        let x = top_left.x + config.tile_width as i32 - 5;
        halt_on_error(
            Rectangle::new(Point::new(x, top_left.y + 1), Size::new(3, 3))
                .into_styled(PrimitiveStyle::with_fill(theme.chest))
                .draw(display),
        );
    }

    if let Some(hero) = map_view.session().state().hero_at(coord) {
        draw_hero_marker(
            display,
            hero.get_id(),
            map_view.session().selected_hero_id(),
            top_left,
            config,
            theme,
        );
    }
}

fn draw_hero_marker<D, C>(
    display: &mut D,
    hero_id: HeroId,
    selected_hero_id: HeroId,
    top_left: Point,
    config: RenderConfig,
    theme: MapViewTheme<C>,
) where
    D: DrawTarget<Color = C>,
    C: PixelColor + Copy,
{
    let color = if hero_id == selected_hero_id {
        theme.selected_hero
    } else {
        theme.hero
    };
    let inset_x = (config.tile_width as i32 / 4).max(1);
    let inset_y = (config.tile_height as i32 / 4).max(1);
    let marker_w = (config.tile_width / 2).max(1);
    let marker_h = (config.tile_height / 2).max(1);
    halt_on_error(
        Rectangle::new(
            top_left + Point::new(inset_x, inset_y),
            Size::new(marker_w, marker_h),
        )
        .into_styled(PrimitiveStyle::with_fill(color))
        .draw(display),
    );
}

fn cell_signature(map_view: &MapViewApp, coord: MapCoord) -> u32 {
    let map = &map_view.session().state().map;
    let Ok(tile) = map.get_tile(coord) else {
        return EMPTY_TILE;
    };

    let mut signature = tile.kind.to_gid();
    if let Some(hero) = map_view.session().state().hero_at(coord) {
        signature |= (u32::from(hero.get_id()) + 1) << 16;
    }
    if let Some(team_id) = map_view.session().state().city_owner(coord) {
        signature |= (u32::from(team_id) + 1) << 24;
    }
    if map.has_enemy_spawn(coord) {
        signature |= 1 << 30;
    }
    if map.has_chest_spawn(coord) {
        signature |= 1 << 31;
    }
    signature
}

fn clear_band<D, C>(display: &mut D, rect: Rectangle, color: C)
where
    D: DrawTarget<Color = C>,
    C: PixelColor + Copy,
{
    halt_on_error(
        rect.into_styled(PrimitiveStyle::with_fill(color))
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
