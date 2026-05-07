//! Screen state definitions for the T-Deck app.

mod map_view;

use alloc::boxed::Box;

pub use map_view::{MapViewScreen, SaveOverlay};
pub use rpg_embedded::list::ListScreen;
pub use rpg_embedded::splash::SplashScreen;

/// Top-level screen state.
pub enum Screen {
    /// Initial splash screen.
    Splash(SplashScreen),
    /// Map selection screen backed by SD card content.
    MapSelect(ListScreen),
    /// Save selection screen backed by SD card content.
    SaveSelect(ListScreen),
    /// Active gameplay and map rendering screen.
    MapView(Box<MapViewScreen>),
}
