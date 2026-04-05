//! T-Deck standalone prototype with map selection and visible-area rendering.

#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::RefCell;

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::{FONT_10X20, FONT_6X10};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::{Dimensions, Point, Primitive, RgbColor, Size};
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::{Alignment, Text};
use embedded_graphics::Drawable;
use embedded_hal_bus::spi::RefCellDevice;
use embedded_sdmmc::{LfnBuffer, Mode, SdCard, TimeSource, Timestamp, VolumeIdx, VolumeManager};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::Level::{High, Low};
use esp_hal::gpio::{Input, InputConfig, Output, OutputConfig, Pull};
use esp_hal::i2c::master::{BusTimeout, Config as I2cConfig, I2c};
use esp_hal::main;
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::time::{Duration, Instant, Rate};
use mipidsi::Builder;
use mipidsi::interface::SpiInterface;
use mipidsi::models::ST7789;
use mipidsi::options::{ColorInversion, ColorOrder, Orientation, Rotation};

const KEYBOARD_I2C_ADDRESS: u8 = 0x55;
const MAPS_DIR: &str = "MAPS";
const TILE_SIZE: i32 = 16;
const HEADER_HEIGHT: i32 = 22;
const FOOTER_HEIGHT: i32 = 12;
const EMPTY_TILE: u16 = u16::MAX;

// This creates the ESP-IDF app descriptor required by the T-Deck bootloader.
esp_bootloader_esp_idf::esp_app_desc!();

#[derive(Clone)]
struct MapEntry {
    short_name: String,
    display_name: String,
    size_bytes: u32,
}

struct LoadedMap {
    name: String,
    width: usize,
    height: usize,
    tiles: Vec<u16>,
}

struct LaunchConfig {
    start_map: Option<&'static str>,
    start_x: usize,
    start_y: usize,
}

struct MapViewCache {
    map_name: String,
    map_width: usize,
    map_height: usize,
    visible_cols: usize,
    visible_rows: usize,
    visible_tiles: Vec<u16>,
}

struct RenderCache {
    map_view: Option<MapViewCache>,
}

enum Screen {
    Splash {
        status: Option<String>,
    },
    MapSelect {
        maps: Vec<MapEntry>,
        selected: usize,
        scroll: usize,
        status: Option<String>,
    },
    MapView {
        map: LoadedMap,
        view_x: usize,
        view_y: usize,
        status: Option<String>,
    },
}

enum InputEvent {
    None,
    Enter,
    Back,
    Up,
    Down,
    Left,
    Right,
}

struct InputState {
    last_click_high: bool,
    last_up_high: bool,
    last_down_high: bool,
    last_left_high: bool,
    last_right_high: bool,
}

enum AppError {
    StorageUnavailable,
    MapsDirMissing,
    NoMapsFound,
    InvalidTmx(&'static str),
    InvalidConfiguredMap,
}

#[derive(Default)]
struct DummyTimesource;

impl TimeSource for DummyTimesource {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 0,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

impl LaunchConfig {
    fn from_env() -> Self {
        Self {
            start_map: option_env!("TDECK_START_MAP"),
            start_x: parse_env_usize(option_env!("TDECK_VIEW_X")).unwrap_or(0),
            start_y: parse_env_usize(option_env!("TDECK_VIEW_Y")).unwrap_or(0),
        }
    }
}

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 96 * 1024);

    let mut delay = Delay::new();

    let mut board_power: Output<'_> =
        Output::new(peripherals.GPIO10, High, OutputConfig::default());
    board_power.set_high();
    delay.delay_millis(1_000);

    let mut radio_cs: Output<'_> = Output::new(peripherals.GPIO9, High, OutputConfig::default());
    radio_cs.set_high();
    let mut tft_enable: Output<'_> = Output::new(peripherals.GPIO42, High, OutputConfig::default());
    tft_enable.set_high();

    let tft_cs: Output<'_> = Output::new(peripherals.GPIO12, High, OutputConfig::default());
    let sd_cs: Output<'_> = Output::new(peripherals.GPIO39, High, OutputConfig::default());
    let tft_dc: Output<'_> = Output::new(peripherals.GPIO11, Low, OutputConfig::default());

