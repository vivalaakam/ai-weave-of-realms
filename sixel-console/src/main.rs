//! Standalone console launcher that renders AI RPG maps as sixel graphics.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::OriginDimensions;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Circle, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle};
use embedded_graphics::text::Text;
use rpg_engine::game_state::GameState;
use rpg_engine::hero::Hero;
use rpg_engine::map::game_map::{GameMap, MapCoord};
use rpg_engine::spawn;
use rpg_engine::team::Team;
use rpg_mapgen::map_assembler::{MapAssembler, MapConfig};
use rpg_tiled::read_tmx;
use terminal_size::{Height, Width, terminal_size};
use tracing::{error, info};

const TILE_PX: u32 = 64;
const HEADER_HEIGHT: u32 = 28;
const BACKGROUND: Rgb888 = Rgb888::new(20, 22, 26);
const PANEL_TEXT: Rgb888 = Rgb888::new(235, 238, 242);
const PANEL_MUTED: Rgb888 = Rgb888::new(130, 138, 150);
const PLAYER_COLOR: Rgb888 = Rgb888::new(220, 50, 50);
const ENEMY_COLOR: Rgb888 = Rgb888::new(168, 96, 214);
const CHEST_COLOR: Rgb888 = Rgb888::new(248, 198, 66);
const ENEMY_SPAWN_COLOR: Rgb888 = Rgb888::new(255, 120, 120);

type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Parser)]
#[command(name = "weave-of-realms-sixel", author, version, about)]
struct Args {
    /// Load a saved game state from an .rpgs file.
    #[arg(long)]
    save: Option<PathBuf>,

    /// Load a TMX map instead of generating one.
    #[arg(long)]
    tmx: Option<PathBuf>,

    /// Seed phrase for deterministic generation.
    #[arg(long, default_value = "default-seed")]
    seed: String,

    /// Map width in tiles when generating.
    #[arg(long, default_value_t = 96)]
    width: u32,

    /// Map height in tiles when generating.
    #[arg(long, default_value_t = 96)]
    height: u32,

    /// Generator script path (repeatable pipeline).
    #[arg(long = "generator", value_name = "SCRIPT")]
    generators: Vec<PathBuf>,

    /// Directory with validation rule scripts.
    #[arg(long)]
    validator_dir: Option<PathBuf>,

    /// Path to a single Lua validator script.
    #[arg(long)]
    validator: Option<PathBuf>,

