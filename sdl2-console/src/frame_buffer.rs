use crate::AppResult;
use game::prelude::render::{DrawTarget, OriginDimensions, Pixel, Rgb888, RgbColor, Size};

pub struct Framebuffer {
    pub(crate) size: Size,
    pub(crate) pixels: Vec<Rgb888>,
}

impl Framebuffer {
    pub fn new(size: Size, background: Rgb888) -> AppResult<Self> {
        let len = usize::try_from(size.width)
            .ok()
            .and_then(|width| {
                usize::try_from(size.height).ok().map(|height| width.saturating_mul(height))
            })
            .ok_or_else(|| "framebuffer dimensions overflow".to_string())?;
        Ok(Self { size, pixels: vec![background; len] })
    }

    pub fn rgb_bytes_scaled(&self, scale: usize) -> Vec<u8> {
        let src_width = self.size.width as usize;
        let src_height = self.size.height as usize;
        let mut bytes = Vec::with_capacity(src_width * src_height * scale * scale * 3);
        for y in 0..src_height {
            let row_start = y * src_width;
            let row = &self.pixels[row_start..row_start + src_width];
            for _ in 0..scale {
                for color in row {
                    for _ in 0..scale {
                        bytes.push(color.r());
                        bytes.push(color.g());
                        bytes.push(color.b());
                    }
                }
            }
        }
        bytes
    }
}

impl OriginDimensions for Framebuffer {
    fn size(&self) -> Size {
        self.size
    }
}

impl DrawTarget for Framebuffer {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item=Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x < 0
                || point.y < 0
                || point.x >= self.size.width as i32
                || point.y >= self.size.height as i32
            {
                continue;
            }
            let index = point.y as usize * self.size.width as usize + point.x as usize;
            if let Some(pixel) = self.pixels.get_mut(index) {
                *pixel = color;
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.pixels.fill(color);
        Ok(())
    }
}
