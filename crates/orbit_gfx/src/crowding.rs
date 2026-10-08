//! Finds text that something else gets in the way of: another label, a shape
//! drawn over or under it, or the edge of the round glass. Measured on the
//! pixels each shape actually draws, inside the box the label's letters ink.

use alloc::{vec, vec::Vec};
use core::convert::Infallible;

use embedded_graphics::{
    Pixel,
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::{Rgb565, raw::RawU16},
    prelude::*,
    primitives::Rectangle,
    text::{Baseline, Text},
};

use crate::{Canvas, SCREEN_CENTER, SCREEN_SIZE, Shape, shape::label_rect};

/// What gets in a label's way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clash {
    /// Another label's letters, by its index in the frame.
    Text(usize),
    /// A shape drawn after the label, covering part of it.
    Over(usize),
    /// A shape drawn before the label, under part of it but not all of it.
    /// Bands across the whole screen (sky, water, ground) are backdrops even
    /// where their edge crosses text.
    Under(usize),
    /// Letters past the edge of the round glass.
    OffGlass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crowding {
    /// The label's index in the frame.
    pub label: usize,
    pub clash: Clash,
}

/// The box a label's letters actually ink, which is smaller than its font
/// cells; `None` for anything else, or a label with nothing to show.
pub fn ink(shape: &Shape) -> Option<Rectangle> {
    let Shape::Label {
        center,
        text,
        scale,
        ..
    } = shape
    else {
        return None;
    };
    let mut extent = Extent::default();
    let Ok(_) = Text::with_baseline(
        text,
        Point::zero(),
        MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE),
        Baseline::Top,
    )
    .draw(&mut extent);
    let (min, max) = extent.corners?;
    let origin = label_rect(*center, text, *scale).top_left;
    let scale = *scale as i32;
    Some(Rectangle::with_corners(
        origin + min * scale,
        origin + (max + Point::new(1, 1)) * scale - Point::new(1, 1),
    ))
}

/// Everything in the way of every label in `shapes` (a frame, back to front).
pub fn crowding(shapes: &[Shape]) -> Vec<Crowding> {
    let hides = |label: usize, area: Rectangle| {
        shapes[label + 1..].iter().any(|shape| {
            !matches!(shape, Shape::Label { .. })
                && contains(shape.bounds(), area)
                && covered_pixels(shape, area) == (area.size.width * area.size.height) as usize
        })
    };
    let inks: Vec<(usize, Rectangle)> = shapes
        .iter()
        .enumerate()
        .filter_map(|(i, shape)| Some((i, ink(shape)?)))
        .filter(|&(i, area)| !hides(i, area))
        .collect();
    let mut found = Vec::new();
    for (n, &(label, area)) in inks.iter().enumerate() {
        if !on_glass(area) {
            found.push(Crowding {
                label,
                clash: Clash::OffGlass,
            });
        }
        for &(other, other_area) in &inks[n + 1..] {
            if !area
                .offset(1)
                .intersection(&other_area.offset(1))
                .is_zero_sized()
            {
                found.push(Crowding {
                    label,
                    clash: Clash::Text(other),
                });
            }
        }
        let all = (area.size.width * area.size.height) as usize;
        let touching = |other: &usize| {
            let shape = &shapes[*other];
            !matches!(shape, Shape::Label { .. })
                && !area.intersection(&shape.bounds()).is_zero_sized()
        };
        // Underneath, only what's above the last shape that hides all of it shows.
        for other in (0..label).rev().filter(touching) {
            let shape = &shapes[other];
            match covered_pixels(shape, area) {
                0 => {}
                covered if covered == all => break,
                _ if is_band(shape) => {}
                _ => found.push(Crowding {
                    label,
                    clash: Clash::Under(other),
                }),
            }
        }
        for other in (label + 1..shapes.len()).filter(touching) {
            if covered_pixels(&shapes[other], area) > 0 {
                found.push(Crowding {
                    label,
                    clash: Clash::Over(other),
                });
            }
        }
    }
    found
}

fn contains(outer: Rectangle, inner: Rectangle) -> bool {
    inner
        .bottom_right()
        .is_some_and(|end| outer.contains(inner.top_left) && outer.contains(end))
}

fn is_band(shape: &Shape) -> bool {
    let Shape::Rect { area, .. } = shape else {
        return false;
    };
    area.top_left.x <= 0 && area.top_left.x + area.size.width as i32 >= SCREEN_SIZE as i32
}

fn on_glass(area: Rectangle) -> bool {
    let Some(bottom_right) = area.bottom_right() else {
        return true;
    };
    let radius = SCREEN_SIZE as f32 / 2.0;
    [
        area.top_left,
        Point::new(bottom_right.x, area.top_left.y),
        Point::new(area.top_left.x, bottom_right.y),
        bottom_right,
    ]
    .iter()
    .all(|corner| {
        let d = *corner - SCREEN_CENTER;
        let (x, y) = (d.x as f32 + 0.5, d.y as f32 + 0.5);
        x * x + y * y <= radius * radius
    })
}