    let spi = halt_on_error(
        Spi::new(
            peripherals.SPI2,
            SpiConfig::default().with_frequency(Rate::from_mhz(40)),
        ),
    )
    .with_sck(peripherals.GPIO40)
    .with_miso(Input::new(
        peripherals.GPIO38,
        InputConfig::default().with_pull(Pull::Up),
    ))
    .with_mosi(peripherals.GPIO41);

    let spi_bus: RefCell<_> = RefCell::new(spi);

    let display_device = halt_on_error(RefCellDevice::new(&spi_bus, tft_cs, Delay::new()));
    let mut display_buffer: [u8; 512] = [0; 512];
    let display_interface = SpiInterface::new(display_device, tft_dc, &mut display_buffer);

    let mut display = halt_on_error(
        Builder::new(ST7789, display_interface)
            .display_size(240, 320)
            .invert_colors(ColorInversion::Inverted)
            .color_order(ColorOrder::Rgb)
            .orientation(Orientation::new().rotate(Rotation::Deg90))
            .init(&mut delay),
    );

    let sd_device = halt_on_error(RefCellDevice::new(&spi_bus, sd_cs, Delay::new()));
    let sd_card = SdCard::new(sd_device, Delay::new());
    let volume_mgr = VolumeManager::<_, DummyTimesource, 4, 4, 1>::new(sd_card, DummyTimesource);

    let mut keyboard = halt_on_error(
        I2c::new(
            peripherals.I2C0,
            I2cConfig::default()
                .with_frequency(Rate::from_khz(100))
                .with_timeout(BusTimeout::Disabled),
        ),
    )
    .with_sda(peripherals.GPIO18)
    .with_scl(peripherals.GPIO8);

    let trackball_click: Input<'_> = Input::new(
        peripherals.GPIO0,
        InputConfig::default().with_pull(Pull::Up),
    );
    let trackball_right: Input<'_> = Input::new(
        peripherals.GPIO2,
        InputConfig::default().with_pull(Pull::Up),
    );
    let trackball_left: Input<'_> = Input::new(
        peripherals.GPIO1,
        InputConfig::default().with_pull(Pull::Up),
    );
    let trackball_up: Input<'_> = Input::new(
        peripherals.GPIO3,
        InputConfig::default().with_pull(Pull::Up),
    );
    let trackball_down: Input<'_> = Input::new(
        peripherals.GPIO15,
        InputConfig::default().with_pull(Pull::Up),
    );

    let launch = LaunchConfig::from_env();
    let mut screen = initial_screen(&volume_mgr, &launch);
    let mut needs_redraw = true;
    let mut render_cache = RenderCache { map_view: None };
    let mut input_state = InputState {
        last_click_high: trackball_click.is_high(),
        last_up_high: trackball_up.is_high(),
        last_down_high: trackball_down.is_high(),
        last_left_high: trackball_left.is_high(),
        last_right_high: trackball_right.is_high(),
    };

    loop {
        let screen_size = display.bounding_box().size;
        let event = poll_input(
            &mut keyboard,
            &trackball_click,
            &trackball_up,
            &trackball_down,
            &trackball_left,
            &trackball_right,
            &mut input_state,
        );

        if handle_event(&mut screen, event, &volume_mgr, &launch, screen_size) {
            needs_redraw = true;
        }

        if needs_redraw {
            draw_screen(&mut display, &screen, screen_size, &mut render_cache);
            needs_redraw = false;
        }

        let frame_start = Instant::now();
        while frame_start.elapsed() < Duration::from_millis(16) {}
    }
}

