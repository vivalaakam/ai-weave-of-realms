//! Standalone console launcher that renders AI RPG maps as sixel graphics.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::OriginDimensions;
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use game::app::{AppHost, AppLayout, EmbeddedApp, LaunchConfig, LoadedGame};
use game::info_overlay::InfoOverlay;
use game::input::InputEvent;
use game::list::ListEntry;
use game::render::{
    AppRenderCache, AppTheme, InfoOverlayTheme, ListTheme, MapViewTheme, RenderConfig,
    SaveOverlayTheme, SplashTheme, draw_app_screen, visible_tiles,
};
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
    header_height: 28,
    footer_height: 16,
};

const BACKGROUND: Rgb888 = Rgb888::new(20, 22, 26);
const SPLASH_BACKGROUND: Rgb888 = Rgb888::new(36, 0, 72);
const TEXT: Rgb888 = Rgb888::new(235, 238, 242);
const OUTPUT_SCALE: usize = 2;

type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Debug, Parser)]
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

struct ConsoleHost {
    args: Args,
}

#[derive(Debug)]
enum ConsoleHostError {
    Message(String),
    Io(io::Error),
    Engine(String),
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
    let mut host = ConsoleHost { args: Args::parse() };
    let _raw_mode = RawModeGuard::new()?;
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let mut app_state = EmbeddedApp::new(
        &mut host,
        LaunchConfig {
            start_map: None,
            start_x: 0,
            start_y: 0,
        },
    );
    let mut render_cache = AppRenderCache::default();
    let mut last_output_size: Option<Size> = None;
    let mut needs_redraw = true;

