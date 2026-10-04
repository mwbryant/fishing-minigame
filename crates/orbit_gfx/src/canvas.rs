use core::convert::Infallible;

use embedded_graphics::{
    Pixel,
    pixelcolor::{Rgb565, raw::RawU16},
    prelude::*,
    primitives::Rectangle,
};

/// A window of the screen backed by a borrowed big-endian RGB565 byte buffer.
/// Drawing outside `area` is clipped.
pub struct Canvas<'a> {
    area: Rectangle,
    bytes: &'a mut [u8],
}

impl<'a> Canvas<'a> {
    /// `bytes` may be longer than needed; only the leading `width * height * 2`
    /// bytes are used, so one band buffer can serve areas of any size.
    pub fn new(area: Rectangle, bytes: &'a mut [u8]) -> Self {
        let len = area.size.width as usize * area.size.height as usize * 2;
        assert!(bytes.len() >= len, "canvas buffer too small for {area:?}");
        Self {
            area,
            bytes: &mut bytes[..len],
        }
    }

    pub fn area(&self) -> Rectangle {
        self.area
    }

    pub fn bytes(&self) -> &[u8] {
        self.bytes
    }

    pub fn pixel(&self, p: Point) -> Option<Rgb565> {
        let i = self.offset(p)?;
        let raw = u16::from_be_bytes([self.bytes[i], self.bytes[i + 1]]);
        Some(RawU16::new(raw).into())
    }

    fn offset(&self, p: Point) -> Option<usize> {
        if !self.area.contains(p) {
            return None;
        }
        let local = p - self.area.top_left;
        Some((local.y as usize * self.area.size.width as usize + local.x as usize) * 2)
    }
}

impl Dimensions for Canvas<'_> {
    fn bounding_box(&self) -> Rectangle {
        self.area
    }
}

impl DrawTarget for Canvas<'_> {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(p, color) in pixels {
            if let Some(i) = self.offset(p) {
                let [hi, lo] = RawU16::from(color).into_inner().to_be_bytes();
                self.bytes[i] = hi;
                self.bytes[i + 1] = lo;
            }
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let clipped = area.intersection(&self.area);
        let Some(bottom_right) = clipped.bottom_right() else {
            return Ok(());
        };
        let [hi, lo] = RawU16::from(color).into_inner().to_be_bytes();
        let stride = self.area.size.width as usize * 2;
        let x0 = (clipped.top_left.x - self.area.top_left.x) as usize * 2;
        let x1 = (bottom_right.x - self.area.top_left.x + 1) as usize * 2;
        for y in clipped.top_left.y..=bottom_right.y {
            let row = (y - self.area.top_left.y) as usize * stride;
            for px in self.bytes[row + x0..row + x1].chunks_exact_mut(2) {
                px[0] = hi;
                px[1] = lo;
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.fill_solid(&self.area.clone(), color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas_at(x: i32, y: i32, w: u32, h: u32, buf: &mut [u8]) -> Canvas<'_> {
        Canvas::new(Rectangle::new(Point::new(x, y), Size::new(w, h)), buf)
    }

    #[test]
    fn pixels_are_stored_big_endian_at_screen_coordinates() {
        let mut buf = [0u8; 4 * 3 * 2];
        let mut canvas = canvas_at(10, 20, 4, 3, &mut buf);
        Pixel(Point::new(11, 21), Rgb565::new(0b10101, 0b110011, 0b01110))
            .draw(&mut canvas)
            .unwrap();
        let i = (4 + 1) * 2;
        assert_eq!(
            [buf[i], buf[i + 1]],
            (0b10101u16 << 11 | 0b110011 << 5 | 0b01110).to_be_bytes()
        );
    }

    #[test]
    fn drawing_outside_the_area_is_clipped() {
        let mut buf = [0u8; 2 * 2 * 2];
        let mut canvas = canvas_at(5, 5, 2, 2, &mut buf);
        Pixel(Point::new(4, 5), Rgb565::WHITE)
            .draw(&mut canvas)
            .unwrap();
        canvas
            .fill_solid(
                &Rectangle::new(Point::new(0, 0), Size::new(5, 100)),
                Rgb565::WHITE,
            )
            .unwrap();
        assert!(buf.iter().all(|&b| b == 0));
    }

    #[test]
    fn fill_solid_covers_only_the_overlap() {
        let mut buf = [0u8; 4 * 4 * 2];
        let mut canvas = canvas_at(0, 0, 4, 4, &mut buf);
        canvas
            .fill_solid(
                &Rectangle::new(Point::new(2, 2), Size::new(10, 10)),
                Rgb565::RED,
            )
            .unwrap();
        assert_eq!(canvas.pixel(Point::new(3, 3)), Some(Rgb565::RED));
        assert_eq!(canvas.pixel(Point::new(2, 2)), Some(Rgb565::RED));
        assert_eq!(canvas.pixel(Point::new(1, 3)), Some(Rgb565::BLACK));
        assert_eq!(canvas.pixel(Point::new(3, 1)), Some(Rgb565::BLACK));
    }

    #[test]
    fn oversized_buffers_are_trimmed_to_the_area() {
        let mut buf = [0u8; 100];
        let canvas = canvas_at(0, 0, 3, 2, &mut buf);
        assert_eq!(canvas.bytes().len(), 12);
    }
}
