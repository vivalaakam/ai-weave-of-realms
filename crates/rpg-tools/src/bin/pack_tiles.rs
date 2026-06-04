//! Packs all PNG tilesets in an assets directory into a single binary tile atlas.
//!
//! # Binary format
//!
//! ```text
//! [magic:  4 bytes] b"RPGT"
//! [tile_w: 1 byte ] u8  — tile width in pixels
//! [tile_h: 1 byte ] u8  — tile height in pixels
//! [count:  4 bytes] u32 LE — total number of tiles
//! [mask_0: N bytes]       — bitmask for tile 0 (N = ceil(tile_w * tile_h / 8))
//! [mask_1: N bytes]       — bitmask for tile 1
//! ...
//! ```
//!
//! Each bitmask stores pixels row by row, MSB-first.
//! Bit at (x, y) is at position `y * tile_w + x`; byte `bit/8`, bit `7 - bit%8`.
//!
//! PNG files are processed in alphabetical order. Within each PNG, tiles are
//! scanned left-to-right, top-to-bottom. Partial tile columns/rows at the image
//! edges are skipped. A pixel is "set" when its alpha channel >= 128.
//!
//! # Usage
//!
//! ```sh
//! cargo run -p rpg-tools --bin pack-tiles -- \
//!     --assets assets/ \
//!     --output assets/tiles.bin \
//!     --tile-width 16 \
//!     --tile-height 16
//! ```

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Parser;
use tracing::{error, info, warn};

#[derive(Parser, Debug)]
#[command(name = "pack-tiles", about = "Pack PNG tilesets into a binary tile atlas")]
struct Args {
    /// Directory containing PNG tileset files.
    #[arg(long, default_value = "assets")]
    assets: PathBuf,

    /// Output binary file path.
    #[arg(long, default_value = "assets/tiles.bin")]
    output: PathBuf,

    /// Tile width in pixels.
    #[arg(long, default_value_t = 14)]
    tile_width: u8,

    /// Tile height in pixels.
    #[arg(long, default_value_t = 14)]
    tile_height: u8,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    match run(&args) {
        Ok(total) => info!(
            tiles = total,
            output = %args.output.display(),
            "tile atlas written"
        ),
        Err(e) => {
            error!(error = %e, "pack-tiles failed");
            std::process::exit(1);
        }
    }
}

fn run(args: &Args) -> Result<usize, Box<dyn std::error::Error>> {
    let tile_w = args.tile_width as usize;
    let tile_h = args.tile_height as usize;
    let mask_bytes = (tile_w * tile_h).div_ceil(8);

    let mut pngs = collect_pngs(&args.assets)?;
    pngs.sort();

    if pngs.is_empty() {
        return Err(format!("no PNG files found in {}", args.assets.display()).into());
    }

    let mut masks: Vec<Vec<u8>> = Vec::new();

    for path in &pngs {
        match process_png(path, tile_w, tile_h, mask_bytes) {
            Ok(tiles) => {
                info!(
                    path = %path.display(),
                    tiles = tiles.len(),
                    "processed"
                );
                masks.extend(tiles);
            }
            Err(e) => {
                warn!(path = %path.display(), error = %e, "skipping PNG");
            }
        }
    }

    let total = masks.len();
    if total == 0 {
        return Err("no tiles extracted from any PNG".into());
    }

    write_atlas(&args.output, tile_w as u8, tile_h as u8, &masks)?;
    Ok(total)
}

fn collect_pngs(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    if !dir.is_dir() {
        return Err(format!("{} is not a directory", dir.display()).into());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("png"))
            .unwrap_or(false)
        {
            paths.push(path);
        }
    }
    Ok(paths)
}

fn process_png(
    path: &Path,
    tile_w: usize,
    tile_h: usize,
    mask_bytes: usize,
) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
    let img = image::open(path)?.into_rgba8();
    let img_w = img.width() as usize;
    let img_h = img.height() as usize;

    let cols = img_w / tile_w;
    let rows = img_h / tile_h;

    if cols == 0 || rows == 0 {
        return Err(format!(
            "image {}×{} is smaller than one tile ({}×{})",
            img_w, img_h, tile_w, tile_h
        )
        .into());
    }

    let mut tiles = Vec::with_capacity(cols * rows);

    for row in 0..rows {
        for col in 0..cols {
            let sx = col * tile_w;
            let sy = row * tile_h;
            let mask = extract_mask(&img, sx, sy, tile_w, tile_h, mask_bytes);
            tiles.push(mask);
        }
    }

    Ok(tiles)
}

fn extract_mask(
    img: &image::RgbaImage,
    sx: usize,
    sy: usize,
    tile_w: usize,
    tile_h: usize,
    mask_bytes: usize,
) -> Vec<u8> {
    let mut mask = vec![0u8; mask_bytes];
    for y in 0..tile_h {
        for x in 0..tile_w {
            let pixel = img.get_pixel((sx + x) as u32, (sy + y) as u32);
            if pixel[3] >= 128 {
                let bit = y * tile_w + x;
                mask[bit / 8] |= 1 << (7 - bit % 8);
            }
        }
    }
    mask
}

fn write_atlas(
    path: &Path,
    tile_w: u8,
    tile_h: u8,
    masks: &[Vec<u8>],
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let count = masks.len() as u32;
    let mut out = Vec::new();

    // Header
    out.extend_from_slice(b"RPGT");
    out.push(tile_w);
    out.push(tile_h);
    out.extend_from_slice(&count.to_le_bytes());

    // Tile masks
    for mask in masks {
        out.extend_from_slice(mask);
    }

    let mut file = fs::File::create(path)?;
    file.write_all(&out)?;

    info!(
        bytes = out.len(),
        tiles = count,
        tile_w,
        tile_h,
        mask_bytes = out.len().saturating_sub(10) / count.max(1) as usize,
        "atlas header: RPGT + u8 tile_w + u8 tile_h + u32 count, then count × mask_bytes"
    );

    Ok(())
}
