//! Standalone SDL2 launcher that renders AI RPG maps via `embedded-graphics`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Parser;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::OriginDimensions;
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use rpg_embedded::app::{AppHost, AppLayout, AppScreen, EmbeddedApp, LaunchConfig, LoadedGame};
use rpg_embedded::info_overlay::InfoOverlay;
use rpg_embedded::input::InputEvent;
use rpg_embedded::list::ListEntry;
use rpg_embedded::render::{
    draw_app_screen, visible_tiles, AppRenderCache, AppTheme, InfoOverlayTheme, ListTheme,
    MapViewTheme, RenderConfig, SaveOverlayTheme, SplashTheme,
};
use rpg_engine::game_state::GameState;
use rpg_engine::hero::Hero;
use rpg_engine::map::game_map::GameMap;
use rpg_engine::map::tile::Tiles;
use rpg_engine::spawn;
use rpg_engine::team::Team;
use rpg_mapgen::map_assembler::{MapAssembler, MapConfig};
use rpg_tiled::read_tmx;
use sdl2::controller::{Axis, Button, GameController};
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::pixels::PixelFormatEnum;
use tracing::{error, info, warn};

const MAP_RENDER_CONFIG: RenderConfig = RenderConfig {
    header_height: 28,
    footer_height: 16,
};

const BACKGROUND: Rgb888 = Rgb888::new(20, 22, 26);
const SPLASH_BACKGROUND: Rgb888 = Rgb888::new(36, 0, 72);
const TEXT: Rgb888 = Rgb888::new(235, 238, 242);
const OUTPUT_SCALE: usize = 2;
const INITIAL_WINDOW_WIDTH: u32 = 720;
const INITIAL_WINDOW_HEIGHT: u32 = 720;
const MIN_WINDOW_WIDTH: u32 = 320;
const MIN_WINDOW_HEIGHT: u32 = 240;

type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Debug, Parser)]
#[command(name = "weave-of-realms-sdl2", author, version, about)]
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

struct SdlHost {
    args: Args,
    screen_size: Size,
}

#[derive(Debug)]
enum HostError {
    Message(String),
    Io(std::io::Error),
    Engine(String),
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    if let Err(error) = run() {
        error!(%error, "sdl2 launcher failed");
        std::process::exit(1);
    }
}

