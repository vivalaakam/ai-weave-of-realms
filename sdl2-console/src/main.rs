//! Standalone SDL2 launcher that renders AI RPG maps via `embedded-graphics`.

use std::time::Duration;

use args::Args;
use clap::Parser;
use frame_buffer::Framebuffer;
use game::app::{AppLayout, AppScreen, EmbeddedApp, LaunchConfig};
use game::input::InputEvent;
use game::prelude::render::{Rgb888, Size};
use game::render::{
    draw_app_screen, visible_tiles, AppRenderCache, AppTheme, InfoOverlayTheme, ListTheme,
    MapViewTheme, RenderConfig, SaveOverlayTheme, SplashTheme,
};
use game::Tiles;
use sdl2::controller::{Axis, Button, GameController};
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::pixels::PixelFormatEnum;
use sdl_host::SdlHost;
use tracing::{error, info, warn};

mod args;
mod error;
mod frame_buffer;
mod sdl_host;

const MAP_RENDER_CONFIG: RenderConfig = RenderConfig { header_height: 28, footer_height: 16 };

const BACKGROUND: Rgb888 = Rgb888::new(20, 22, 26);
const SPLASH_BACKGROUND: Rgb888 = Rgb888::new(36, 0, 72);
const TEXT: Rgb888 = Rgb888::new(235, 238, 242);
const OUTPUT_SCALE: usize = 2;
const INITIAL_WINDOW_WIDTH: u32 = 720;
const INITIAL_WINDOW_HEIGHT: u32 = 720;
const MIN_WINDOW_WIDTH: u32 = 320;
const MIN_WINDOW_HEIGHT: u32 = 240;

type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

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
        .window("weave of realms", INITIAL_WINDOW_WIDTH, INITIAL_WINDOW_HEIGHT)
        .position_centered()
        .resizable()
        .allow_highdpi()
        .build()
        .map_err(boxed_error)?;
    let mut canvas =
        window.into_canvas().accelerated().present_vsync().build().map_err(boxed_error)?;
    let texture_creator = canvas.texture_creator();
    let mut event_pump = sdl.event_pump().map_err(boxed_error)?;

    let size = canvas.output_size().map_err(boxed_error)?;
    let initial_size = window_size(size.0, size.1);
    let initial_render_size = logical_render_size(initial_size);
    let mut host = SdlHost {
        args: Args::parse(),
        screen_size: initial_size,
        right_x_right: false,
        right_x_left: false,
        right_y_down: false,
        right_y_up: false,
        trigger_r_active: false,
    };
    let mut app_state =
        EmbeddedApp::new(&mut host, LaunchConfig { start_map: None, start_x: 0, start_y: 0 });
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
                Event::KeyDown { keycode: Some(Keycode::Q), keymod, repeat: false, .. }
                if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) =>
                    {
                        break 'running
                    }
                Event::KeyDown { keycode: Some(keycode), keymod, repeat: false, .. } => {
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
                    const TRIGGER_THRESHOLD: i16 = 16_000;
                    let mut generated = Vec::new();
                    match axis {
                        Axis::LeftX if value > DEAD_ZONE => generated.push(InputEvent::Right),
                        Axis::LeftX if value < -DEAD_ZONE => generated.push(InputEvent::Left),
                        Axis::LeftY if value > DEAD_ZONE => generated.push(InputEvent::Down),
                        Axis::LeftY if value < -DEAD_ZONE => generated.push(InputEvent::Up),
                        Axis::RightX => {
                            if value > DEAD_ZONE {
                                if !host.right_x_right {
                                    host.right_x_right = true;
                                    generated.push(InputEvent::PanRight);
                                }
                            } else {
                                host.right_x_right = false;
                            }
                            if value < -DEAD_ZONE {
                                if !host.right_x_left {
                                    host.right_x_left = true;
                                    generated.push(InputEvent::PanLeft);
                                }
                            } else {
                                host.right_x_left = false;
                            }
                        }
                        Axis::RightY => {
                            if value > DEAD_ZONE {
                                if !host.right_y_down {
                                    host.right_y_down = true;
                                    generated.push(InputEvent::PanDown);
                                }
                            } else {
                                host.right_y_down = false;
                            }
                            if value < -DEAD_ZONE {
                                if !host.right_y_up {
                                    host.right_y_up = true;
                                    generated.push(InputEvent::PanUp);
                                }
                            } else {
                                host.right_y_up = false;
                            }
                        }
                        Axis::TriggerRight => {
                            if value > TRIGGER_THRESHOLD {
                                if !host.trigger_r_active {
                                    host.trigger_r_active = true;
                                    generated.push(InputEvent::NextHero);
                                }
                            } else {
                                host.trigger_r_active = false;
                            }
                        }
                        _ => {}
                    }
                    for input in generated {
                        if app_state.handle_input(
                            &mut host,
                            input,
                            app_layout(logical_render_size(last_output_size)),
                        ) {
                            needs_redraw = true;
                        }
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
            render_frame(render_size, app_state.screen(), &mut render_cache, &mut framebuffer);
            present_frame(&mut canvas, &texture_creator, &framebuffer)?;
            needs_redraw = false;
        }

        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}

fn map_key_event(keycode: Keycode, keymod: Mod) -> InputEvent {
    match keycode {
        Keycode::Space => InputEvent::NextTurn,
        Keycode::Escape => InputEvent::Back,
        Keycode::Up => InputEvent::Up,
        Keycode::Down => InputEvent::Down,
        Keycode::Left => InputEvent::Left,
        Keycode::Right => InputEvent::Right,
        Keycode::Tab => InputEvent::NextHero,
        Keycode::Return => InputEvent::Enter,
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
        splash: SplashTheme { background: SPLASH_BACKGROUND, text: TEXT },
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
    Rgb888::new(r.saturating_add(40), g.saturating_add(30), b.saturating_add(10))
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
        .update(None, &bytes, framebuffer.size.width as usize * OUTPUT_SCALE * 3)
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
        Button::Y => InputEvent::NextTurn,
        Button::B | Button::Back => InputEvent::Back,
        _ => InputEvent::None,
    }
}

fn boxed_error<E>(error: E) -> Box<dyn std::error::Error>
where
    E: ToString,
{
    Box::new(std::io::Error::new(std::io::ErrorKind::Other, error.to_string()))
}