    loop {
        let output_size = detect_screen_size();
        let render_size = logical_render_size(output_size);
        if last_output_size != Some(output_size) {
            app_state.clamp_view_to_layout(app_layout(render_size));
            last_output_size = Some(output_size);
            needs_redraw = true;
        }

        if needs_redraw {
            let framebuffer = render_frame(render_size, app_state.screen(), &mut render_cache)?;
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
                    if app_state.handle_input(&mut host, map_key_event(key), app_layout(render_size))
                    {
                        needs_redraw = true;
                    }
                }
                Event::Resize(_, _) => {
                    last_output_size = None;
                    needs_redraw = true;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn load_state(args: &Args, map_path: Option<&Path>) -> AppResult<GameState> {
    if let Some(path) = &args.save {
        let bytes = fs::read(path)?;
        let state = GameState::from_save_bytes(&bytes)?;
        info!(path = %path.display(), "loaded saved game state");
        return Ok(state);
    }

    if let Some(path) = map_path.or(args.tmx.as_deref()) {
        let map = read_tmx(path)?;
        info!(path = %path.display(), "loaded TMX map");
        return build_default_state(map, &args.seed);
    }

    let map = generate_map(args)?;
    build_default_state(map, &args.seed)
}

impl AppHost for ConsoleHost {
    type Error = ConsoleHostError;

    fn discover_maps(&mut self) -> Result<Vec<ListEntry>, Self::Error> {
        let mut entries = discover_rpgs_dir(Path::new("maps"), "map:")?;
        if let Some(path) = &self.args.tmx {
            entries.push(file_entry("tmx:", path)?);
        }
        if entries.is_empty() {
            entries.push(ListEntry {
                id: format!("generated:{}", self.args.seed),
                label: format!("Generated map ({})", self.args.seed),
                meta: self.args.width.saturating_mul(self.args.height),
            });
        }
        entries.sort_by(|left, right| left.label.cmp(&right.label));
        Ok(entries)
    }

    fn discover_saves(&mut self) -> Result<Vec<ListEntry>, Self::Error> {
        let mut entries = discover_rpgs_dir(Path::new("savegame"), "save:")?;
        if let Some(path) = &self.args.save {
            entries.push(file_entry("save:", path)?);
        }
        entries.sort_by(|left, right| left.label.cmp(&right.label));
        Ok(entries)
    }

    fn load_map(&mut self, entry: &ListEntry) -> Result<LoadedGame, Self::Error> {
        if let Some(path) = entry.id.strip_prefix("tmx:") {
            let state = load_state(&self.args, Some(Path::new(path)))
                .map_err(|error| ConsoleHostError::Engine(error.to_string()))?;
            return Ok(LoadedGame {
                map_name: entry.label.clone(),
                state,
            });
        }
        if entry.id.starts_with("generated:") {
            let state =
                load_state(&self.args, None).map_err(|error| ConsoleHostError::Engine(error.to_string()))?;
            return Ok(LoadedGame {
                map_name: entry.label.clone(),
                state,
            });
        }
        if let Some(path) = entry.id.strip_prefix("map:") {
            let bytes = fs::read(path).map_err(ConsoleHostError::Io)?;
            let state = GameState::from_save_bytes(&bytes)
                .map_err(|error| ConsoleHostError::Engine(error.to_string()))?;
            return Ok(LoadedGame {
                map_name: entry.label.clone(),
                state,
            });
        }
        Err(ConsoleHostError::Message("Unknown map entry".to_string()))
    }

    fn load_save(&mut self, entry: &ListEntry) -> Result<LoadedGame, Self::Error> {
        let path = entry
            .id
            .strip_prefix("save:")
            .ok_or_else(|| ConsoleHostError::Message("Unknown save entry".to_string()))?;
        let bytes = fs::read(path).map_err(ConsoleHostError::Io)?;
        let state = GameState::from_save_bytes(&bytes)
            .map_err(|error| ConsoleHostError::Engine(error.to_string()))?;
        Ok(LoadedGame {
            map_name: entry.label.clone(),
            state,
        })
    }

    fn save_game(&mut self, name: &str, state: &GameState) -> Result<(), Self::Error> {
        let dir = Path::new("savegame");
        fs::create_dir_all(dir).map_err(ConsoleHostError::Io)?;
        let file_name = sanitize_save_filename(name);
        let path = dir.join(file_name);
        let bytes = state
            .to_save_bytes_with_name(name)
            .map_err(|error| ConsoleHostError::Engine(error.to_string()))?;
        fs::write(path, bytes).map_err(ConsoleHostError::Io)
    }

    fn info_overlay(&mut self) -> Option<InfoOverlay> {
        let screen_size = detect_screen_size();
        Some(InfoOverlay::new(
            "System Info".to_string(),
            vec![
                format!("Viewport: {}x{}", screen_size.width, screen_size.height),
                format!("Seed: {}", self.args.seed),
            ],
            "Enter or q: close".to_string(),
        ))
    }

    fn error_message(&self, error: Self::Error) -> String {
        match error {
            ConsoleHostError::Message(message) => message,
            ConsoleHostError::Io(error) => format!("I/O error: {error}"),
            ConsoleHostError::Engine(message) => format!("Engine error: {message}"),
        }
    }
}

fn discover_rpgs_dir(dir: &Path, prefix: &str) -> Result<Vec<ListEntry>, ConsoleHostError> {
    let mut entries: Vec<ListEntry> = Vec::new();
    if !dir.is_dir() {
        return Ok(entries);
    }

    let read_dir = fs::read_dir(dir).map_err(ConsoleHostError::Io)?;
    for entry in read_dir {
        let entry = entry.map_err(ConsoleHostError::Io)?;
        let path = entry.path();
        if !is_rpgs_path(&path) {
            continue;
        }
        let metadata = entry.metadata().map_err(ConsoleHostError::Io)?;
        let label = read_save_name(&path).unwrap_or_else(|| file_label(&path));
        entries.push(ListEntry {
            id: format!("{prefix}{}", path.display()),
            label,
            meta: u32::try_from(metadata.len()).unwrap_or(u32::MAX),
        });
    }
    Ok(entries)
}

fn file_entry(prefix: &str, path: &Path) -> Result<ListEntry, ConsoleHostError> {
    let metadata = fs::metadata(path).map_err(ConsoleHostError::Io)?;
    Ok(ListEntry {
        id: format!("{prefix}{}", path.display()),
        label: file_label(path),
        meta: u32::try_from(metadata.len()).unwrap_or(u32::MAX),
    })
}

fn read_save_name(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    GameState::read_save_name(&bytes).ok()
}

fn file_label(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("unnamed")
        .to_string()
}

fn is_rpgs_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("rpgs"))
        .unwrap_or(false)
}

fn sanitize_save_filename(name: &str) -> String {
    let mut cleaned = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            cleaned.push(ch.to_ascii_uppercase());
        }
    }
    if cleaned.is_empty() {
        cleaned.push_str("SAVE");
    }
    cleaned.truncate(7);
    format!("{cleaned}.RPGS")
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
    let minimum = minimum_output_size();
    if let Some((Width(columns), Height(rows))) = terminal_size() {
        let width = u32::from(columns).saturating_mul(10);
        let height = u32::from(rows.saturating_sub(2)).saturating_mul(20);
        Size::new(
            width.max(minimum.width),
            height.max(minimum.height),
        )
    } else {
        Size::new(minimum.width.max(240), minimum.height.max(160))
    }
}

