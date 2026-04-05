# T-Deck Prototype

Minimal Rust embedded project for the LilyGO T-Deck.

Current behavior:
- `cargo run` builds for `xtensa-esp32s3-none-elf`
- `espflash` immediately flashes the firmware to the connected board
- boot screen waits for `Enter`
- after `Enter`, the firmware reads `/maps/*.tmx` from the SD card and shows a selectable list
- `Enter` loads the selected map
- only the visible tile area is rendered on screen
- trackball or `WASD` pans the loaded map viewport

## Requirements

- LilyGO T-Deck connected over USB
- `espflash` installed
- Rust `esp` toolchain installed

## Run

```sh
cd tdeck
cargo run
```

## SD Card Layout

Place generated TMX maps on the SD card in:

```txt
/maps
```

Example:

```txt
/maps/default-seed-terrain-96x96.tmx
```

## Controls

- Splash: `Enter`
- Map select: trackball up/down or `W`/`S`, `Enter` to load
- Map view: trackball or `W`/`A`/`S`/`D` to pan, `Backspace`/`Esc` to return

## Start With A Specific Map

Bare-metal `no_std` firmware does not have normal runtime `argv`, so direct launch is implemented with compile-time environment variables:

```sh
cd tdeck
TDECK_START_MAP=default-seed-terrain-96x96.tmx cargo run
```

Optional initial viewport:

```sh
cd tdeck
TDECK_START_MAP=default-seed-terrain-96x96.tmx TDECK_VIEW_X=8 TDECK_VIEW_Y=12 cargo run
```

The runner is configured in `.cargo/config.toml` as:

```toml
linker = "/Users/vivalaakam/.rustup/toolchains/esp/xtensa-esp-elf/esp-15.2.0_20250920/xtensa-esp-elf/bin/xtensa-esp32s3-elf-gcc"
runner = "espflash flash --chip esp32s3"
```

If you want a serial monitor after flashing, run:

```sh
espflash monitor
```

The display pinout and boot sequence are based on:
- `joshmarinacci/rust-tdeck-experiments`
