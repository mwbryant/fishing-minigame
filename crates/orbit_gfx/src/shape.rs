use alloc::string::String;
use core::convert::Infallible;

use embedded_graphics::{
    Pixel,
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};

use crate::{
    Canvas,
    arc::draw_arc,
    sprite::{SpriteRef, draw_sprite, draw_sprite_fit, sprite_rect},
};

const GLYPH_WIDTH: u32 = 10;
const GLYPH_HEIGHT: u32 = 20;

/// One drawable item of a frame. Angles are degrees clockwise from 3 o'clock
/// (screen space, y down), matching `atan2(dy, dx)` on screen coordinates.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Disc {
        center: Point,
        radius: u32,
        color: Rgb565,
    },
    Ring {
        center: Point,
        radius: u32,
        width: u32,
        color: Rgb565,
    },
    Arc {
        center: Point,
        radius: u32,
        width: u32,
        start_deg: f32,
        sweep_deg: f32,
        color: Rgb565,
    },
    /// ASCII text in a 10x20 mono font, each font pixel drawn `scale` px wide.
    Label {
        center: Point,
        text: String,
        scale: u32,
        color: Rgb565,
    },
    Rect {
        area: Rectangle,
        color: Rgb565,
    },
    Line {
        from: Point,
        to: Point,
        width: u32,
        color: Rgb565,
    },
    Sprite {
        sprite: SpriteRef,
        top_left: Point,
        scale: u32,
        flip_x: bool,
    },
    /// A sprite stretched to fill `area`, e.g. squashed to fake a 3D spin.
    SpriteFit {
        sprite: SpriteRef,
        area: Rectangle,
        flip_x: bool,
    },
}

impl Shape {
    /// Conservative screen-space box containing every pixel the shape draws.
    pub fn bounds(&self) -> Rectangle {
        match self {
            Shape::Disc { center, radius, .. } => square_around(*center, *radius + 1),
            Shape::Ring {
                center,
                radius,
                width,
                ..
            } => square_around(*center, *radius + width / 2 + 1),
            Shape::Arc {
                center,
                radius,
                width,
                start_deg,
                sweep_deg,
                ..
            } => arc_bounds(*center, *radius, *width, *start_deg, *sweep_deg),
            Shape::Label {
                center,
                text,
                scale,
                ..
            } => label_rect(*center, text, *scale),
            Shape::Rect { area, .. } => *area,
            Shape::Line {
                from, to, width, ..
            } => {
                let pad = (*width / 2 + 1) as i32;
                Rectangle::with_corners(
                    Point::new(from.x.min(to.x) - pad, from.y.min(to.y) - pad),
                    Point::new(from.x.max(to.x) + pad, from.y.max(to.y) + pad),
                )
            }
            Shape::Sprite {
                sprite,
                top_left,
                scale,
                ..
            } => sprite_rect(sprite.0, *top_left, *scale),
            Shape::SpriteFit { area, .. } => *area,
        }
    }

    pub fn draw(&self, canvas: &mut Canvas) {
        let Ok(()) = match self {
            Shape::Disc {
                center,
                radius,
                color,
            } => Circle::with_center(*center, radius * 2 + 1)
                .into_styled(PrimitiveStyle::with_fill(*color))
                .draw(canvas),
            Shape::Ring {
                center,
                radius,
                width,
                color,
            } => Circle::with_center(*center, radius * 2 + 1)
                .into_styled(PrimitiveStyle::with_stroke(*color, *width))
                .draw(canvas),
            Shape::Arc {
                center,
                radius,
                width,
                start_deg,
                sweep_deg,
                color,
            } => draw_arc(
                canvas,
                arc_bounds(*center, *radius, *width, *start_deg, *sweep_deg),
                *center,
                *radius,
                *width,
                *start_deg,
                *sweep_deg,
                *color,
            ),
            Shape::Label {
                center,
                text,
                scale,
                color,
            } => draw_label(canvas, *center, text, *scale, *color),
            Shape::Rect { area, color } => canvas.fill_solid(area, *color),
            Shape::Line {
                from,
                to,
                width,
                color,
            } => Line::new(*from, *to)
                .into_styled(PrimitiveStyle::with_stroke(*color, *width))
                .draw(canvas),
            Shape::Sprite {
                sprite,
                top_left,
                scale,
                flip_x,
            } => draw_sprite(canvas, sprite.0, *top_left, *scale, *flip_x),
            Shape::SpriteFit {
                sprite,
                area,
                flip_x,
            } => draw_sprite_fit(canvas, sprite.0, *area, *flip_x),
        };
    }
}

/// Clears the canvas to `background` and draws every shape that touches it.
/// Rendering a list into adjacent canvases produces the same pixels as one
/// canvas covering both, which is what lets the firmware render in bands.
pub fn render(shapes: &[Shape], background: Rgb565, canvas: &mut Canvas) {
    let Ok(()) = canvas.clear(background);
    let area = canvas.area();
    for shape in shapes {
        if !shape.bounds().intersection(&area).is_zero_sized() {
            shape.draw(canvas);
        }
    }
}

