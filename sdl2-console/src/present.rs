use sdl2::pixels::PixelFormatEnum;

use crate::error::HostError;
use crate::frame_buffer::Framebuffer;

const OUTPUT_SCALE: usize = 2;

/// Copy the RGB framebuffer into an SDL2 texture and present it on the canvas.
///
/// # Arguments
/// * `canvas`           — SDL2 render canvas.
/// * `texture_creator`  — SDL2 texture factory.
/// * `framebuffer`      — Logical pixel buffer to upload.
///
/// # Errors
/// Returns `Err` if SDL2 texture creation or update fails.
pub fn present_frame(
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    texture_creator: &sdl2::render::TextureCreator<sdl2::video::WindowContext>,
    framebuffer: &Framebuffer,
) -> Result<(), HostError> {
    let mut texture = texture_creator
        .create_texture_streaming(
            PixelFormatEnum::RGB24,
            framebuffer.size.width * OUTPUT_SCALE as u32,
            framebuffer.size.height * OUTPUT_SCALE as u32,
        )
        .map_err(|e| HostError::Message(e.to_string()))?;
    let bytes = framebuffer.rgb_bytes_scaled(OUTPUT_SCALE);
    texture
        .update(None, &bytes, framebuffer.size.width as usize * OUTPUT_SCALE * 3)
        .map_err(|e| HostError::Message(e.to_string()))?;
    canvas.clear();
    canvas.copy(&texture, None, None).map_err(|e| HostError::Message(e.to_string()))?;
    canvas.present();
    Ok(())
}
