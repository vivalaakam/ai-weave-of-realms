use clap::Parser;
use std::path::PathBuf;

#[derive(Clone, Debug, Parser)]
#[command(name = "weave-of-realms-sdl2", author, version, about)]
pub struct Args {
    /// Load a saved game state from an .rpgs file.
    #[arg(long)]
    pub save: Option<PathBuf>,

    /// Load a TMX map instead of generating one.
    #[arg(long)]
    pub tmx: Option<PathBuf>,

    /// Seed phrase for deterministic generation.
    #[arg(long, default_value = "default-seed")]
    pub seed: String,

    /// Map width in tiles when generating.
    #[arg(long, default_value_t = 96)]
    pub width: u32,

    /// Map height in tiles when generating.
    #[arg(long, default_value_t = 96)]
    pub height: u32,

    /// Generator script path (repeatable pipeline).
    #[arg(long = "generator", value_name = "SCRIPT")]
    pub generators: Vec<PathBuf>,

    /// Directory with validation rule scripts.
    #[arg(long)]
    pub validator_dir: Option<PathBuf>,

    /// Path to a single Lua validator script.
    #[arg(long)]
    pub validator: Option<PathBuf>,

    /// Path to the Lua evaluator script.
    #[arg(long)]
    pub evaluator: Option<PathBuf>,
    /// Start in windowed mode instead of fullscreen.
    #[arg(long = "windowed")]
    pub window_mode: bool,
}
