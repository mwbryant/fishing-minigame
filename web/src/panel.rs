//! The browser's panel: the presenter's RGB565 bands land as RGBA pixels the
//! page copies onto its canvas. The glass is round in CSS.

use embedded_graphics::pixelcolor::IntoStorage;
use orbit_gfx::{Panel, Rectangle, Rgb565, SCREEN_SIZE};

pub const SIZE: usize = SCREEN_SIZE as usize;
const BLACK: [u8; 4] = [0, 0, 0, 255];

pub struct RgbaPanel {
    pixels: Vec<u8>,
}

impl Default for RgbaPanel {
    fn default() -> Self {
        Self {
            pixels: BLACK.repeat(SIZE * SIZE),
        }
    }
}

impl RgbaPanel {
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// The switched-off panel.
    pub fn clear(&mut self) {
        for px in self.pixels.chunks_exact_mut(4) {
            px.copy_from_slice(&BLACK);
        }
    }
}

impl Panel for RgbaPanel {
    fn write(&mut self, area: Rectangle, bytes: &[u8]) {
        let width = area.size.width as usize;
        let (left, top) = (area.top_left.x as usize, area.top_left.y as usize);
        for (row, line) in bytes.chunks_exact(width * 2).enumerate() {
            let start = ((top + row) * SIZE + left) * 4;
            let out = &mut self.pixels[start..start + width * 4];
            for (rgba, px) in out.chunks_exact_mut(4).zip(line.chunks_exact(2)) {
                rgba.copy_from_slice(&expand(u16::from_be_bytes([px[0], px[1]])));
            }
        }
    }
}

/// `color` as the panel shows it.
pub fn rgba8(color: Rgb565) -> [u8; 4] {
    expand(color.into_storage())
}

fn expand(c: u16) -> [u8; 4] {
    let r = (c >> 11 & 0x1f) as u8;
    let g = (c >> 5 & 0x3f) as u8;
    let b = (c & 0x1f) as u8;
    [r << 3 | r >> 2, g << 2 | g >> 4, b << 3 | b >> 2, 255]
}

#[cfg(test)]
mod tests {
    use orbit_gfx::{Point, Size};

    use super::*;

    fn at(panel: &RgbaPanel, x: usize, y: usize) -> &[u8] {
        let i = (y * SIZE + x) * 4;
        &panel.pixels()[i..i + 4]
    }

    #[test]
    fn a_band_lands_as_rgba_where_the_presenter_sent_it() {
        let mut panel = RgbaPanel::default();
        let area = Rectangle::new(Point::new(10, 20), Size::new(2, 2));
        let red = 0xf800u16.to_be_bytes();
        let white = 0xffffu16.to_be_bytes();
        let bytes = [red, white, white, red].concat();
        panel.write(area, &bytes);
        assert_eq!(at(&panel, 10, 20), [255, 0, 0, 255]);
        assert_eq!(at(&panel, 11, 20), [255, 255, 255, 255]);
        assert_eq!(at(&panel, 10, 21), [255, 255, 255, 255]);
        assert_eq!(at(&panel, 11, 21), [255, 0, 0, 255]);
        assert_eq!(at(&panel, 12, 20), BLACK);
        assert_eq!(at(&panel, 10, 22), BLACK);
    }
}