fn run() -> AppResult<()> {
    let sdl = sdl2::init().map_err(boxed_error)?;
    let game_controller = sdl.game_controller().map_err(boxed_error)?;
    let video = sdl.video().map_err(boxed_error)?;
    let window = video
        .window(
            "weave of realms",
            INITIAL_WINDOW_WIDTH,
            INITIAL_WINDOW_HEIGHT,
        )
        .position_centered()
        .resizable()
        .allow_highdpi()
        .build()
        .map_err(boxed_error)?;
    let mut canvas = window
        .into_canvas()
        .accelerated()
        .present_vsync()
        .build()
        .map_err(boxed_error)?;
    let texture_creator = canvas.texture_creator();
    let mut event_pump = sdl.event_pump().map_err(boxed_error)?;

    let size = canvas.output_size().map_err(boxed_error)?;
    let initial_size = window_size(size.0, size.1);
    let initial_render_size = logical_render_size(initial_size);
    let mut host = SdlHost {
        args: Args::parse(),
        screen_size: initial_size,
    };
    let mut app_state = EmbeddedApp::new(
        &mut host,
        LaunchConfig {
            start_map: None,
            start_x: 0,
            start_y: 0,
        },
    );
    let mut render_cache = AppRenderCache::default();
    let mut last_output_size = initial_size;
    let mut needs_redraw = true;
    let mut framebuffer = Framebuffer::new(initial_render_size, BACKGROUND)?;
    let mut controllers: Vec<GameController> = open_controllers(&game_controller);
    app_state.clamp_view_to_layout(app_layout(initial_render_size));

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::Window {
                    win_event: sdl2::event::WindowEvent::SizeChanged(width, height),
                    ..
                }
                | Event::Window {
                    win_event: sdl2::event::WindowEvent::Resized(width, height),
                    ..
                } => {
                    let output_size = window_size(width as u32, height as u32);
                    host.screen_size = output_size;
                    app_state.clamp_view_to_layout(app_layout(logical_render_size(output_size)));
                    last_output_size = output_size;
                    needs_redraw = true;
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Q),
                    keymod,
                    repeat: false,
                    ..
                } if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => break 'running,
                Event::KeyDown {
                    keycode: Some(keycode),
                    keymod,
                    repeat: false,
                    ..
                } => {
                    let input = map_key_event(keycode, keymod);
                    if app_state.handle_input(
                        &mut host,
                        input,
                        app_layout(logical_render_size(last_output_size)),
                    ) {
                        needs_redraw = true;
                    }
                }
                Event::ControllerDeviceAdded { which, .. } => {
                    if game_controller.is_game_controller(which) {
                        match game_controller.open(which) {
                            Ok(c) => {
                                info!(name = c.name(), "controller connected");
                                controllers.push(c);
                            }
                            Err(e) => warn!("failed to open controller {which}: {e}"),
                        }
                    }
                }
                Event::ControllerDeviceRemoved { which, .. } => {
                    controllers.retain(|c| c.instance_id() != which);
                    info!(id = which, "controller disconnected");
                }
                Event::ControllerButtonDown { button, .. } => {
                    let input = map_controller_button(button);
                    if app_state.handle_input(
                        &mut host,
                        input,
                        app_layout(logical_render_size(last_output_size)),
                    ) {
                        needs_redraw = true;
                    }
                }
                Event::ControllerAxisMotion { axis, value, .. } => {
                    const DEAD_ZONE: i16 = 16_000;
                    let input = match axis {
                        Axis::LeftX if value > DEAD_ZONE => InputEvent::Right,
                        Axis::LeftX if value < -DEAD_ZONE => InputEvent::Left,
                        Axis::LeftY if value > DEAD_ZONE => InputEvent::Down,
                        Axis::LeftY if value < -DEAD_ZONE => InputEvent::Up,
                        _ => InputEvent::None,
                    };
                    if app_state.handle_input(
                        &mut host,
                        input,
                        app_layout(logical_render_size(last_output_size)),
                    ) {
                        needs_redraw = true;
                    }
                }
                _ => {}
            }
        }

        if needs_redraw {
            let render_size = logical_render_size(last_output_size);
            if framebuffer.size != render_size {
                framebuffer = Framebuffer::new(render_size, BACKGROUND)?;
            }
            render_frame(
                render_size,
                app_state.screen(),
                &mut render_cache,
                &mut framebuffer,
            );
            present_frame(&mut canvas, &texture_creator, &framebuffer)?;
            needs_redraw = false;
        }

        std::thread::sleep(Duration::from_millis(16));
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

impl AppHost for SdlHost {
    type Error = HostError;

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
                .map_err(|error| HostError::Engine(error.to_string()))?;
            return Ok(LoadedGame {
                map_name: entry.label.clone(),
                state,
            });
        }
        if entry.id.starts_with("generated:") {
            let state = load_state(&self.args, None)
                .map_err(|error| HostError::Engine(error.to_string()))?;
            return Ok(LoadedGame {
                map_name: entry.label.clone(),
                state,
            });
        }
        if let Some(path) = entry.id.strip_prefix("map:") {
            let bytes = fs::read(path).map_err(HostError::Io)?;
            let state = GameState::from_save_bytes(&bytes)
                .map_err(|error| HostError::Engine(error.to_string()))?;
            return Ok(LoadedGame {
                map_name: entry.label.clone(),
                state,
            });
        }
        Err(HostError::Message("Unknown map entry".to_string()))
    }

    fn load_save(&mut self, entry: &ListEntry) -> Result<LoadedGame, Self::Error> {
        let path = entry
            .id
            .strip_prefix("save:")
            .ok_or_else(|| HostError::Message("Unknown save entry".to_string()))?;
        let bytes = fs::read(path).map_err(HostError::Io)?;
        let state = GameState::from_save_bytes(&bytes)
            .map_err(|error| HostError::Engine(error.to_string()))?;
        Ok(LoadedGame {
            map_name: entry.label.clone(),
            state,
        })
    }

    fn save_game(&mut self, name: &str, state: &GameState) -> Result<(), Self::Error> {
        let dir = Path::new("savegame");
        fs::create_dir_all(dir).map_err(HostError::Io)?;
        let file_name = sanitize_save_filename(name);
        let path = dir.join(file_name);
        let bytes = state
            .to_save_bytes_with_name(name)
            .map_err(|error| HostError::Engine(error.to_string()))?;
        fs::write(path, bytes).map_err(HostError::Io)
    }

    fn info_overlay(&mut self) -> Option<InfoOverlay> {
        Some(InfoOverlay::new(
            "System Info".to_string(),
            vec![
                format!(
                    "Viewport: {}x{}",
                    self.screen_size.width, self.screen_size.height
                ),
                format!("Seed: {}", self.args.seed),
            ],
            "Enter or q: close".to_string(),
        ))
    }

    fn error_message(&self, error: Self::Error) -> String {
        match error {
            HostError::Message(message) => message,
            HostError::Io(error) => format!("I/O error: {error}"),
            HostError::Engine(message) => format!("Engine error: {message}"),
        }
    }
}