fn logical_render_size(output_size: Size) -> Size {
    let minimum_width = 32;
    let minimum_height = MAP_RENDER_CONFIG.header_height + MAP_RENDER_CONFIG.footer_height + 32;
    Size::new(
        (output_size.width / OUTPUT_SCALE as u32).max(minimum_width),
        (output_size.height / OUTPUT_SCALE as u32).max(minimum_height),
    )
}

fn minimum_output_size() -> Size {
    let logical_minimum = logical_render_size(Size::new(0, 0));
    Size::new(
        logical_minimum.width * OUTPUT_SCALE as u32,
        logical_minimum.height * OUTPUT_SCALE as u32,
    )
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
    screen: &game::app::AppScreen,
    render_cache: &mut AppRenderCache,
) -> AppResult<Framebuffer> {
    let mut framebuffer = Framebuffer::new(screen_size, BACKGROUND)?;
    draw_app_screen(
        &mut framebuffer,
        screen_size,
        screen,
        render_cache,
        MAP_RENDER_CONFIG,
        app_theme(),
        app_layout(screen_size).list_rows,
        app_layout(screen_size).save_rows,
    );
    Ok(framebuffer)
}

fn app_layout(screen_size: Size) -> AppLayout {
    let (map_visible_cols, map_visible_rows) = visible_tiles(screen_size, MAP_RENDER_CONFIG);
    AppLayout {
        list_rows: screen_size.height.saturating_sub(32) as usize / 14,
        save_rows: 4,
        map_visible_cols,
        map_visible_rows,
    }
}

fn app_theme() -> AppTheme<Rgb888> {
    AppTheme {
        splash: SplashTheme {
            background: SPLASH_BACKGROUND,
            text: TEXT,
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

fn tile_color(tile: rpg_engine::map::tile::Tiles) -> Rgb888 {
    let (r, g, b) = tile.as_color();
    Rgb888::new(r, g, b)
}

fn tile_sprite_color(tile: rpg_engine::map::tile::Tiles) -> Rgb888 {
    let (r, g, b) = tile.as_color();
    Rgb888::new(r.saturating_add(40), g.saturating_add(30), b.saturating_add(10))
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
    let width = framebuffer.size.width as usize * OUTPUT_SCALE;
    let height = framebuffer.size.height as usize * OUTPUT_SCALE;
    let (palette, indices) = palette_indexed_pixels_scaled(framebuffer, OUTPUT_SCALE)?;

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

fn palette_indexed_pixels_scaled(
    framebuffer: &Framebuffer,
    scale: usize,
) -> AppResult<(Vec<Rgb888>, Vec<u16>)> {
    let mut palette: Vec<Rgb888> = Vec::new();
    let mut palette_map: HashMap<u32, u16> = HashMap::new();
    let src_width = framebuffer.size.width as usize;
    let src_height = framebuffer.size.height as usize;
    let scaled_len = src_width
        .checked_mul(scale)
        .and_then(|value| value.checked_mul(src_height))
        .and_then(|value| value.checked_mul(scale))
        .ok_or("scaled framebuffer size overflow")?;
    let mut indices = Vec::with_capacity(scaled_len);

    for y in 0..src_height {
        let row_start = y * src_width;
        let row = &framebuffer.pixels[row_start..row_start + src_width];
        for _ in 0..scale {
            for color in row {
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
                for _ in 0..scale {
                    indices.push(index);
                }
            }
        }
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
