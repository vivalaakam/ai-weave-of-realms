use crate::args::Args;
use game::app_host::AppHost;
use game::prelude::render::Size;
use std::path::Path;

pub struct ConsoleHost {
    pub args: Args,
    pub screen_size: Size,
}

impl AppHost for ConsoleHost {
    fn get_maps_dir(&self) -> &Path {
        Path::new("maps")
    }

    fn get_saves_dir(&self) -> &Path {
        Path::new("savegame")
    }

    fn get_seed(&self) -> &str {
        self.args.seed.as_str()
    }

    fn get_width(&self) -> u32 {
        self.args.width
    }

    fn get_height(&self) -> u32 {
        self.args.height
    }

    fn get_screen_width(&self) -> u32 {
        self.screen_size.width
    }

    fn get_screen_height(&self) -> u32 {
        self.screen_size.height
    }

    fn get_generator(&self) -> Option<&Path> {
        self.args.generators.as_deref()
    }

    fn get_validator_dir(&self) -> Option<&Path> {
        self.args.validator_dir.as_deref()
    }

    fn get_validator(&self) -> Option<&Path> {
        self.args.validator.as_deref()
    }

    fn get_evaluator(&self) -> Option<&Path> {
        self.args.evaluator.as_deref()
    }
}