fn initial_screen<D>(
    volume_mgr: &VolumeManager<D, DummyTimesource, 4, 4, 1>,
    launch: &LaunchConfig,
) -> Screen
where
    D: embedded_sdmmc::BlockDevice,
{
    if let Some(requested_map) = launch.start_map {
        match discover_maps(volume_mgr) {
            Ok(maps) => {
                if let Some(entry) = maps
                    .iter()
                    .find(|entry| names_match(&entry.display_name, requested_map) || names_match(&entry.short_name, requested_map))
                {
                    match load_map(volume_mgr, entry) {
                        Ok(map) => {
                            return Screen::MapView {
                                map,
                                view_x: launch.start_x,
                                view_y: launch.start_y,
                                status: Some(
                                    "Started from TDECK_START_MAP compile-time config".to_string(),
                                ),
                            };
                        }
                        Err(err) => {
                            return Screen::MapSelect {
                                maps,
                                selected: 0,
                                scroll: 0,
                                status: Some(error_message(err)),
                            };
                        }
                    }
                }

                return Screen::MapSelect {
                    maps,
                    selected: 0,
                    scroll: 0,
                    status: Some(error_message(AppError::InvalidConfiguredMap)),
                };
            }
            Err(err) => {
                return Screen::Splash {
                    status: Some(error_message(err)),
                };
            }
        }
    }

    Screen::Splash { status: None }
}

fn poll_input(
    keyboard: &mut I2c<'_, esp_hal::Blocking>,
    trackball_click: &Input<'_>,
    trackball_up: &Input<'_>,
    trackball_down: &Input<'_>,
    trackball_left: &Input<'_>,
    trackball_right: &Input<'_>,
    input_state: &mut InputState,
) -> InputEvent {
    let mut key_data: [u8; 1] = [0; 1];
    if keyboard.read(KEYBOARD_I2C_ADDRESS, &mut key_data).is_ok() {
        let key = key_data[0];
        let event = match key {
            b'\r' | b'\n' => InputEvent::Enter,
            b'w' | b'W' | b'k' | b'K' => InputEvent::Up,
            b's' | b'S' | b'j' | b'J' => InputEvent::Down,
            b'a' | b'A' | b'h' | b'H' => InputEvent::Left,
            b'd' | b'D' | b'l' | b'L' => InputEvent::Right,
            0x08 | 0x1B | 0x7F => InputEvent::Back,
            _ => InputEvent::None,
        };

        if !matches!(event, InputEvent::None) {
            return event;
        }
    }

    let click_high = trackball_click.is_high();
    if click_high != input_state.last_click_high {
        input_state.last_click_high = click_high;
        if !click_high {
            return InputEvent::Enter;
        }
    }

    let up_high = trackball_up.is_high();
    if up_high != input_state.last_up_high {
        input_state.last_up_high = up_high;
        if !up_high {
            return InputEvent::Up;
        }
    }

    let down_high = trackball_down.is_high();
    if down_high != input_state.last_down_high {
        input_state.last_down_high = down_high;
        if !down_high {
            return InputEvent::Down;
        }
    }

    let left_high = trackball_left.is_high();
    if left_high != input_state.last_left_high {
        input_state.last_left_high = left_high;
        if !left_high {
            return InputEvent::Left;
        }
    }

    let right_high = trackball_right.is_high();
    if right_high != input_state.last_right_high {
        input_state.last_right_high = right_high;
        if !right_high {
            return InputEvent::Right;
        }
    }

    InputEvent::None
}

