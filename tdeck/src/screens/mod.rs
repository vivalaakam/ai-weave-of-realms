//! Screen state definitions for the T-Deck app.

mod map_select;
mod map_view;
mod splash;

pub use map_select::MapSelectScreen;
pub use map_view::{InteractionMode, MapViewScreen, SaveOverlay};
pub use splash::SplashScreen;

/// Top-level screen state.
pub enum Screen {
    /// Initial splash screen.
    Splash(SplashScreen),
    /// Map selection screen backed by SD card content.
    MapSelect(MapSelectScreen),
    /// Save selection screen backed by SD card content.
    SaveSelect(MapSelectScreen),
    /// Active gameplay and map rendering screen.
    MapView(MapViewScreen),
}
