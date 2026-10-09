use alloc::{vec, vec::Vec};

use embedded_graphics::{prelude::*, primitives::Rectangle};

use crate::{Shape, screen_rect};

/// Remembers the previous frame and reports which screen regions changed, so
/// only those need re-rendering and re-sending to the panel.
#[derive(Default)]
pub struct FrameDiff {
    prev: Option<Vec<Shape>>,
}

impl FrameDiff {
    /// Regions to redraw to turn the last frame into `shapes`. The first call
    /// returns the whole screen, since panel RAM starts as garbage. After that
    /// it's the merged, even-aligned bounds of every shape that came or went.
    pub fn update(&mut self, shapes: &[Shape]) -> Vec<Rectangle> {
        let rects = match &self.prev {
            None => vec![screen_rect()],
            Some(prev) => changed_regions(prev, shapes),
        };
        self.prev = Some(shapes.to_vec());
        rects
    }

    /// Forces the next `update` to redraw the whole screen.
    pub fn invalidate(&mut self) {
        self.prev = None;
    }
}

fn changed_regions(prev: &[Shape], curr: &[Shape]) -> Vec<Rectangle> {
    let mut rects: Vec<Rectangle> = prev
        .iter()
        .filter(|s| !curr.contains(s))
        .chain(curr.iter().filter(|s| !prev.contains(s)))
        .map(|s| align_even(s.bounds()).intersection(&screen_rect()))
        .filter(|r| !r.is_zero_sized())
        .collect();
    while let Some((i, j)) = overlapping_pair(&rects) {
        let other = rects.swap_remove(j);
        rects[i] = union(&rects[i], &other);
    }
    rects
}

fn overlapping_pair(rects: &[Rectangle]) -> Option<(usize, usize)> {
    for i in 0..rects.len() {
        for j in i + 1..rects.len() {
            if !rects[i].intersection(&rects[j]).is_zero_sized() {
                return Some((i, j));
            }
        }
    }
    None
}

fn union(a: &Rectangle, b: &Rectangle) -> Rectangle {
    let (ax, ay, bx, by) = (a.columns(), a.rows(), b.columns(), b.rows());
    Rectangle::with_corners(
        Point::new(ax.start.min(bx.start), ay.start.min(by.start)),
        Point::new(ax.end.max(bx.end) - 1, ay.end.max(by.end) - 1),
    )
}

/// Grows `rect` so every edge lands on an even coordinate. The CO5300 rejects
/// windows with odd start columns/rows or odd widths/heights.
pub fn align_even(rect: Rectangle) -> Rectangle {
    let (cols, rows) = (rect.columns(), rect.rows());
    let floor = |v: i32| v & !1;
    let ceil = |v: i32| (v + 1) & !1;
    Rectangle::with_corners(
        Point::new(floor(cols.start), floor(rows.start)),
        Point::new(ceil(cols.end) - 1, ceil(rows.end) - 1),
    )
}

/// Splits `rect` into horizontal bands as wide as it, each fitting in
/// `max_bytes` of RGB565. Bands are an even number of rows (at least 2), except
/// a last one cut short by an odd-height `rect`.
pub fn bands(rect: Rectangle, max_bytes: usize) -> impl Iterator<Item = Rectangle> {
    let row_bytes = rect.size.width as usize * 2;
    let rows_per_band = ((max_bytes / row_bytes.max(1)) as u32 & !1).max(2);
    rect.rows().step_by(rows_per_band as usize).map(move |y| {
        let rows = rows_per_band.min(rect.rows().end as u32 - y as u32);
        Rectangle::new(
            Point::new(rect.top_left.x, y),
            Size::new(rect.size.width, rows),
        )
    })
}

#[cfg(test)]
mod tests {
    use embedded_graphics::pixelcolor::Rgb565;

    use super::*;

    fn ball(x: i32, y: i32) -> Shape {
        Shape::Disc {
            center: Point::new(x, y),
            radius: 8,
            color: Rgb565::WHITE,
        }
    }

    fn rect(x: i32, y: i32, w: u32, h: u32) -> Rectangle {
        Rectangle::new(Point::new(x, y), Size::new(w, h))
    }

    #[test]
    fn first_frame_redraws_the_whole_screen() {
        let mut diff = FrameDiff::default();
        assert_eq!(diff.update(&[ball(100, 100)]), vec![screen_rect()]);
    }

    #[test]
    fn identical_frames_need_no_redraw() {
        let mut diff = FrameDiff::default();
        diff.update(&[ball(100, 100)]);
        assert!(diff.update(&[ball(100, 100)]).is_empty());
    }

    #[test]
    fn a_moved_shape_dirties_both_its_old_and_new_position() {
        let mut diff = FrameDiff::default();
        diff.update(&[ball(100, 100)]);
        let rects = diff.update(&[ball(300, 300)]);
        assert_eq!(rects.len(), 2);
        for old_or_new in [ball(100, 100).bounds(), ball(300, 300).bounds()] {
            assert!(
                rects
                    .iter()
                    .any(|r| r.intersection(&old_or_new) == old_or_new)
            );
        }
    }

    #[test]
    fn overlapping_regions_merge_into_one() {
        let mut diff = FrameDiff::default();
        diff.update(&[ball(100, 100)]);
        assert_eq!(diff.update(&[ball(104, 102)]).len(), 1);
    }

    #[test]
    fn invalidate_forces_a_full_redraw() {
        let mut diff = FrameDiff::default();
        diff.update(&[ball(100, 100)]);
        diff.invalidate();
        assert_eq!(diff.update(&[ball(100, 100)]), vec![screen_rect()]);
    }

    #[test]
    fn regions_stay_on_screen_and_even_aligned() {
        let mut diff = FrameDiff::default();
        diff.update(&[]);
        for r in diff.update(&[ball(3, 461)]) {
            assert_eq!(r.intersection(&screen_rect()), r);
            assert_eq!(r.top_left.x % 2, 0);
            assert_eq!(r.top_left.y % 2, 0);
            assert_eq!(r.size.width % 2, 0);
            assert_eq!(r.size.height % 2, 0);
        }
    }

    #[test]
    fn align_even_only_grows_the_rect() {
        let r = rect(3, 5, 4, 3);
        let aligned = align_even(r);
        assert_eq!(aligned, rect(2, 4, 6, 4));
        assert_eq!(aligned.intersection(&r), r);
    }

    #[test]
    fn bands_tile_the_rect_within_the_byte_budget() {
        let area = rect(10, 20, 100, 51);
        let all: Vec<Rectangle> = bands(area, 100 * 2 * 7).collect();
        assert!(
            all.iter()
                .all(|b| b.size.width == 100 && b.size.height <= 6)
        );
        assert_eq!(all.iter().map(|b| b.size.height).sum::<u32>(), 51);
        assert_eq!(all[0].top_left, Point::new(10, 20));
        for pair in all.windows(2) {
            assert_eq!(pair[1].top_left.y, pair[0].rows().end);
        }
    }
}
