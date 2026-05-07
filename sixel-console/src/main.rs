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
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use rpg_embedded::input::InputEvent;
use rpg_embedded::map_view::{MapViewApp, MapViewOutcome};
use rpg_embedded::render::{
    MapViewTheme, RenderCache, RenderConfig, SplashTheme, draw_map_view, draw_splash_screen,
    visible_tiles,
};
use rpg_embedded::session::GameSession;
use rpg_embedded::splash::{SplashOutcome, SplashScreen};
use rpg_engine::game_state::GameState;
use rpg_engine::hero::Hero;
use rpg_engine::map::game_map::GameMap;
use rpg_engine::spawn;
use rpg_engine::team::Team;
use rpg_mapgen::map_assembler::{MapAssembler, MapConfig};
use rpg_tiled::read_tmx;
use terminal_size::{Height, Width, terminal_size};
use tracing::{error, info};

const MAP_RENDER_CONFIG: RenderConfig = RenderConfig {
    tile_width: 64,
    tile_height: 64,
    header_height: 28,
    footer_height: 16,
};

const BACKGROUND: Rgb888 = Rgb888::new(20, 22, 26);
const SPLASH_BACKGROUND: Rgb888 = Rgb888::new(36, 0, 72);
const TEXT: Rgb888 = Rgb888::new(235, 238, 242);

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

struct RawModeGuard;

enum ConsoleScreen {
    Splash(SplashScreen),
    Map(MapViewApp),
}

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
    let session = GameSession::from_state("terminal".to_string(), state)?;
    let _raw_mode = RawModeGuard::new()?;
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let app = MapViewApp::new(
        session,
        0,
        0,
        Some("Ctrl+Q exits. Enter toggles pan/hero mode.".to_string()),
    );
    let mut pending_app = Some(app);
    let mut screen = ConsoleScreen::Splash(SplashScreen::new(0, None));
    let mut render_cache = RenderCache::default();
    let mut last_screen_size: Option<Size> = None;
    let mut needs_redraw = true;

    loop {
        let screen_size = detect_screen_size();
        if last_screen_size != Some(screen_size) {
            if let ConsoleScreen::Map(app) = &mut screen {
                let (visible_cols, visible_rows) = visible_tiles(screen_size, MAP_RENDER_CONFIG);
                app.clamp_view_to_map(visible_cols, visible_rows);
            }
            last_screen_size = Some(screen_size);
            needs_redraw = true;
        }

        if needs_redraw {
            let framebuffer = render_frame(screen_size, &screen, &mut render_cache)?;
            write_frame(&mut handle, &framebuffer)?;
            needs_redraw = false;
        }

        if event::poll(Duration::from_millis(120))? {
            match event::read()? {
                Event::Key(key)
                    if key.code == KeyCode::Char('q')
                        && key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    clear_screen(&mut handle)?;
                    handle.flush()?;
                    break;
                }
                Event::Key(key) => {
                    match &mut screen {
                        ConsoleScreen::Splash(splash) => match splash.handle_input(map_key_event(key), 1) {
                            SplashOutcome::Selected(_) => {
                                if let Some(app) = pending_app.take() {
                                    screen = ConsoleScreen::Map(app);
                                    reset_render_cache(&mut render_cache);
                                    needs_redraw = true;
                                }
                            }
                            SplashOutcome::Changed => {
                                needs_redraw = true;
                            }
                            SplashOutcome::BackRequested | SplashOutcome::NoChange => {}
                        },
                        ConsoleScreen::Map(app) => {
                            let input = map_key_event(key);
                            let (visible_cols, visible_rows) =
                                visible_tiles(screen_size, MAP_RENDER_CONFIG);
                            match app.handle_input(input, visible_cols, visible_rows) {
                                MapViewOutcome::NoChange => {}
                                MapViewOutcome::Changed => {
                                    needs_redraw = true;
                                }
                                MapViewOutcome::BackRequested => {
                                    app.set_status(Some(
                                        "Back is reserved. Use Ctrl+Q to exit.".to_string(),
                                    ));
                                    needs_redraw = true;
                                }
                            }
                        }
                    }
                }
                Event::Resize(_, _) => {
                    last_screen_size = None;
                    needs_redraw = true;
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

fn detect_screen_size() -> Size {
    if let Some((Width(columns), Height(rows))) = terminal_size() {
        let width = u32::from(columns).saturating_mul(10);
        let height = u32::from(rows.saturating_sub(2)).saturating_mul(20);
        Size::new(
            width.max(MAP_RENDER_CONFIG.tile_width),
            height.max(MAP_RENDER_CONFIG.header_height + MAP_RENDER_CONFIG.footer_height + MAP_RENDER_CONFIG.tile_height),
        )
    } else {
        Size::new(240, 160)
    }
}

fn map_key_event(key: crossterm::event::KeyEvent) -> InputEvent {
    match key.code {
        KeyCode::Enter => InputEvent::Enter,
        KeyCode::Char(' ') => InputEvent::Enter,
        KeyCode::Esc | KeyCode::Backspace => InputEvent::Back,
        KeyCode::Up => InputEvent::Up,
        KeyCode::Down => InputEvent::Down,
        KeyCode::Left => InputEvent::Left,
        KeyCode::Right => InputEvent::Right,
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => InputEvent::Key(ch),
        _ => InputEvent::None,
    }
}

fn render_frame(
    screen_size: Size,
    screen: &ConsoleScreen,
    render_cache: &mut RenderCache,
) -> AppResult<Framebuffer> {
    let mut framebuffer = Framebuffer::new(screen_size, BACKGROUND)?;
    match screen {
        ConsoleScreen::Splash(splash) => draw_splash(&mut framebuffer, screen_size, splash),
        ConsoleScreen::Map(app) => draw_map_view(
            &mut framebuffer,
            screen_size,
            app,
            render_cache,
            MAP_RENDER_CONFIG,
            map_theme(),
        ),
    }
    Ok(framebuffer)
}

fn draw_splash(framebuffer: &mut Framebuffer, screen_size: Size, splash: &SplashScreen) {
    draw_splash_screen(
        framebuffer,
        screen_size,
        splash,
        "weave of realms",
        &["Start"],
        "Enter: start  Ctrl+Q: exit",
        SplashTheme {
            background: SPLASH_BACKGROUND,
            text: TEXT,
        },
    );
}

fn map_theme() -> MapViewTheme<Rgb888> {
    MapViewTheme {
        background: BACKGROUND,
        text: TEXT,
        selected_hero: Rgb888::new(255, 255, 120),
        hero: Rgb888::new(255, 255, 255),
        enemy_spawn: Rgb888::new(255, 120, 120),
        chest: Rgb888::new(248, 198, 66),
        tile_color,
        team_color,
    }
}

fn reset_render_cache(render_cache: &mut RenderCache) {
    *render_cache = RenderCache::default();
}

fn tile_color(tile: rpg_engine::map::tile::Tiles) -> Rgb888 {
    let (r, g, b) = tile.as_color();
    Rgb888::new(r, g, b)
}

fn team_color(team_id: usize) -> Rgb888 {
    match team_id {
        0 => Rgb888::new(220, 50, 50),
        1 => Rgb888::new(50, 100, 220),
        _ => Rgb888::new(140, 140, 140),
    }
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
