//! Stroked arcs rasterized only where they meet the canvas.
//!
//! embedded-graphics tests every point of an arc's whole circle (see the `PERF`
//! note in its arc iterator), and the presenter draws each shape once per band.
//! For the r=212 rim gauge that was about 190k points per band, which held a
//! charging cast to 2-3 fps on the ESP32. This walks only the canvas rows inside
//! `bounds` and, per row, the spans of the ring. It uses embedded-graphics' own
//! distance thresholds, so whole circles match it exactly and partial arcs
//! differ only along their two end edges.

use core::convert::Infallible;

use embedded_graphics::{pixelcolor::Rgb565, prelude::*, primitives::Rectangle};

use crate::Canvas;

/// Fixed-point scale of the sector edge directions.
const ONE: f32 = 4096.0;

/// Strokes the part of the arc inside both `bounds` and the canvas. A negative
/// sweep runs counterclockwise.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_arc(
    canvas: &mut Canvas,
    bounds: Rectangle,
    center: Point,
    radius: u32,
    width: u32,
    start_deg: f32,
    sweep_deg: f32,
    color: Rgb565,
) -> Result<(), Infallible> {
    let clip = bounds.intersection(&canvas.area());
    let Some(bottom_right) = clip.bottom_right() else {
        return Ok(());
    };
    let diameter = radius * 2 + 1;
    let outer = threshold(diameter + 2 * (width / 2));
    let inner = threshold(diameter.saturating_sub(2 * width.div_ceil(2)));
    let sector = Sector::new(start_deg, sweep_deg);
    for y in clip.top_left.y..=bottom_right.y {
        let dy = y - center.y;
        let dy4 = 4 * dy.unsigned_abs() * dy.unsigned_abs();
        let Some(reach) = half_width(outer, dy4) else {
            continue;
        };
        let spans = match half_width(inner, dy4) {
            Some(hole) => [(-reach, -hole - 1), (hole + 1, reach)],
            None => [(-reach, reach), (1, 0)],
        };
        for (from, to) in spans {
            let from = (center.x + from).max(clip.top_left.x);
            let to = (center.x + to).min(bottom_right.x);
            let mut run = None;
            for x in from..=to {
                match (sector.contains(x - center.x, dy), run) {
                    (true, None) => run = Some(x),
                    (false, Some(start)) => {
                        fill_row(canvas, start, x, y, color)?;
                        run = None;
                    }
                    _ => {}
                }
            }
            if let Some(start) = run {
                fill_row(canvas, start, to + 1, y, color)?;
            }
        }
    }
    Ok(())
}

fn fill_row(
    canvas: &mut Canvas,
    from: i32,
    to: i32,
    y: i32,
    color: Rgb565,
) -> Result<(), Infallible> {
    let run = Rectangle::new(Point::new(from, y), Size::new((to - from) as u32, 1));
    canvas.fill_solid(&run, color)
}

/// embedded-graphics' circle threshold: a point is inside a circle of this
/// diameter when 4 * (dx^2 + dy^2) is below it.
const fn threshold(diameter: u32) -> u32 {
    if diameter <= 4 {
        diameter * diameter - diameter / 2
    } else {
        diameter * diameter
    }
}

/// The largest |dx| inside the circle with `threshold` on a row `dy4` = 4 * dy^2
/// from its center, or `None` when the row misses it.
fn half_width(threshold: u32, dy4: u32) -> Option<i32> {
    let room = threshold.checked_sub(dy4 + 1)? / 4;
    let mut dx = libm::sqrtf(room as f32) as u32;
    while dx * dx > room {
        dx -= 1;
    }
    while (dx + 1) * (dx + 1) <= room {
        dx += 1;
    }
    Some(dx as i32)
}

/// The angles an arc covers, as half-planes through its center bounded by its
/// two end directions (angles run clockwise from 3 o'clock, as on screen).
struct Sector {
    start: (i32, i32),
    end: (i32, i32),
    whole: bool,
    wide: bool,
}

impl Sector {
    fn new(start_deg: f32, sweep_deg: f32) -> Self {
        let (from, to) = if sweep_deg < 0.0 {
            (start_deg + sweep_deg, start_deg)
        } else {
            (start_deg, start_deg + sweep_deg)
        };
        Self {
            start: direction(from),
            end: direction(to),
            whole: sweep_deg.abs() >= 360.0,
            wide: sweep_deg.abs() >= 180.0,
        }
    }