    /// Path to the Lua evaluator script.
    #[arg(long)]
    evaluator: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Viewport {
    origin_x: u32,
    origin_y: u32,
    tiles_w: u32,
    tiles_h: u32,
}

struct RawModeGuard;

impl RawModeGuard {
    fn new() -> AppResult<Self> {
        enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(io::stderr)
        .init();

    if let Err(error) = run() {
        error!(%error, "sixel launcher failed");
        std::process::exit(1);
    }
}

fn run() -> AppResult<()> {
    let args = Args::parse();
    let state = load_state(&args)?;
    let _raw_mode = RawModeGuard::new()?;
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let mut previous_viewport: Option<Viewport> = None;

    loop {
        let viewport = detect_viewport(&state);
        if previous_viewport != Some(viewport) {
            let framebuffer = render_state(&state, viewport)?;
            write_frame(&mut handle, &framebuffer)?;
            previous_viewport = Some(viewport);
        }

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key)
                    if key.code == KeyCode::Char('q')
                        && key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    clear_screen(&mut handle)?;
                    break;
                }
                Event::Resize(_, _) => {
                    previous_viewport = None;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn load_state(args: &Args) -> AppResult<GameState> {
    if let Some(path) = &args.save {
        let bytes = fs::read(path)?;
        let state = GameState::from_save_bytes(&bytes)?;
        info!(path = %path.display(), "loaded saved game state");
        return Ok(state);
    }

    if let Some(path) = &args.tmx {
        let map = read_tmx(path)?;
        info!(path = %path.display(), "loaded TMX map");
        return build_default_state(map, &args.seed);
    }

    let map = generate_map(args)?;
    build_default_state(map, &args.seed)
}

fn generate_map(args: &Args) -> AppResult<GameMap> {
    let mut generators = args.generators.clone();
    if generators.is_empty() {
        generators.push(PathBuf::from("scripts/generators/default.lua"));
    }

    let first = generators.remove(0);
    let mut config = MapConfig::default_3x3(args.seed.clone(), first);
    config.width = args.width;
    config.height = args.height;

    for generator in generators {
        config = config.with_generator(generator);
    }

    if let Some(path) = &args.validator_dir {
        config = config.with_validator_dir(path.clone());
    } else if let Some(path) = &args.validator {
        config = config.with_validator(path.clone());
    } else {
        let default = PathBuf::from("scripts/rules");
        if default.is_dir() {
            config = config.with_validator_dir(default);
        }
    }

    if let Some(path) = &args.evaluator {
        config = config.with_evaluator(path.clone());
    } else {
        let default = PathBuf::from("scripts/evaluators/evaluate.lua");
        if default.exists() {
            config = config.with_evaluator(default);
        }
    }

    let assembler = MapAssembler::new(config)?;
    match assembler.generate_validated() {
        Ok(map) => Ok(map),
        Err(rpg_mapgen::error::Error::ValidationFailed(reason)) => {
            info!(%reason, "map failed validation, falling back to raw generation");
            Ok(assembler.generate()?)
        }
        Err(error) => Err(Box::new(error)),
    }
}

fn build_default_state(map: GameMap, seed: &str) -> AppResult<GameState> {
    let spawns = spawn::find_spawn_positions(&map)?;
    let mut state = GameState::new(map, seed);
    let player_team_id = state.add_team(Team::red());
    let enemy_team_id = state.add_team(Team::enemy());

    state.add_hero(Hero::new(
        0,
        "Hero",
        100,
        20,
        10,
        15,
        spawns.player,
        player_team_id,
    ));
    state.add_hero(Hero::new(
        1,
        "Enemy",
        85,
        16,
        8,
        12,
        spawns.enemy,
        enemy_team_id,
    ));
    let _ = state.set_city_owner(spawns.player, Some(player_team_id));
    let _ = state.on_turn();
    Ok(state)
}

fn detect_viewport(state: &GameState) -> Viewport {
    let (terminal_width, terminal_height) = detect_terminal_pixels().unwrap_or((240, 140));
    let available_height = terminal_height.saturating_sub(HEADER_HEIGHT);
    let tiles_w = (terminal_width / TILE_PX).max(1).min(state.map.tile_width());
    let tiles_h = (available_height / TILE_PX).max(1).min(state.map.tile_height());

    Viewport {
        origin_x: 0,
        origin_y: 0,
        tiles_w,
        tiles_h,
    }
}

fn detect_terminal_pixels() -> Option<(u32, u32)> {
    let (Width(columns), Height(rows)) = terminal_size()?;
    let width = u32::from(columns).saturating_mul(10);
    let height = u32::from(rows.saturating_sub(2)).saturating_mul(20);
    Some((width.max(TILE_PX), height.max(HEADER_HEIGHT + TILE_PX)))
}

fn render_state(state: &GameState, viewport: Viewport) -> AppResult<Framebuffer> {
    let size = Size::new(
        viewport.tiles_w.saturating_mul(TILE_PX),
        HEADER_HEIGHT + viewport.tiles_h.saturating_mul(TILE_PX),
    );
    let mut framebuffer = Framebuffer::new(size, BACKGROUND)?;

    draw_header(&mut framebuffer, state, viewport);
    draw_map(&mut framebuffer, state, viewport)?;

    Ok(framebuffer)
}

fn draw_header(framebuffer: &mut Framebuffer, state: &GameState, viewport: Viewport) {
    let title_style = MonoTextStyle::new(&FONT_6X10, PANEL_TEXT);
    let subtitle_style = MonoTextStyle::new(&FONT_6X10, PANEL_MUTED);
    let hero_count = state.living_heroes(true).len() + state.living_heroes(false).len();
    let title = format!(
        "weave of realms sixel  map={}x{}  view={}x{}  tile=64px",
        state.map.tile_width(),
        state.map.tile_height(),
        viewport.tiles_w,
        viewport.tiles_h
    );
    let subtitle = format!(
        "heroes={} enemy-spawns={} chests={}  ctrl+q exit",
        hero_count,
        state.map.enemy_spawns().len(),
        state.map.chest_spawns().len()
    );

    let _ = Text::new(&title, Point::new(4, 10), title_style).draw(framebuffer);
    let _ = Text::new(&subtitle, Point::new(4, 22), subtitle_style).draw(framebuffer);
}

fn draw_map(framebuffer: &mut Framebuffer, state: &GameState, viewport: Viewport) -> AppResult<()> {
    let x_end = viewport.origin_x.saturating_add(viewport.tiles_w);
    let y_end = viewport.origin_y.saturating_add(viewport.tiles_h);

    for map_y in viewport.origin_y..y_end {
        for map_x in viewport.origin_x..x_end {
            let coord = MapCoord::new(map_x, map_y);
            let tile = state.map.get_tile(coord)?;
            let screen_x = (map_x - viewport.origin_x) * TILE_PX;
            let screen_y = HEADER_HEIGHT + (map_y - viewport.origin_y) * TILE_PX;
            let top_left = Point::new(screen_x as i32, screen_y as i32);
            let rect = Rectangle::new(top_left, Size::new(TILE_PX, TILE_PX));

            rect.into_styled(
                PrimitiveStyleBuilder::new()
                    .fill_color(tile_color(tile.kind))
                    .stroke_color(Rgb888::new(0, 0, 0))
                    .stroke_width(1)
                    .build(),
            )
            .draw(framebuffer)?;

            if state.map.has_enemy_spawn(coord) {
                draw_marker(framebuffer, top_left, ENEMY_SPAWN_COLOR)?;
            }
            if state.map.has_chest_spawn(coord) {
                draw_marker(framebuffer, top_left, CHEST_COLOR)?;
            }
        }
    }

    for hero in state.living_heroes(true) {
        draw_hero(framebuffer, hero.position, viewport, PLAYER_COLOR)?;
    }
    for hero in state.living_heroes(false) {
        draw_hero(framebuffer, hero.position, viewport, ENEMY_COLOR)?;
    }

    Ok(())
}

fn draw_marker(framebuffer: &mut Framebuffer, top_left: Point, color: Rgb888) -> AppResult<()> {
    let diameter = TILE_PX / 2;
    let center_x = top_left.x + (TILE_PX as i32 / 2) - (diameter as i32 / 2);
    let center_y = top_left.y + (TILE_PX as i32 / 2) - (diameter as i32 / 2);
    Circle::new(Point::new(center_x, center_y), diameter)
        .into_styled(PrimitiveStyle::with_fill(color))
        .draw(framebuffer)?;
    Ok(())
}

fn draw_hero(
    framebuffer: &mut Framebuffer,
    coord: MapCoord,
    viewport: Viewport,
    color: Rgb888,
) -> AppResult<()> {
    if coord.x < viewport.origin_x
        || coord.y < viewport.origin_y
        || coord.x >= viewport.origin_x + viewport.tiles_w
        || coord.y >= viewport.origin_y + viewport.tiles_h
    {
        return Ok(());
    }

    let inset = 8;
    let size = TILE_PX.saturating_sub(inset * 2);
    let rect = Rectangle::new(
        Point::new(
            ((coord.x - viewport.origin_x) * TILE_PX + inset) as i32,
            (HEADER_HEIGHT + (coord.y - viewport.origin_y) * TILE_PX + inset) as i32,
        ),
        Size::new(size, size),
    );
    rect.into_styled(
        PrimitiveStyleBuilder::new()
            .fill_color(color)
            .stroke_color(Rgb888::new(255, 255, 255))
            .stroke_width(2)
            .build(),
    )
    .draw(framebuffer)?;
    Ok(())
}

fn tile_color(tile: rpg_engine::map::tile::Tiles) -> Rgb888 {
    let (r, g, b) = tile.as_color();
    Rgb888::new(r, g, b)
}

fn write_frame<W: Write>(writer: &mut W, framebuffer: &Framebuffer) -> AppResult<()> {
    clear_screen(writer)?;
    write_sixel(writer, framebuffer)?;
    writer.flush()?;
    Ok(())
}

fn clear_screen<W: Write>(writer: &mut W) -> io::Result<()> {
    writer.write_all(b"\x1b[2J\x1b[H")
}

fn write_sixel<W: Write>(writer: &mut W, framebuffer: &Framebuffer) -> AppResult<()> {
    let width = framebuffer.size.width as usize;
    let height = framebuffer.size.height as usize;
    let (palette, indices) = palette_indexed_pixels(framebuffer)?;

    writer.write_all(b"\x1bPq")?;
    for (index, color) in palette.iter().enumerate() {
        let r = sixel_percent(color.r());
        let g = sixel_percent(color.g());
        let b = sixel_percent(color.b());
        write!(writer, "#{index};2;{r};{g};{b}")?;
    }

    for band_y in (0..height).step_by(6) {
        let colors_in_band = colors_in_band(&indices, width, height, band_y, palette.len());
        for (color_pos, color_index) in colors_in_band.iter().enumerate() {
            write!(writer, "#{color_index}")?;
            let mut runs: Vec<(u8, usize)> = Vec::with_capacity(width);

            for x in 0..width {
                let mut pattern = 0u8;
                for bit in 0..6usize {
                    let y = band_y + bit;
                    if y >= height {
                        continue;
                    }
                    let pixel_index = y * width + x;
                    if indices[pixel_index] == *color_index as u16 {
                        pattern |= 1u8 << bit;
                    }
                }
                push_run(&mut runs, pattern.saturating_add(63));
            }

            for (value, count) in runs {
                if count >= 4 {
                    write!(writer, "!{count}{}", char::from(value))?;
                } else {
                    for _ in 0..count {
                        writer.write_all(&[value])?;
                    }
                }
            }

            if color_pos + 1 < colors_in_band.len() {
                writer.write_all(b"$")?;
            }
        }
        writer.write_all(b"-")?;
    }

    writer.write_all(b"\x1b\\")?;
    Ok(())
}

fn palette_indexed_pixels(framebuffer: &Framebuffer) -> AppResult<(Vec<Rgb888>, Vec<u16>)> {
    let mut palette: Vec<Rgb888> = Vec::new();
    let mut palette_map: HashMap<u32, u16> = HashMap::new();
    let mut indices = Vec::with_capacity(framebuffer.pixels.len());

    for color in &framebuffer.pixels {
        let key = ((color.r() as u32) << 16) | ((color.g() as u32) << 8) | color.b() as u32;
        let index = if let Some(index) = palette_map.get(&key) {
            *index
        } else {
            let next = u16::try_from(palette.len())?;
            if next >= 256 {
                return Err("sixel palette exceeded 256 colors".into());
            }
            palette.push(*color);
            palette_map.insert(key, next);
            next
        };
        indices.push(index);
    }

    Ok((palette, indices))
}

fn colors_in_band(
    indices: &[u16],
    width: usize,
    height: usize,
    band_y: usize,
    palette_len: usize,
) -> Vec<usize> {
    let mut seen = vec![false; palette_len];
    for bit in 0..6usize {
        let y = band_y + bit;
        if y >= height {
            continue;
        }
        for x in 0..width {
            let color = indices[y * width + x] as usize;
            if color < seen.len() {
                seen[color] = true;
            }
        }
    }

    seen.into_iter()
        .enumerate()
        .filter_map(|(index, present)| present.then_some(index))
        .collect()
}

fn push_run(runs: &mut Vec<(u8, usize)>, value: u8) {
    if let Some((last, count)) = runs.last_mut() {
        if *last == value {
            *count += 1;
            return;
        }
    }
    runs.push((value, 1));
}

fn sixel_percent(component: u8) -> u8 {
    (((component as u16) * 100) / 255) as u8
}

struct Framebuffer {
    size: Size,
    pixels: Vec<Rgb888>,
}

impl Framebuffer {
    fn new(size: Size, background: Rgb888) -> AppResult<Self> {
        let len = usize::try_from(size.width)?
            .checked_mul(usize::try_from(size.height)?)
            .ok_or("framebuffer size overflow")?;
        Ok(Self {
            size,
            pixels: vec![background; len],
        })
    }

    fn index_of(&self, point: Point) -> Option<usize> {
        if point.x < 0 || point.y < 0 {
            return None;
        }

        let x = usize::try_from(point.x).ok()?;
        let y = usize::try_from(point.y).ok()?;
        let width = usize::try_from(self.size.width).ok()?;
        let height = usize::try_from(self.size.height).ok()?;
        if x >= width || y >= height {
            return None;
        }
        Some(y * width + x)
    }
}

impl DrawTarget for Framebuffer {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if let Some(index) = self.index_of(point) {
                self.pixels[index] = color;
            }
        }
        Ok(())
    }
}

impl OriginDimensions for Framebuffer {
    fn size(&self) -> Size {
        self.size
    }
}
