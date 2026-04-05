//! Splash screen state.

use alloc::string::String;

/// Splash screen model.
pub struct SplashScreen {
    /// Optional status line shown at the bottom of the screen.
    pub status: Option<String>,
}