fn square_around(center: Point, half: u32) -> Rectangle {
    Rectangle::with_center(center, Size::new_equal(half * 2 + 1))
}

fn arc_bounds(center: Point, radius: u32, width: u32, start_deg: f32, sweep_deg: f32) -> Rectangle {
    let outer = (radius + width / 2 + 1) as f32;
    if sweep_deg.abs() >= 360.0 {
        return square_around(center, outer as u32);
    }
    let inner = radius.saturating_sub(width / 2 + 1) as f32;
    let (start, end) = if sweep_deg < 0.0 {
        (start_deg + sweep_deg, start_deg)
    } else {
        (start_deg, start_deg + sweep_deg)
    };

    let (mut min_x, mut min_y) = (f32::MAX, f32::MAX);
    let (mut max_x, mut max_y) = (f32::MIN, f32::MIN);
    let mut include = |deg: f32, r: f32| {
        let (sin, cos) = libm::sincosf(deg.to_radians());
        min_x = min_x.min(r * cos);
        max_x = max_x.max(r * cos);
        min_y = min_y.min(r * sin);
        max_y = max_y.max(r * sin);
    };
    for deg in [start, end] {
        include(deg, outer);
        include(deg, inner);
    }
    let mut cardinal = libm::ceilf(start / 90.0) * 90.0;
    while cardinal <= end {
        include(cardinal, outer);
        cardinal += 90.0;
    }

    let pad = 2;
    let top_left = center
        + Point::new(
            libm::floorf(min_x) as i32 - pad,
            libm::floorf(min_y) as i32 - pad,
        );
    let bottom_right = center
        + Point::new(
            libm::ceilf(max_x) as i32 + pad,
            libm::ceilf(max_y) as i32 + pad,
        );
    Rectangle::with_corners(top_left, bottom_right)
}

fn label_rect(center: Point, text: &str, scale: u32) -> Rectangle {
    let size = Size::new(
        text.len() as u32 * GLYPH_WIDTH * scale,
        GLYPH_HEIGHT * scale,
    );
    Rectangle::new(
        center - Point::new(size.width as i32 / 2, size.height as i32 / 2),
        size,
    )
}

fn draw_label(
    canvas: &mut Canvas,
    center: Point,
    text: &str,
    scale: u32,
    color: Rgb565,
) -> Result<(), Infallible> {
    let mut scaled = Scaled {
        target: canvas,
        origin: label_rect(center, text, scale).top_left,
        scale,
    };
    Text::with_baseline(
        text,
        Point::zero(),
        MonoTextStyle::new(&FONT_10X20, color),
        Baseline::Top,
    )
    .draw(&mut scaled)?;
    Ok(())
}

/// Draws each incoming pixel as a `scale`-sized block at `origin + p * scale`.
struct Scaled<'a, 'b> {
    target: &'a mut Canvas<'b>,
    origin: Point,
    scale: u32,
}

impl Dimensions for Scaled<'_, '_> {
    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(Point::zero(), Size::new_equal(u16::MAX as u32))
    }
}

