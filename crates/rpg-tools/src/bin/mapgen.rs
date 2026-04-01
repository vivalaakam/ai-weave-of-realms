//! Map generation tester.
//!
//! Generates a map from a seed phrase and produces:
//! - A PNG image saved to disk (coloured by tile type)
//! - A TMX tiled map referencing the root project tileset
//! - ASCII art printed to stdout
//! - Tile statistics

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;
use image::{ImageBuffer, Rgb};
use tracing::{error, info, warn};

use rpg_engine::map::game_map::GameMap;
use rpg_engine::map::tile::Tiles;
use rpg_mapgen::map_assembler::{MapAssembler, MapConfig};
use rpg_tiled::write_tmx;

const TILESET_PATH: &str = "tileset/tileset.tsx";

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
            error!(error = %e, "failed to initialise map assembler");
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
                    error!(error = %e, "map generation failed");
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            error!(error = %e, "map generation failed");
            std::process::exit(1);
        }
    };

    let output_dir = match create_generation_dir(&args.output) {
        Ok(path) => path,
        Err(e) => {
            error!(error = %e, root = %args.output.display(), "failed to create generation output directory");
            std::process::exit(1);
        }
    };

    let png_path = output_dir.join("map.png");
    let tmx_path = output_dir.join("map.tmx");
    let tileset_path = resolve_project_path(TILESET_PATH);

    // ── Print statistics ──────────────────────────────────────────────────────
    print_stats(&map);

    // ── ASCII art ─────────────────────────────────────────────────────────────
    if !args.no_ascii {
        print_ascii(&map);
    }

    save_png(&map, &png_path, args.scale);

    let tileset_rel = relative_path(tmx_path.parent().unwrap_or(&tmx_path), &tileset_path);
    if let Err(e) = write_tmx(&map, &tmx_path, &tileset_rel) {
        error!(error = %e, path = %tmx_path.display(), "failed to save TMX");
        std::process::exit(1);
    }

    if args.open {
        if let Err(e) = open_file(&tmx_path) {
            error!(error = %e, path = %tmx_path.display(), "failed to open generated TMX");
            std::process::exit(1);
        }
    }

    info!(
        output_dir = %output_dir.display(),
        png = %png_path.display(),
        tmx = %tmx_path.display(),
        "done"
    );
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

    /// Root directory where timestamped generation folders will be created.
    #[arg(long, default_value = "output")]
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

    /// Open the generated TMX file with the system default application.
    #[arg(long)]
    open: bool,
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

    match img.save(output) {
        Ok(_) => info!(path = %output.display(), width = w, height = h, "PNG saved"),
        Err(e) => error!(error = %e, path = %output.display(), "failed to save PNG"),
    }
}

/// Creates a new timestamped directory for this generation under `root`.
fn create_generation_dir(root: &Path) -> Result<PathBuf, std::io::Error> {
    let timestamp = generation_timestamp();
    let dir = root.join(timestamp);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Returns a monotonic-enough timestamp label suitable for directory names.
fn generation_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("gen-{}-{:03}", now.as_secs(), now.subsec_millis())
}

/// Resolves a repo-relative path against the current working directory.
fn resolve_project_path(relative: &str) -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(relative)
}

/// Computes a relative path from `from_dir` to `to_path`.
fn relative_path(from_dir: &std::path::Path, to_path: &std::path::Path) -> String {
    let from_abs = if from_dir.is_absolute() {
        from_dir.to_path_buf()
    } else {
        resolve_project_path(from_dir.to_string_lossy().as_ref())
    };
    let to_abs = if to_path.is_absolute() {
        to_path.to_path_buf()
    } else {
        resolve_project_path(to_path.to_string_lossy().as_ref())
    };

    let from_components: Vec<_> = from_abs.components().collect();
    let to_components: Vec<_> = to_abs.components().collect();

    let mut common_len = 0usize;
    while common_len < from_components.len()
        && common_len < to_components.len()
        && from_components[common_len] == to_components[common_len]
    {
        common_len += 1;
    }

    let mut rel = PathBuf::new();
    for _ in common_len..from_components.len() {
        rel.push("..");
    }
    for component in &to_components[common_len..] {
        rel.push(component.as_os_str());
    }

    rel.to_string_lossy().replace('\\', "/")
}

/// Opens a file using the default system handler.
fn open_file(path: &PathBuf) -> Result<(), std::io::Error> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = Command::new("open");
        cmd.arg(path);
        cmd
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(path);
        cmd
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", &path.to_string_lossy()]);
        cmd
    };

    let status = command.status()?;
    if status.success() {
        info!(path = %path.display(), "opened generated TMX");
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "open command exited with status {status}"
        )))
    }
}
