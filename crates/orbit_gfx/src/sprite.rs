use core::{convert::Infallible, fmt};

use embedded_graphics::{pixelcolor::Rgb565, prelude::*, primitives::Rectangle};

use crate::Canvas;

/// Pixel art authored as text: each row is a string of palette keys, and any
/// byte missing from the palette (by convention `.`) is transparent. Sprites
/// live in flash as `static`s, so they cost no RAM on the device.
pub struct Sprite {
    pub rows: &'static [&'static str],
    pub palette: &'static [(u8, Rgb565)],
}

impl Sprite {
    pub fn size(&self) -> Size {
        let width = self.rows.iter().map(|row| row.len()).max().unwrap_or(0);
        Size::new(width as u32, self.rows.len() as u32)
    }

    fn color(&self, key: u8) -> Option<Rgb565> {
        self.palette
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, color)| *color)
    }
}

/// A sprite reference compared by identity, so diffing frames never walks the
/// pixel data.
#[derive(Clone, Copy)]
pub struct SpriteRef(pub &'static Sprite);

impl PartialEq for SpriteRef {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self.0, other.0)
    }
}

impl fmt::Debug for SpriteRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sprite({:p}, {:?})", self.0, self.0.size())
    }
}

pub(crate) fn sprite_rect(sprite: &Sprite, top_left: Point, scale: u32) -> Rectangle {
    Rectangle::new(top_left, sprite.size() * scale)
}

/// Draws only the source rows that land inside the canvas, each pixel as a
/// `scale`-sized block.
pub(crate) fn draw_sprite(
    canvas: &mut Canvas,
    sprite: &Sprite,
    top_left: Point,
    scale: u32,
    flip_x: bool,
) -> Result<(), Infallible> {
    let area = canvas.area();
    let size = sprite.size();
    let scale_i = scale as i32;
    let first = ((area.top_left.y - top_left.y) / scale_i).max(0) as usize;
    let last = ((area.rows().end - top_left.y + scale_i - 1) / scale_i).max(0) as usize;
    let block = Size::new_equal(scale);
    for (y, row) in sprite
        .rows
        .iter()
        .enumerate()
        .take(last.min(size.height as usize))
        .skip(first)
    {
        for (x, key) in row.bytes().enumerate() {
            let Some(color) = sprite.color(key) else {
                continue;
            };
            let column = if flip_x {
                size.width as usize - 1 - x
            } else {
                x
            };
            let origin = top_left + Point::new(column as i32 * scale_i, y as i32 * scale_i);
            canvas.fill_solid(&Rectangle::new(origin, block), color)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use embedded_graphics::pixelcolor::RgbColor;

    use super::*;

    static ARROW: Sprite = Sprite {
        rows: &["R..", "RG.", "RGB"],
        palette: &[
            (b'R', Rgb565::RED),
            (b'G', Rgb565::GREEN),
            (b'B', Rgb565::BLUE),
        ],
    };
    static ARROW_TWIN: Sprite = Sprite {
        rows: &["R..", "RG.", "RGB"],
        palette: &[(b'R', Rgb565::RED)],
    };

    fn drawn(scale: u32, flip_x: bool) -> [[Option<Rgb565>; 9]; 9] {
        let mut buf = vec![0u8; 9 * 9 * 2];
        let mut canvas = Canvas::new(Rectangle::new(Point::zero(), Size::new(9, 9)), &mut buf);
        draw_sprite(&mut canvas, &ARROW, Point::zero(), scale, flip_x).unwrap();
        let mut out = [[None; 9]; 9];
        for (y, row) in out.iter_mut().enumerate() {
            for (x, px) in row.iter_mut().enumerate() {
                *px = canvas
                    .pixel(Point::new(x as i32, y as i32))
                    .filter(|&c| c != Rgb565::BLACK);
            }
        }
        out
    }

    #[test]
    fn unmapped_keys_are_transparent() {
        let px = drawn(1, false);
        assert_eq!(px[0][0], Some(Rgb565::RED));
        assert_eq!(px[0][1], None);
        assert_eq!(px[2][2], Some(Rgb565::BLUE));
    }

    #[test]
    fn each_source_pixel_becomes_a_scale_sized_block() {
        let px = drawn(3, false);
        assert!((0..3).all(|y| (0..3).all(|x| px[y][x] == Some(Rgb565::RED))));
        assert_eq!(px[3][3], Some(Rgb565::GREEN));
        assert_eq!(px[0][3], None);
    }

    #[test]
    fn flipping_mirrors_columns() {
        let px = drawn(1, true);
        assert_eq!(px[0][2], Some(Rgb565::RED));
        assert_eq!(px[2][0], Some(Rgb565::BLUE));
        assert_eq!(px[0][0], None);
    }

    #[test]
    fn sprite_refs_compare_by_identity_not_content() {
        assert_eq!(SpriteRef(&ARROW), SpriteRef(&ARROW));
        assert_ne!(SpriteRef(&ARROW), SpriteRef(&ARROW_TWIN));
    }
}