impl DrawTarget for Scaled<'_, '_> {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let block = Size::new_equal(self.scale);
        for Pixel(p, color) in pixels {
            let top_left = self.origin + p * self.scale as i32;
            self.target
                .fill_solid(&Rectangle::new(top_left, block), color)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alloc::{vec, vec::Vec};

    use super::*;
    use crate::{SCREEN_CENTER, screen_rect};

    fn full_screen(shapes: &[Shape]) -> Vec<u8> {
        let rect = screen_rect();
        let mut buf = vec![0u8; rect.size.width as usize * rect.size.height as usize * 2];
        render(shapes, Rgb565::BLACK, &mut Canvas::new(rect, &mut buf));
        buf
    }

    fn lit_pixels(shapes: &[Shape]) -> Vec<Point> {
        let mut buf = full_screen(shapes);
        let canvas = Canvas::new(screen_rect(), &mut buf);
        screen_rect()
            .points()
            .filter(|&p| canvas.pixel(p) != Some(Rgb565::BLACK))
            .collect()
    }

    static TEST_SPRITE: crate::Sprite = crate::Sprite {
        rows: &["..YY", "YYYY", "Y..Y"],
        palette: &[(b'Y', Rgb565::YELLOW)],
    };

    fn arc(start_deg: f32, sweep_deg: f32) -> Shape {
        Shape::Arc {
            center: SCREEN_CENTER,
            radius: 200,
            width: 12,
            start_deg,
            sweep_deg,
            color: Rgb565::WHITE,
        }
    }

    #[test]
    fn positive_arc_angles_run_clockwise_from_three_oclock() {
        let lit = lit_pixels(&[arc(80.0, 20.0)]);
        let six_oclock = SCREEN_CENTER + Point::new(0, 200);
        let twelve_oclock = SCREEN_CENTER - Point::new(0, 200);
        assert!(lit.contains(&six_oclock));
        assert!(!lit.contains(&twelve_oclock));
    }

    #[test]
    fn every_drawn_pixel_lies_inside_the_shape_bounds() {
        let shapes = [
            arc(0.0, 40.0),
            arc(350.0, 40.0),
            arc(-30.0, 75.0),
            arc(170.0, -60.0),
            arc(85.0, 100.0),
            arc(0.0, 360.0),
            Shape::Disc {
                center: Point::new(100, 300),
                radius: 9,
                color: Rgb565::RED,
            },
            Shape::Ring {
                center: SCREEN_CENTER,
                radius: 225,
                width: 6,
                color: Rgb565::BLUE,
            },
            Shape::Label {
                center: SCREEN_CENTER,
                text: "SCORE 42".into(),
                scale: 3,
                color: Rgb565::GREEN,
            },
            Shape::Rect {
                area: Rectangle::new(Point::new(40, 300), Size::new(70, 9)),
                color: Rgb565::CYAN,
            },
            Shape::Line {
                from: Point::new(300, 120),
                to: Point::new(90, 260),
                width: 3,
                color: Rgb565::WHITE,
            },
            Shape::Line {
                from: Point::new(100, 100),
                to: Point::new(101, 400),
                width: 5,
                color: Rgb565::WHITE,
            },
            Shape::Sprite {
                sprite: SpriteRef(&TEST_SPRITE),
                top_left: Point::new(250, 60),
                scale: 4,
                flip_x: true,
            },
            Shape::SpriteFit {
                sprite: SpriteRef(&TEST_SPRITE),
                area: Rectangle::new(Point::new(150, 300), Size::new(37, 50)),
                flip_x: true,
            },
        ];
        for shape in &shapes {
            let bounds = shape.bounds();
            let lit = lit_pixels(core::slice::from_ref(shape));
            assert!(!lit.is_empty(), "{shape:?} drew nothing");
            for p in lit {
                assert!(
                    bounds.contains(p),
                    "{shape:?} drew {p:?} outside {bounds:?}"
                );
            }
        }
    }

    #[test]
    fn clipping_an_arc_to_its_bounds_never_loses_a_pixel() {
        for (start, sweep) in [
            (0.0, 40.0),
            (350.0, 40.0),
            (-30.0, 75.0),
            (170.0, -60.0),
            (85.0, 100.0),
            (100.0, 70.0),
            (166.5, 3.5),
            (0.0, 360.0),
        ] {
            let shape = arc(start, sweep);
            let unclipped = {
                let rect = screen_rect();
                let mut buf = vec![0u8; rect.size.width as usize * rect.size.height as usize * 2];
                let mut canvas = Canvas::new(rect, &mut buf);
                let Ok(()) = draw_arc(
                    &mut canvas,
                    rect,
                    SCREEN_CENTER,
                    200,
                    12,
                    start,
                    sweep,
                    Rgb565::WHITE,
                );
                buf
            };
            assert!(full_screen(&[shape]) == unclipped, "{start}+{sweep}");
        }
    }

    #[test]
    fn short_arcs_get_bounds_much_smaller_than_their_circle() {
        let bounds = arc(30.0, 40.0).bounds();
        let area = bounds.size.width * bounds.size.height;
        assert!(area < 412 * 412 / 8, "{bounds:?} is not tight");
    }

    #[test]
    fn labels_are_centered_on_their_anchor() {
        let label = Shape::Label {
            center: Point::new(200, 100),
            text: "AB".into(),
            scale: 2,
            color: Rgb565::WHITE,
        };
        assert_eq!(
            label.bounds(),
            Rectangle::new(Point::new(180, 80), Size::new(40, 40))
        );
    }

    #[test]
    fn rendering_in_bands_matches_rendering_the_whole_screen() {
        let shapes = [
            arc(200.0, 50.0),
            Shape::Disc {
                center: Point::new(240, 233),
                radius: 30,
                color: Rgb565::RED,
            },
            Shape::Label {
                center: SCREEN_CENTER,
                text: "ORBIT".into(),
                scale: 4,
                color: Rgb565::YELLOW,
            },
            Shape::Sprite {
                sprite: SpriteRef(&TEST_SPRITE),
                top_left: Point::new(200, 150),
                scale: 7,
                flip_x: false,
            },
            Shape::Line {
                from: Point::new(20, 230),
                to: Point::new(440, 100),
                width: 2,
                color: Rgb565::WHITE,
            },
            Shape::SpriteFit {
                sprite: SpriteRef(&TEST_SPRITE),
                area: Rectangle::new(Point::new(90, 120), Size::new(123, 77)),
                flip_x: false,
            },
        ];
        let whole = full_screen(&shapes);

        let mut banded = Vec::new();
        let mut band = vec![0u8; 466 * 17 * 2];
        let mut y = 0;
        while y < 466 {
            let rows = 17.min(466 - y);
            let rect = Rectangle::new(Point::new(0, y as i32), Size::new(466, rows));
            let mut canvas = Canvas::new(rect, &mut band);
            render(&shapes, Rgb565::BLACK, &mut canvas);
            banded.extend_from_slice(canvas.bytes());
            y += rows;
        }
        assert!(banded == whole, "banded render differs from full render");
    }
}