    fn contains(&self, dx: i32, dy: i32) -> bool {
        if self.whole {
            return true;
        }
        let past_start = self.start.0 * dy - self.start.1 * dx >= 0;
        let before_end = dx * self.end.1 - dy * self.end.0 >= 0;
        if self.wide {
            past_start || before_end
        } else {
            past_start && before_end
        }
    }
}

fn direction(deg: f32) -> (i32, i32) {
    let (sin, cos) = libm::sincosf(deg.to_radians());
    ((cos * ONE) as i32, (sin * ONE) as i32)
}

#[cfg(test)]
mod tests {
    use alloc::{vec, vec::Vec};

    use embedded_graphics::{
        pixelcolor::RgbColor,
        primitives::{Arc, Primitive, PrimitiveStyle},
    };

    use super::*;
    use crate::{SCREEN_CENTER, screen_rect};

    fn lit(draw: impl FnOnce(&mut Canvas)) -> Vec<bool> {
        let rect = screen_rect();
        let mut buf = vec![0u8; rect.size.width as usize * rect.size.height as usize * 2];
        let mut canvas = Canvas::new(rect, &mut buf);
        draw(&mut canvas);
        rect.points()
            .map(|p| canvas.pixel(p) != Some(Rgb565::BLACK))
            .collect()
    }

    /// Pixels where our arc and embedded-graphics' disagree.
    fn mismatches(radius: u32, width: u32, start_deg: f32, sweep_deg: f32) -> usize {
        let ours = lit(|canvas| {
            draw_arc(
                canvas,
                screen_rect(),
                SCREEN_CENTER,
                radius,
                width,
                start_deg,
                sweep_deg,
                Rgb565::WHITE,
            )
            .unwrap()
        });
        let reference = lit(|canvas| {
            Arc::with_center(
                SCREEN_CENTER,
                radius * 2 + 1,
                start_deg.deg(),
                sweep_deg.deg(),
            )
            .into_styled(PrimitiveStyle::with_stroke(Rgb565::WHITE, width))
            .draw(canvas)
            .unwrap()
        });
        ours.iter().zip(&reference).filter(|(a, b)| a != b).count()
    }

    #[test]
    fn full_circles_match_embedded_graphics_exactly() {
        for (radius, width) in [(212, 12), (225, 6), (40, 3), (9, 2)] {
            assert_eq!(
                mismatches(radius, width, 0.0, 360.0),
                0,
                "r {radius} w {width}"
            );
        }
    }

    #[test]
    fn partial_arcs_match_embedded_graphics_up_to_their_two_end_edges() {
        for (radius, width, start, sweep) in [
            (212, 12, 100.0, 70.0),
            (212, 12, 100.0, 3.0),
            (212, 12, 166.5, 3.5),
            (200, 14, 350.0, 40.0),
            (200, 14, -30.0, 75.0),
            (200, 14, 170.0, -60.0),
            (180, 8, 85.0, 200.0),
            (120, 10, 300.0, -250.0),
        ] {
            let edge_pixels = 2 * (width as usize + 2);
            let off = mismatches(radius, width, start, sweep);
            assert!(off <= edge_pixels, "{start}+{sweep}: {off} pixels differ");
        }
    }

    #[test]
    fn an_arc_drawn_in_bands_equals_one_drawn_whole() {
        let draw = |canvas: &mut Canvas| {
            draw_arc(
                canvas,
                screen_rect(),
                SCREEN_CENTER,
                212,
                12,
                100.0,
                70.0,
                Rgb565::YELLOW,
            )
            .unwrap()
        };
        let mut whole = vec![0u8; 466 * 466 * 2];
        draw(&mut Canvas::new(screen_rect(), &mut whole));
        let mut banded = Vec::new();
        let mut band = vec![0u8; 466 * 8 * 2];
        for y in (0..466).step_by(8) {
            let rows = 8.min(466 - y);
            let rect = Rectangle::new(Point::new(0, y as i32), Size::new(466, rows));
            let mut canvas = Canvas::new(rect, &mut band);
            let Ok(()) = canvas.clear(Rgb565::BLACK);
            draw(&mut canvas);
            banded.extend_from_slice(canvas.bytes());
        }
        assert!(banded == whole);
    }
}