fn handle_event<D>(
    screen: &mut Screen,
    event: InputEvent,
    volume_mgr: &VolumeManager<D, DummyTimesource, 4, 4, 1>,
    launch: &LaunchConfig,
    screen_size: Size,
) -> bool
where
    D: embedded_sdmmc::BlockDevice,
{
    match screen {
        Screen::Splash { .. } => {
            if matches!(event, InputEvent::Enter) {
                *screen = match discover_maps(volume_mgr) {
                    Ok(maps) => Screen::MapSelect {
                        maps,
                        selected: 0,
                        scroll: 0,
                        status: Some("Select a map and press Enter".to_string()),
                    },
                    Err(err) => Screen::Splash {
                        status: Some(error_message(err)),
                    },
                };
                return true;
            }
        }
        Screen::MapSelect {
            maps,
            selected,
            scroll,
            status,
        } => {
            if maps.is_empty() {
                if matches!(event, InputEvent::Enter) {
                    *screen = Screen::Splash {
                        status: Some("No .tmx maps found in /maps".to_string()),
                    };
                    return true;
                }
                return false;
            }

            let mut changed = false;
            match event {
                InputEvent::Up => {
                    if *selected > 0 {
                        *selected -= 1;
                        changed = true;
                    }
                }
                InputEvent::Down => {
                    if *selected + 1 < maps.len() {
                        *selected += 1;
                        changed = true;
                    }
                }
                InputEvent::Enter => match load_map(volume_mgr, &maps[*selected]) {
                    Ok(map) => {
                        *screen = Screen::MapView {
                            map,
                            view_x: launch.start_x,
                            view_y: launch.start_y,
                            status: Some("Map loaded from SD card".to_string()),
                        };
                        return true;
                    }
                    Err(err) => {
                        *status = Some(error_message(err));
                        changed = true;
                    }
                },
                InputEvent::Back => {
                    *screen = Screen::Splash { status: None };
                    return true;
                }
                InputEvent::Left | InputEvent::Right | InputEvent::None => {}
            }

            let previous_scroll = *scroll;
            let visible_rows = selectable_rows(screen_size);
            if *selected < *scroll {
                *scroll = *selected;
            } else if *selected >= *scroll + visible_rows {
                *scroll = selected.saturating_sub(visible_rows.saturating_sub(1));
            }
            return changed || *scroll != previous_scroll;
        }
        Screen::MapView {
            map,
            view_x,
            view_y,
            status: _,
        } => {
            let (visible_cols, visible_rows) = map_view_tiles(screen_size);
            let max_x = map.width.saturating_sub(visible_cols);
            let max_y = map.height.saturating_sub(visible_rows);

            let previous_x = *view_x;
            let previous_y = *view_y;
            match event {
                InputEvent::Up => *view_y = view_y.saturating_sub(1),
                InputEvent::Down => *view_y = (*view_y + 1).min(max_y),
                InputEvent::Left => *view_x = view_x.saturating_sub(1),
                InputEvent::Right => *view_x = (*view_x + 1).min(max_x),
                InputEvent::Back => {
                    *screen = match discover_maps(volume_mgr) {
                        Ok(maps) => Screen::MapSelect {
                            selected: maps
                                .iter()
                                .position(|entry| entry.display_name == map.name)
                                .unwrap_or(0),
                            scroll: 0,
                            maps,
                            status: Some("Returned to map selection".to_string()),
                        },
                        Err(err) => Screen::Splash {
                            status: Some(error_message(err)),
                        },
                    };
                    return true;
                }
                InputEvent::Enter | InputEvent::None => {}
            }

            *view_x = (*view_x).min(max_x);
            *view_y = (*view_y).min(max_y);
            return *view_x != previous_x || *view_y != previous_y;
        }
    }

    false
}

fn draw_screen<D>(
    display: &mut D,
    screen: &Screen,
    screen_size: Size,
    render_cache: &mut RenderCache,
)
where
    D: DrawTarget<Color = Rgb565>,
{
    match screen {
        Screen::Splash { status } => {
            render_cache.map_view = None;
            draw_splash(display, screen_size, status.as_deref());
        }
        Screen::MapSelect {
            maps,
            selected,
            scroll,
            status,
        } => {
            render_cache.map_view = None;
            draw_map_select(display, screen_size, maps, *selected, *scroll, status.as_deref());
        }
        Screen::MapView {
            map,
            view_x,
            view_y,
            status,
        } => draw_map_view(
            display,
            screen_size,
            map,
            *view_x,
            *view_y,
            status.as_deref(),
            render_cache,
        ),
    }
}

