//! T-Deck standalone app bootstrap.

#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

extern crate alloc;

mod app;
mod input;
mod render;
mod screens;
mod session;
mod storage;
mod system_info;

use core::cell::RefCell;

use app::{LaunchConfig, initial_screen};
use embedded_graphics::prelude::Dimensions;
use embedded_hal_bus::spi::RefCellDevice;
use embedded_sdmmc::{SdCard, TimeSource, Timestamp, VolumeManager};
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
use render::RenderCache;

// This creates the ESP-IDF app descriptor required by the T-Deck bootloader.
esp_bootloader_esp_idf::esp_app_desc!();

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
    let mut system_info = system_info::SystemInfoReader::new(peripherals.ADC1, peripherals.GPIO4);
    let mut screen = initial_screen(&volume_mgr, &launch);
    let mut needs_redraw = true;
    let mut render_cache = RenderCache::default();
    let mut input_state = input::InputState::new(
        trackball_click.is_high(),
        trackball_up.is_high(),
        trackball_down.is_high(),
        trackball_left.is_high(),
        trackball_right.is_high(),
    );

    loop {
        let screen_size = display.bounding_box().size;
        let event = input::poll_input(
            &mut keyboard,
            &trackball_click,
            &trackball_up,
            &trackball_down,
            &trackball_left,
            &trackball_right,
            &mut input_state,
        );

        if app::handle_event(
            &mut screen,
            event,
            &volume_mgr,
            &launch,
            &mut system_info,
            screen_size,
        ) {
            needs_redraw = true;
        }

        if needs_redraw {
            render::draw_screen(&mut display, &screen, screen_size, &mut render_cache);
            needs_redraw = false;
        }

        let frame_start = Instant::now();
        while frame_start.elapsed() < Duration::from_millis(16) {}
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