/// How many pixels of `area` the shape draws, rendered alone onto two
/// different backgrounds so a pixel the color of one still shows on the other.
fn covered_pixels(shape: &Shape, area: Rectangle) -> usize {
    if let Shape::Rect { area: rect, .. } = shape {
        let overlap = rect.intersection(&area);
        return (overlap.size.width * overlap.size.height) as usize;
    }
    let backgrounds = [Rgb565::new(1, 2, 3), Rgb565::new(30, 60, 29)];
    let len = (area.size.width * area.size.height * 2) as usize;
    let mut drawn = vec![false; len / 2];
    let mut buf = vec![0u8; len];
    for background in backgrounds {
        let mut canvas = Canvas::new(area, &mut buf);
        let Ok(()) = canvas.clear(background);
        shape.draw(&mut canvas);
        let [hi, lo] = RawU16::from(background).into_inner().to_be_bytes();
        for (pixel, mark) in canvas.bytes().chunks_exact(2).zip(drawn.iter_mut()) {
            *mark |= pixel != [hi, lo];
        }
    }
    drawn.iter().filter(|&&mark| mark).count()
}

/// A draw target that only remembers the corners of what was drawn.
#[derive(Default)]
struct Extent {
    corners: Option<(Point, Point)>,
}

impl OriginDimensions for Extent {
    fn size(&self) -> Size {
        Size::new_equal(u16::MAX as u32)
    }
}

impl DrawTarget for Extent {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(p, _) in pixels {
            self.corners = Some(match self.corners {
                Some((min, max)) => (
                    Point::new(min.x.min(p.x), min.y.min(p.y)),
                    Point::new(max.x.max(p.x), max.y.max(p.y)),
                ),
                None => (p, p),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use super::*;

    fn label(x: i32, y: i32, text: &str) -> Shape {
        Shape::Label {
            center: Point::new(x, y),
            text: String::from(text),
            scale: 1,
            color: Rgb565::WHITE,
        }
    }

    fn rect(x: i32, y: i32, w: u32, h: u32) -> Shape {
        Shape::Rect {
            area: Rectangle::new(Point::new(x, y), Size::new(w, h)),
            color: Rgb565::BLACK,
        }
    }

    #[test]
    fn a_label_inks_less_than_its_font_cells() {
        let shape = label(233, 233, "HI");
        let ink = ink(&shape).unwrap();
        let cells = shape.bounds();
        assert!(ink.size.height < cells.size.height, "{ink:?} in {cells:?}");
        assert!(cells.contains(ink.top_left) && cells.contains(ink.bottom_right().unwrap()));
        assert_eq!(super::ink(&label(233, 233, "   ")), None);
    }

    #[test]
    fn lines_one_row_apart_are_fine_and_lines_on_top_of_each_other_are_not() {
        let apart = [label(233, 200, "TANK 0/3"), label(233, 222, "TURN TO PICK")];
        assert_eq!(crowding(&apart), []);
        let stacked = [label(233, 200, "TANK 0/3"), label(233, 210, "TURN TO PICK")];
        assert_eq!(
            crowding(&stacked),
            [Crowding {
                label: 0,
                clash: Clash::Text(1)
            }]
        );
    }

    #[test]
    fn text_on_a_backdrop_is_fine_but_half_on_a_box_or_under_one_is_not() {
        let on_backdrop = [rect(100, 180, 266, 40), label(233, 200, "BACK")];
        assert_eq!(crowding(&on_backdrop), []);
        let half_on = [rect(100, 180, 133, 40), label(233, 200, "BACK")];
        assert_eq!(
            crowding(&half_on),
            [Crowding {
                label: 1,
                clash: Clash::Under(0)
            }]
        );
        let covered = [label(233, 200, "BACK"), rect(230, 180, 10, 40)];
        assert_eq!(
            crowding(&covered),
            [Crowding {
                label: 0,
                clash: Clash::Over(1)
            }]
        );
    }

    #[test]
    fn what_a_plate_hides_under_the_text_doesnt_count() {
        let path_under_a_sign = [
            rect(220, 0, 26, 466),
            rect(100, 180, 266, 40),
            label(233, 200, "A SIGN"),
        ];
        assert_eq!(crowding(&path_under_a_sign), []);
        let hidden_by_an_overlay = [
            label(233, 200, "BEHIND"),
            rect(0, 0, 466, 466),
            label(233, 205, "GOOD NIGHT"),
        ];
        assert_eq!(crowding(&hidden_by_an_overlay), []);
    }

    #[test]
    fn letters_touching_crowd_but_a_two_pixel_gap_is_enough() {
        let first = ink(&label(233, 200, "TOP")).unwrap();
        let gap = |y: i32| {
            let second = label(233, y, "TOP");
            ink(&second).unwrap().top_left.y - first.bottom_right().unwrap().y - 1
        };
        let y_for_gap = |want: i32| (200..260).find(|&y| gap(y) == want).unwrap();
        let touching = [label(233, 200, "TOP"), label(233, y_for_gap(1), "TOP")];
        assert_eq!(crowding(&touching).len(), 1);
        let clear = [label(233, 200, "TOP"), label(233, y_for_gap(2), "TOP")];
        assert_eq!(crowding(&clear), []);
    }

    #[test]
    fn a_band_across_the_whole_screen_is_a_backdrop_even_at_its_edge() {
        let band_edge = [rect(0, 200, 466, 100), label(233, 200, "SKY AND WATER")];
        assert_eq!(crowding(&band_edge), []);
        let strip = [rect(220, 0, 26, 466), label(233, 200, "A PATH UNDER IT")];
        assert_eq!(crowding(&strip).len(), 1);
    }

    #[test]
    fn text_past_the_round_edge_is_off_the_glass() {
        let low = [label(233, 455, "A LONG LINE NEAR THE BOTTOM")];
        assert_eq!(
            crowding(&low),
            [Crowding {
                label: 0,
                clash: Clash::OffGlass
            }]
        );
        assert_eq!(
            crowding(&[label(233, 233, "A LONG LINE IN THE MIDDLE")]),
            []
        );
    }
}