fn draw_splash<D>(display: &mut D, screen_size: Size, status: Option<&str>)
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

    if let Some(status_text) = status {
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

fn draw_map_select<D>(
    display: &mut D,
    screen_size: Size,
    maps: &[MapEntry],
    selected: usize,
    scroll: usize,
    status: Option<&str>,
) where
    D: DrawTarget<Color = Rgb565>,
{
    halt_on_error(display.clear(Rgb565::BLACK));

    let title_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
    let selected_style = PrimitiveStyle::with_fill(Rgb565::new(0, 18, 0));
    let line_height: i32 = 14;
    let start_y: i32 = 18;

    halt_on_error(Text::new("Maps on /maps", Point::new(6, 10), title_style).draw(display));

    let max_rows = selectable_rows(screen_size);
    let end = core::cmp::min(scroll + max_rows, maps.len());

    for (row, entry_index) in (scroll..end).enumerate() {
        let y = start_y + (row as i32 * line_height);
        if entry_index == selected {
            halt_on_error(
                Rectangle::new(Point::new(2, y - 9), Size::new(screen_size.width - 4, 12))
                    .into_styled(selected_style)
                    .draw(display),
            );
        }

        let prefix = if entry_index == selected { ">" } else { " " };
        let line = format!(
            "{} {} ({}b)",
            prefix, maps[entry_index].display_name, maps[entry_index].size_bytes
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

    if let Some(status_text) = status {
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
    map: &LoadedMap,
    view_x: usize,
    view_y: usize,
    status: Option<&str>,
    render_cache: &mut RenderCache,
) where
    D: DrawTarget<Color = Rgb565>,
{
    let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
    let header = format!("{}  {}x{}  @{},{}", map.name, map.width, map.height, view_x, view_y);

    let origin_x = 0;
    let origin_y = HEADER_HEIGHT;
    let map_width_px = screen_size.width as i32;
    let map_height_px = screen_size.height as i32 - HEADER_HEIGHT - FOOTER_HEIGHT;
    let visible_cols = (map_width_px / TILE_SIZE).max(0) as usize;
    let visible_rows = (map_height_px / TILE_SIZE).max(0) as usize;
    let requires_full_redraw = match render_cache.map_view.as_ref() {
        Some(cache) => {
            cache.map_name != map.name
                || cache.map_width != map.width
                || cache.map_height != map.height
                || cache.visible_cols != visible_cols
                || cache.visible_rows != visible_rows
        }
        None => true,
    };

    if requires_full_redraw {
        halt_on_error(display.clear(Rgb565::BLACK));
        render_cache.map_view = Some(MapViewCache {
            map_name: map.name.clone(),
            map_width: map.width,
            map_height: map.height,
            visible_cols,
            visible_rows,
            visible_tiles: alloc::vec![EMPTY_TILE; visible_cols * visible_rows],
        });
    }

    let cache = render_cache
        .map_view
        .as_mut()
        .expect("map view cache must exist before drawing");

    clear_band(display, Rectangle::new(Point::new(0, 0), Size::new(screen_size.width, HEADER_HEIGHT as u32)));
    halt_on_error(Text::new(&header, Point::new(4, 10), text_style).draw(display));

    for row in 0..visible_rows {
        for col in 0..visible_cols {
            let map_x = view_x + col;
            let map_y = view_y + row;
            let tile = if map_x < map.width && map_y < map.height {
                map.tiles[map_y * map.width + map_x]
            } else {
                EMPTY_TILE
            };
            let cache_index = row * visible_cols + col;
            if cache.visible_tiles[cache_index] == tile {
                continue;
            }

            cache.visible_tiles[cache_index] = tile;
            let tile_color = if tile == EMPTY_TILE {
                Rgb565::BLACK
            } else {
                tile_color(tile)
            };
            let x = origin_x + (col as i32 * TILE_SIZE);
            let y = origin_y + (row as i32 * TILE_SIZE);

            halt_on_error(
                Rectangle::new(
                    Point::new(x, y),
                    Size::new((TILE_SIZE - 1) as u32, (TILE_SIZE - 1) as u32),
                )
                .into_styled(PrimitiveStyle::with_fill(tile_color))
                .draw(display),
            );
        }
    }

    let footer = "Trackball/WASD: pan  Back: maps";
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

    if let Some(status_text) = status {
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

fn clear_band<D>(display: &mut D, rect: Rectangle)
where
    D: DrawTarget<Color = Rgb565>,
{
    halt_on_error(
        rect.into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
            .draw(display),
    );
}

fn discover_maps<D>(
    volume_mgr: &VolumeManager<D, DummyTimesource, 4, 4, 1>,
) -> Result<Vec<MapEntry>, AppError>
where
    D: embedded_sdmmc::BlockDevice,
{
    let volume = volume_mgr
        .open_volume(VolumeIdx(0))
        .map_err(|_| AppError::StorageUnavailable)?;
    let root_dir = volume
        .open_root_dir()
        .map_err(|_| AppError::StorageUnavailable)?;

    let maps_dir = root_dir
        .open_dir(MAPS_DIR)
        .or_else(|_| root_dir.open_dir("maps"))
        .map_err(|_| AppError::MapsDirMissing)?;

    let mut entries: Vec<MapEntry> = Vec::new();
    let mut lfn_storage: [u8; 128] = [0; 128];
    let mut lfn_buffer = LfnBuffer::new(&mut lfn_storage);

    maps_dir
        .iterate_dir_lfn(&mut lfn_buffer, |dir_entry, long_name| {
            if dir_entry.attributes.is_directory() {
                return;
            }

            let short_name = dir_entry.name.to_string();
            let display_name = long_name.unwrap_or(short_name.as_str()).to_string();

            if has_tmx_extension(&display_name) || has_tmx_extension(&short_name) {
                entries.push(MapEntry {
                    short_name,
                    display_name,
                    size_bytes: dir_entry.size,
                });
            }
        })
        .map_err(|_| AppError::StorageUnavailable)?;

    entries.sort_unstable_by(|left, right| left.display_name.cmp(&right.display_name));

    if entries.is_empty() {
        return Err(AppError::NoMapsFound);
    }

    Ok(entries)
}

fn load_map<D>(
    volume_mgr: &VolumeManager<D, DummyTimesource, 4, 4, 1>,
    entry: &MapEntry,
) -> Result<LoadedMap, AppError>
where
    D: embedded_sdmmc::BlockDevice,
{
    let volume = volume_mgr
        .open_volume(VolumeIdx(0))
        .map_err(|_| AppError::StorageUnavailable)?;
    let root_dir = volume
        .open_root_dir()
        .map_err(|_| AppError::StorageUnavailable)?;
    let maps_dir = root_dir
        .open_dir(MAPS_DIR)
        .or_else(|_| root_dir.open_dir("maps"))
        .map_err(|_| AppError::MapsDirMissing)?;
    let file = maps_dir
        .open_file_in_dir(entry.short_name.as_str(), Mode::ReadOnly)
        .map_err(|_| AppError::StorageUnavailable)?;

    let file_len = file.length() as usize;
    let mut bytes: Vec<u8> = alloc::vec![0; file_len];
    let mut offset = 0usize;

    while !file.is_eof() && offset < bytes.len() {
        let read = file
            .read(&mut bytes[offset..])
            .map_err(|_| AppError::StorageUnavailable)?;
        if read == 0 {
            break;
        }
        offset += read;
    }

    bytes.truncate(offset);
    let xml = core::str::from_utf8(&bytes).map_err(|_| AppError::InvalidTmx("TMX is not UTF-8"))?;
    parse_tmx(entry.display_name.as_str(), xml)
}

fn parse_tmx(name: &str, xml: &str) -> Result<LoadedMap, AppError> {
    let map_tag_start = xml.find("<map").ok_or(AppError::InvalidTmx("Missing <map> tag"))?;
    let map_tag_end = xml[map_tag_start..]
        .find('>')
        .ok_or(AppError::InvalidTmx("Malformed <map> tag"))?
        + map_tag_start;
    let map_tag = &xml[map_tag_start..=map_tag_end];

    let width = parse_attribute_usize(map_tag, "width")?;
    let height = parse_attribute_usize(map_tag, "height")?;

    let data_start_tag = "<data encoding=\"csv\">";
    let data_start = xml
        .find(data_start_tag)
        .ok_or(AppError::InvalidTmx("Missing CSV layer data"))?
        + data_start_tag.len();
    let data_end = xml[data_start..]
        .find("</data>")
        .ok_or(AppError::InvalidTmx("Unclosed CSV layer data"))?
        + data_start;

    let csv = &xml[data_start..data_end];
    let mut tiles: Vec<u16> = Vec::with_capacity(width * height);
    for value in csv.split(',') {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parsed = trimmed
            .parse::<u16>()
            .map_err(|_| AppError::InvalidTmx("Invalid tile gid"))?;
        tiles.push(parsed);
    }

    if tiles.len() != width * height {
        return Err(AppError::InvalidTmx("Tile count does not match map size"));
    }

    Ok(LoadedMap {
        name: name.to_string(),
        width,
        height,
        tiles,
    })
}

fn parse_attribute_usize(tag: &str, attribute: &str) -> Result<usize, AppError> {
    let needle = format!("{attribute}=\"");
    let start = tag
        .find(needle.as_str())
        .ok_or(AppError::InvalidTmx("Missing map dimension"))?
        + needle.len();
    let end = tag[start..]
        .find('"')
        .ok_or(AppError::InvalidTmx("Malformed map attribute"))?
        + start;

    tag[start..end]
        .parse::<usize>()
        .map_err(|_| AppError::InvalidTmx("Invalid numeric map attribute"))
}

fn tile_color(tile_gid: u16) -> Rgb565 {
    match tile_gid {
        0 => Rgb565::BLACK,
        1 => Rgb565::new(8, 42, 8),
        2 => Rgb565::new(0, 24, 0),
        3 => Rgb565::new(14, 14, 14),
        4 => Rgb565::new(0, 14, 28),
        5 => Rgb565::new(31, 28, 0),
        6 => Rgb565::new(31, 20, 0),
        7 => Rgb565::new(22, 16, 8),
        8 => Rgb565::new(0, 32, 31),
        9 => Rgb565::new(16, 10, 4),
        10 => Rgb565::new(20, 26, 12),
        11 => Rgb565::new(26, 8, 20),
        12 => Rgb565::new(16, 16, 18),
        13 => Rgb565::new(31, 31, 0),
        14 => Rgb565::new(31, 0, 0),
        _ => Rgb565::new(31, 0, 31),
    }
}

fn selectable_rows(screen_size: Size) -> usize {
    screen_size.height.saturating_sub(32) as usize / 14
}

fn map_view_tiles(screen_size: Size) -> (usize, usize) {
    let usable_height = screen_size.height as i32 - HEADER_HEIGHT - FOOTER_HEIGHT;
    (
        (screen_size.width as i32 / TILE_SIZE).max(0) as usize,
        (usable_height / TILE_SIZE).max(0) as usize,
    )
}

fn has_tmx_extension(name: &str) -> bool {
    let lower = name.as_bytes();
    lower.len() >= 4
        && lower[lower.len() - 4].eq_ignore_ascii_case(&b'.')
        && lower[lower.len() - 3].eq_ignore_ascii_case(&b't')
        && lower[lower.len() - 2].eq_ignore_ascii_case(&b'm')
        && lower[lower.len() - 1].eq_ignore_ascii_case(&b'x')
}

fn names_match(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn parse_env_usize(value: Option<&'static str>) -> Option<usize> {
    value.and_then(|item| item.parse::<usize>().ok())
}

fn error_message(error: AppError) -> String {
    match error {
        AppError::StorageUnavailable => "SD card is unavailable or unreadable".to_string(),
        AppError::MapsDirMissing => "Folder /maps was not found on the SD card".to_string(),
        AppError::NoMapsFound => "No .tmx maps were found in /maps".to_string(),
        AppError::InvalidTmx(message) => format!("TMX parse error: {message}"),
        AppError::InvalidConfiguredMap => {
            "TDECK_START_MAP does not match any map in /maps".to_string()
        }
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
