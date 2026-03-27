//! Map generation tester.
//!
//! Generates a map from a seed phrase and produces:
//! - A PNG image saved to disk (coloured by tile type)
//! - ASCII art printed to stdout
//! - Tile statistics

use std::collections::HashMap;
use std::path::PathBuf;

use clap::Parser;
use image::{ImageBuffer, Rgb};
use tracing::{info, warn};

use rpg_engine::map::game_map::GameMap;
use rpg_engine::map::tile::Tiles;
use rpg_mapgen::map_assembler::{MapAssembler, MapConfig};

fn main() {
    // ── Init tracing ──────────────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // ── Parse CLI args ────────────────────────────────────────────────────────
    let args = Args::parse();

    info!(seed = %args.seed, "starting map generation");

    // ── Build generator pipeline ──────────────────────────────────────────────
    let mut generators: Vec<PathBuf> = args.generators.clone();

    // Fall back to default script if no generators were specified
    if generators.is_empty() {
        generators.push(PathBuf::from("scripts/generators/default.lua"));
    }

    // ── Build config ──────────────────────────────────────────────────────────
    let first = generators.remove(0);
    let mut config = MapConfig::default_3x3(args.seed.clone(), first);
    config.width = args.width;
    config.height = args.height;
    for g in generators {
        config = config.with_generator(g);
    }
    config.validator_dir = args.validator_dir.clone();
    config.validator_script = args.validator.clone();
    config.evaluator_script = args.evaluator.clone();

    // If no validator dir or script given, try default paths
    if config.validator_dir.is_none() {
        let default = PathBuf::from("scripts/rules");
        if default.is_dir() {
            config.validator_dir = Some(default);
        }
    }
    if config.evaluator_script.is_none() {
        let default = PathBuf::from("scripts/evaluators/evaluate.lua");
        if default.exists() {
            config.evaluator_script = Some(default);
        }
    }

    // ── Assemble map ──────────────────────────────────────────────────────────
    let assembler = match MapAssembler::new(config) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: failed to initialise map assembler: {e}");
            std::process::exit(1);
        }
    };

    let map = match assembler.generate_validated() {
        Ok(m) => m,
        Err(rpg_mapgen::error::Error::ValidationFailed(reason)) => {
            warn!(%reason, "map failed validation; trying without validator");
            match assembler.generate() {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("error: map generation failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("error: map generation failed: {e}");
            std::process::exit(1);
        }
    };

    // ── Print statistics ──────────────────────────────────────────────────────
    print_stats(&map);

    // ── ASCII art ─────────────────────────────────────────────────────────────
    if !args.no_ascii {
        print_ascii(&map);
    }

    // ── Save PNG ──────────────────────────────────────────────────────────────
    save_png(&map, &args.output, args.scale);

    info!(path = %args.output.display(), "done");
}

// ─── CLI Args ─────────────────────────────────────────────────────────────────

/// Map generation tester for the AI RPG project.
///
/// Generates a procedural map from a seed phrase and outputs:
/// a PNG image, ASCII art, and tile statistics.
///
/// ## Pipeline generators
///
/// Pass one or more `--generator path` flags to build the generator pipeline.
/// Generators are applied to every chunk in the order they are specified.
/// Each subsequent generator receives the tiles from the previous stage as
/// its 4th Lua argument.
///
/// If no `--generator` flags are given, `scripts/generators/default.lua` is used.
#[derive(Parser, Debug)]
#[command(name = "mapgen", author, version, about, long_about = None)]
struct Args {
    /// Seed phrase used for deterministic map generation.
    #[arg(long, default_value = "default-seed")]
    seed: String,

    /// Output path for the generated PNG image.
    #[arg(long, default_value = "output/map.png")]
    output: PathBuf,

    /// Map width in tiles (must be a multiple of 32).
    #[arg(long, default_value_t = 32)]
    width: u32,

    /// Map height in tiles (must be a multiple of 32).
    #[arg(long, default_value_t = 32)]
    height: u32,

    /// Generator script path (repeatable, applied as a pipeline in order).
    /// Example: --generator scripts/generators/default.lua --generator scripts/generators/forest.lua
    #[arg(long = "generator", value_name = "SCRIPT")]
    generators: Vec<PathBuf>,

    /// Path to a directory of validation rule .lua files (takes precedence over --validator).
    #[arg(long)]
    validator_dir: Option<PathBuf>,

    /// Path to the Lua validator script (optional; ignored if --validator-dir is set).
    #[arg(long)]
    validator: Option<PathBuf>,

    /// Path to the Lua evaluator script (optional).
    #[arg(long)]
    evaluator: Option<PathBuf>,

    /// Pixels per tile in the output PNG (higher = larger image).
    #[arg(long, default_value_t = 8)]
    scale: u32,

    /// Skip ASCII art output to stdout.
    #[arg(long)]
    no_ascii: bool,
}

// ─── Statistics ───────────────────────────────────────────────────────────────

/// Counts tiles and prints a statistics summary to stdout.
fn print_stats(map: &GameMap) {
    let total = (map.tile_width() * map.tile_height()) as usize;
    let mut counts: HashMap<Tiles, usize> = HashMap::new();

    for tile in map.tiles() {
        *counts.entry(tile.kind).or_insert(0) += 1;
    }

    println!("\n╔═══════════════════════════════════╗");
    println!("║         Map Statistics            ║");
    println!("╠═══════════════════════════════════╣");
    println!(
        "║  Size:   {}×{} tiles",
        map.tile_width(),
        map.tile_height()
    );
    println!("║  Total:  {total} tiles");
    println!("╠═══════════════════════════════════╣");

    let mut sorted: Vec<_> = counts.iter().collect();
    sorted.sort_by_key(|(_, &n)| std::cmp::Reverse(n));

    for (tile, count) in &sorted {
        let pct = **count as f32 / total as f32 * 100.0;
        let bar_len = (pct / 2.0) as usize;
        let bar = "█".repeat(bar_len);
        let pass = if tile.is_passable() { "✓" } else { "✗" };
        println!(
            "║  {pass} {:12}  {:5} ({:5.1}%) {}",
            tile.as_str(),
            count,
            pct,
            bar
        );
    }

    let passable: usize = counts
        .iter()
        .filter(|(t, _)| t.is_passable())
        .map(|(_, n)| n)
        .sum();
    let pct = passable as f32 / total as f32 * 100.0;
    println!("╠═══════════════════════════════════╣");
    println!("║  Passable: {passable}/{total} ({pct:.1}%)");
    println!("╚═══════════════════════════════════╝\n");
}

// ─── ASCII art ────────────────────────────────────────────────────────────────

/// Prints the full map as ASCII art.
///
/// For large maps, only the first 64 rows are printed to avoid flooding the terminal.
fn print_ascii(map: &GameMap) {
    let w = map.tile_width() as usize;
    let h = map.tile_height() as usize;
    let max_rows = h.min(64);

    println!("─── ASCII Map (first {max_rows} rows) ───");

    for y in 0..max_rows {
        let mut row = String::with_capacity(w + 1);
        for x in 0..w {
            let coord = rpg_engine::map::game_map::MapCoord::new(x as u32, y as u32);
            if let Ok(tile) = map.get_tile(coord) {
                row.push(tile.kind.as_char());
            } else {
                row.push('?');
            }
        }
        println!("{row}");
    }

    if h > max_rows {
        println!("... ({} rows not shown)", h - max_rows);
    }
    println!();
}

// ─── PNG export ───────────────────────────────────────────────────────────────

/// Saves the map as a PNG image where each tile is `scale × scale` pixels.
///
/// Creates the output directory if it does not exist.
fn save_png(map: &GameMap, output: &PathBuf, scale: u32) {
    let w = map.tile_width() * scale;
    let h = map.tile_height() * scale;
    let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(w, h);

    for ty in 0..map.tile_height() {
        for tx in 0..map.tile_width() {
            let coord = rpg_engine::map::game_map::MapCoord::new(tx, ty);
            let tile = match map.get_tile(coord) {
                Ok(t) => t,
                Err(_) => continue,
            };
            let (r, g, b) = tile.kind.as_color();
            let color = Rgb([r, g, b]);

            // Fill the scale×scale pixel block for this tile
            for dy in 0..scale {
                for dx in 0..scale {
                    let px = tx * scale + dx;
                    let py = ty * scale + dy;
                    img.put_pixel(px, py, color);
                }
            }
        }
    }

    // Create output directory if needed
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!("warning: could not create output directory: {e}");
            }
        }
    }

    match img.save(output) {
        Ok(_) => info!(path = %output.display(), width = w, height = h, "PNG saved"),
        Err(e) => eprintln!("error: failed to save PNG: {e}"),
    }
}