fn discover_rpgs_dir(dir: &Path, prefix: &str) -> Result<Vec<ListEntry>, HostError> {
    let mut entries: Vec<ListEntry> = Vec::new();
    if !dir.is_dir() {
        return Ok(entries);
    }

    let read_dir = fs::read_dir(dir).map_err(HostError::Io)?;
    for entry in read_dir {
        let entry = entry.map_err(HostError::Io)?;
        let path = entry.path();
        if !is_rpgs_path(&path) {
            continue;
        }
        let metadata = entry.metadata().map_err(HostError::Io)?;
        let label = read_save_name(&path).unwrap_or_else(|| file_label(&path));
        entries.push(ListEntry {
            id: format!("{prefix}{}", path.display()),
            label,
            meta: u32::try_from(metadata.len()).unwrap_or(u32::MAX),
        });
    }
    Ok(entries)
}

fn file_entry(prefix: &str, path: &Path) -> Result<ListEntry, HostError> {
    let metadata = fs::metadata(path).map_err(HostError::Io)?;
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

fn map_key_event(keycode: Keycode, keymod: Mod) -> InputEvent {
    match keycode {
        Keycode::Return | Keycode::Space => InputEvent::Enter,
        Keycode::Escape | Keycode::Backspace => InputEvent::Back,
        Keycode::Up => InputEvent::Up,
        Keycode::Down => InputEvent::Down,
        Keycode::Left => InputEvent::Left,
        Keycode::Right => InputEvent::Right,
        Keycode::Tab => InputEvent::NextHero,
        Keycode::A if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('a'),
        Keycode::B if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('b'),
        Keycode::C if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('c'),
        Keycode::D if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('d'),
        Keycode::E if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('e'),
        Keycode::F if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('f'),
        Keycode::G if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('g'),
        Keycode::H if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('h'),
        Keycode::I if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('i'),
        Keycode::J if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('j'),
        Keycode::K if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('k'),
        Keycode::L if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('l'),
        Keycode::M if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('m'),
        Keycode::N if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('n'),
        Keycode::O if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('o'),
        Keycode::P if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('p'),
        Keycode::Q if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('q'),
        Keycode::R if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('r'),
        Keycode::S if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('s'),
        Keycode::T if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('t'),
        Keycode::U if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('u'),
        Keycode::V if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('v'),
        Keycode::W if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('w'),
        Keycode::X if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('x'),
        Keycode::Y if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('y'),
        Keycode::Z if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('z'),
        Keycode::Num0 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('0'),
        Keycode::Num1 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('1'),
        Keycode::Num2 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('2'),
        Keycode::Num3 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('3'),
        Keycode::Num4 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('4'),
        Keycode::Num5 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('5'),
        Keycode::Num6 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('6'),
        Keycode::Num7 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('7'),
        Keycode::Num8 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('8'),
        Keycode::Num9 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('9'),
        Keycode::Minus if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('-'),
        Keycode::Underscore if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => {
            InputEvent::Key('_')
        }
        Keycode::Period if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => {
            InputEvent::Key('.')
        }
        _ => InputEvent::None,
    }
}

fn render_frame(
    screen_size: Size,
    screen: &AppScreen,
    render_cache: &mut AppRenderCache,
    framebuffer: &mut Framebuffer,
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

fn tile_color(tile: Tiles) -> Rgb888 {
    let (r, g, b) = tile.as_color();
    Rgb888::new(r, g, b)
}

fn tile_sprite_color(tile: Tiles) -> Rgb888 {
    let (r, g, b) = tile.as_color();
    Rgb888::new(
        r.saturating_add(40),
        g.saturating_add(30),
        b.saturating_add(10),
    )
}

fn team_color(team_id: usize) -> Rgb888 {
    match team_id {
        0 => Rgb888::new(220, 50, 50),
        1 => Rgb888::new(50, 100, 220),
        _ => Rgb888::new(140, 140, 140),
    }
}

fn present_frame(
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    texture_creator: &sdl2::render::TextureCreator<sdl2::video::WindowContext>,
    framebuffer: &Framebuffer,
) -> AppResult<()> {
    let mut texture = texture_creator
        .create_texture_streaming(
            PixelFormatEnum::RGB24,
            framebuffer.size.width * OUTPUT_SCALE as u32,
            framebuffer.size.height * OUTPUT_SCALE as u32,
        )
        .map_err(boxed_error)?;
    let bytes = framebuffer.rgb_bytes_scaled(OUTPUT_SCALE);
    texture
        .update(
            None,
            &bytes,
            framebuffer.size.width as usize * OUTPUT_SCALE * 3,
        )
        .map_err(boxed_error)?;
    canvas.clear();
    canvas.copy(&texture, None, None).map_err(boxed_error)?;
    canvas.present();
    Ok(())
}

fn window_size(width: u32, height: u32) -> Size {
    let minimum = minimum_output_size();
    Size::new(
        width.max(MIN_WINDOW_WIDTH).max(minimum.width),
        height.max(MIN_WINDOW_HEIGHT).max(minimum.height),
    )
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

fn open_controllers(subsystem: &sdl2::GameControllerSubsystem) -> Vec<GameController> {
    let count = subsystem.num_joysticks().unwrap_or(0);
    let mut out = Vec::new();
    for i in 0..count {
        if subsystem.is_game_controller(i) {
            match subsystem.open(i) {
                Ok(c) => {
                    info!(name = c.name(), "controller connected");
                    out.push(c);
                }
                Err(e) => warn!("failed to open controller {i}: {e}"),
            }
        }
    }
    out
}

fn map_controller_button(button: Button) -> InputEvent {
    match button {
        Button::DPadUp => InputEvent::Up,
        Button::DPadDown => InputEvent::Down,
        Button::DPadLeft => InputEvent::Left,
        Button::DPadRight => InputEvent::Right,
        Button::A | Button::Start => InputEvent::Enter,
        Button::B | Button::Back => InputEvent::Back,
        _ => InputEvent::None,
    }
}

fn boxed_error<E>(error: E) -> Box<dyn std::error::Error>
where
    E: ToString,
{
    Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        error.to_string(),
    ))
}

struct Framebuffer {
    size: Size,
    pixels: Vec<Rgb888>,
}

impl Framebuffer {
    fn new(size: Size, background: Rgb888) -> AppResult<Self> {
        let len = usize::try_from(size.width)
            .ok()
            .and_then(|width| {
                usize::try_from(size.height)
                    .ok()
                    .map(|height| width.saturating_mul(height))
            })
            .ok_or_else(|| "framebuffer dimensions overflow".to_string())?;
        Ok(Self {
            size,
            pixels: vec![background; len],
        })
    }

    fn rgb_bytes_scaled(&self, scale: usize) -> Vec<u8> {
        let src_width = self.size.width as usize;
        let src_height = self.size.height as usize;
        let mut bytes = Vec::with_capacity(src_width * src_height * scale * scale * 3);
        for y in 0..src_height {
            let row_start = y * src_width;
            let row = &self.pixels[row_start..row_start + src_width];
            for _ in 0..scale {
                for color in row {
                    for _ in 0..scale {
                        bytes.push(color.r());
                        bytes.push(color.g());
                        bytes.push(color.b());
                    }
                }
            }
        }
        bytes
    }
}

impl OriginDimensions for Framebuffer {
    fn size(&self) -> Size {
        self.size
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
            if point.x < 0
                || point.y < 0
                || point.x >= self.size.width as i32
                || point.y >= self.size.height as i32
            {
                continue;
            }
            let index = point.y as usize * self.size.width as usize + point.x as usize;
            if let Some(pixel) = self.pixels.get_mut(index) {
                *pixel = color;
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.pixels.fill(color);
        Ok(())
    }
}
