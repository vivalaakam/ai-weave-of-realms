//! Standalone SDL2 launcher that renders AI RPG maps via `embedded-graphics`.

use std::time::Duration;

use args::Args;
use clap::Parser;
use controller::{handle_controller_event, open_controllers};
use frame_buffer::Framebuffer;
use game::app::{EmbeddedApp, LaunchConfig};
use input::map_key_event;
use layout::{logical_render_size, window_size};
use present::present_frame;
use render::render_frame;
use sdl2::controller::GameController;
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::video::FullscreenType;
use sdl_host::SdlHost;
use tracing::{error, warn};

mod args;
mod controller;
mod error;
mod frame_buffer;
mod input;
mod layout;
mod present;
mod render;
mod sdl_host;

const INITIAL_WINDOW_WIDTH: u32 = 720;
const INITIAL_WINDOW_HEIGHT: u32 = 720;

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

fn run() -> Result<(), error::HostError> {
    let args = Args::parse();

    let sdl = sdl2::init().map_err(|e| error::HostError::Message(e.to_string()))?;
    let game_controller_subsystem =
        sdl.game_controller().map_err(|e| error::HostError::Message(e.to_string()))?;
    let video = sdl.video().map_err(|e| error::HostError::Message(e.to_string()))?;

    let window = if !args.window_mode {
        video
            .window("weave of realms", INITIAL_WINDOW_WIDTH, INITIAL_WINDOW_HEIGHT)
            .position_centered()
            .resizable()
            .allow_highdpi()
            .fullscreen_desktop()
            .build()
    } else {
        video
            .window("weave of realms", INITIAL_WINDOW_WIDTH, INITIAL_WINDOW_HEIGHT)
            .position_centered()
            .resizable()
            .allow_highdpi()
            .build()
    }
    .map_err(|e| error::HostError::Message(e.to_string()))?;

    let mut canvas = window
        .into_canvas()
        .accelerated()
        .present_vsync()
        .build()
        .map_err(|e| error::HostError::Message(e.to_string()))?;

    let texture_creator = canvas.texture_creator();
    let mut event_pump = sdl.event_pump().map_err(|e| error::HostError::Message(e.to_string()))?;

    let size = canvas.output_size().map_err(|e| error::HostError::Message(e.to_string()))?;
    let initial_size = window_size(size.0, size.1);
    let initial_render_size = logical_render_size(initial_size);

    let mut host = SdlHost {
        args,
        screen_size: initial_size,
        left_x_right: false,
        left_x_left: false,
        left_y_down: false,
        left_y_up: false,
        right_x_right: false,
        right_x_left: false,
        right_y_down: false,
        right_y_up: false,
        trigger_r_active: false,
    };
    let mut app_state =
        EmbeddedApp::new(&mut host, LaunchConfig { start_map: None, start_x: 0, start_y: 0 });
    let mut render_cache = game::render::AppRenderCache::default();
    let mut last_output_size = initial_size;
    let mut needs_redraw = true;
    let mut framebuffer = Framebuffer::new(initial_render_size, render::BACKGROUND)?;

    let mut controllers: Vec<GameController> = open_controllers(&game_controller_subsystem);
    app_state.clamp_view_to_layout(render::app_layout(initial_render_size));

    let mut is_fullscreen = !host.args.window_mode;

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
                    app_state
                        .clamp_view_to_layout(render::app_layout(logical_render_size(output_size)));
                    last_output_size = output_size;
                    needs_redraw = true;
                }
                Event::KeyDown { keycode: Some(Keycode::Q), keymod, repeat: false, .. }
                    if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) =>
                {
                    break 'running;
                }
                Event::KeyDown { keycode: Some(Keycode::F11), repeat: false, .. } => {
                    is_fullscreen = toggle_fullscreen(&mut canvas, is_fullscreen);
                    let (w, h) = canvas
                        .output_size()
                        .map_err(|e| error::HostError::Message(e.to_string()))?;
                    let output_size = window_size(w, h);
                    host.screen_size = output_size;
                    app_state
                        .clamp_view_to_layout(render::app_layout(logical_render_size(output_size)));
                    last_output_size = output_size;
                    needs_redraw = true;
                }
                Event::KeyDown { keycode: Some(keycode), keymod, repeat: false, .. } => {
                    let input = map_key_event(keycode, keymod);
                    if app_state.handle_input(
                        &mut host,
                        input,
                        render::app_layout(logical_render_size(last_output_size)),
                    ) {
                        needs_redraw = true;
                    }
                }
                _ => {
                    let layout = render::app_layout(logical_render_size(last_output_size));
                    handle_controller_event(
                        &event,
                        &game_controller_subsystem,
                        &mut controllers,
                        &mut host,
                        &layout,
                        &mut app_state,
                        &mut needs_redraw,
                    );
                }
            }
        }

        if needs_redraw {
            let render_size = logical_render_size(last_output_size);
            if framebuffer.size != render_size {
                framebuffer = Framebuffer::new(render_size, render::BACKGROUND)?;
            }
            render_frame(render_size, app_state.screen(), &mut render_cache, &mut framebuffer);
            present_frame(&mut canvas, &texture_creator, &framebuffer)?;
            needs_redraw = false;
        }

        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}

/// Toggle fullscreen display mode for the window backing `canvas`.
///
/// Returns the new fullscreen state.
fn toggle_fullscreen(
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    current: bool,
) -> bool {
    let target = if current { FullscreenType::Off } else { FullscreenType::Desktop };
    if let Err(e) = canvas.window_mut().set_fullscreen(target) {
        warn!("failed to toggle fullscreen: {}", e);
        current
    } else {
        !current
    }
}
