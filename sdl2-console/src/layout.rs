//! Window and logical render size calculations.

use game::prelude::render::Size;

const OUTPUT_SCALE: usize = 2;
const MIN_WINDOW_WIDTH: u32 = 320;
const MIN_WINDOW_HEIGHT: u32 = 240;

/// Calculate the minimum window size in *output* pixels.
pub fn minimum_output_size() -> Size {
    let logical_minimum = logical_render_size(Size::new(0, 0));
    Size::new(
        logical_minimum.width * OUTPUT_SCALE as u32,
        logical_minimum.height * OUTPUT_SCALE as u32,
    )
}

/// Clamp raw window dimensions to sensible minimums.
pub fn window_size(width: u32, height: u32) -> Size {
    let minimum = minimum_output_size();
    Size::new(
        width.max(MIN_WINDOW_WIDTH).max(minimum.width),
        height.max(MIN_WINDOW_HEIGHT).max(minimum.height),
    )
}

/// Derive logical framebuffer size from physical window size.
///
/// # Arguments
/// * `output_size` — Physical dimensions in pixels.
///
/// # Returns
/// Logical dimensions before output scaling.
pub fn logical_render_size(output_size: Size) -> Size {
    let minimum_width = 32;
    let minimum_height = crate::render::HEADER_HEIGHT + crate::render::FOOTER_HEIGHT + 32;
    Size::new(
        (output_size.width / OUTPUT_SCALE as u32).max(minimum_width),
        (output_size.height / OUTPUT_SCALE as u32).max(minimum_height),
    )
}
