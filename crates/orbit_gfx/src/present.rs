use embedded_graphics::{pixelcolor::Rgb565, primitives::Rectangle};

use crate::{Canvas, FrameDiff, Shape, bands, render};

/// Somewhere rendered pixels go: the CO5300 over QSPI, or a RAM model of it.
pub trait Panel {
    /// Writes `bytes` (big-endian RGB565, row-major) into the window `area`.
    /// `area` always has even coordinates and an even size.
    fn write(&mut self, area: Rectangle, bytes: &[u8]);
}

/// Turns successive frames into minimal panel writes. Each region that changed
/// since the last frame is rendered and sent band by band.
pub struct Presenter<B> {
    diff: FrameDiff,
    band: B,
    background: Rgb565,
}

impl<B: AsMut<[u8]>> Presenter<B> {
    /// `band` is the scratch buffer bands render into. It must hold at least two
    /// full screen rows (466 * 2 * 2 bytes).
    pub fn new(band: B, background: Rgb565) -> Self {
        Self {
            diff: FrameDiff::default(),
            band,
            background,
        }
    }

    /// Makes the next `present` redraw the whole screen.
    pub fn invalidate(&mut self) {
        self.diff.invalidate();
    }

    /// Sends whatever changed since the last call and returns the pixel bytes sent.
    pub fn present(&mut self, shapes: &[Shape], panel: &mut impl Panel) -> usize {
        let buf = self.band.as_mut();
        let mut sent = 0;
        for rect in self.diff.update(shapes) {
            for band in bands(rect, buf.len()) {
                let mut canvas = Canvas::new(band, buf);
                render(shapes, self.background, &mut canvas);
                panel.write(band, canvas.bytes());
                sent += canvas.bytes().len();
            }
        }
        sent
    }
}

#[cfg(test)]
mod tests {
    use alloc::{format, vec, vec::Vec};

    use embedded_graphics::{pixelcolor::RgbColor, prelude::*};

    use super::*;
    use crate::{SCREEN_CENTER, SCREEN_SIZE, screen_rect};

    /// Panel RAM model that starts as garbage, like the real one.
    struct RamPanel {
        ram: Vec<u8>,
        writes: Vec<Rectangle>,
    }

    impl RamPanel {
        fn new() -> Self {
            let len = (SCREEN_SIZE * SCREEN_SIZE * 2) as usize;
            Self {
                ram: (0..len).map(|i| (i * 31 % 251) as u8).collect(),
                writes: Vec::new(),
            }
        }
    }

    impl Panel for RamPanel {
        fn write(&mut self, area: Rectangle, bytes: &[u8]) {
            let row_bytes = area.size.width as usize * 2;
            for (row, src) in bytes.chunks_exact(row_bytes).enumerate() {
                let y = area.top_left.y as usize + row;
                let start = (y * SCREEN_SIZE as usize + area.top_left.x as usize) * 2;
                self.ram[start..start + row_bytes].copy_from_slice(src);
            }
            self.writes.push(area);
        }
    }

    fn full_render(shapes: &[Shape]) -> Vec<u8> {
        let mut buf = vec![0u8; (SCREEN_SIZE * SCREEN_SIZE * 2) as usize];
        render(
            shapes,
            Rgb565::BLACK,
            &mut Canvas::new(screen_rect(), &mut buf),
        );
        buf
    }

    fn scene(t: i32) -> Vec<Shape> {
        let mut shapes = vec![
            Shape::Ring {
                center: SCREEN_CENTER,
                radius: 212,
                width: 2,
                color: Rgb565::BLUE,
            },
            Shape::Label {
                center: SCREEN_CENTER,
                text: format!("{}", t / 7),
                scale: 6,
                color: Rgb565::GREEN,
            },
            Shape::Arc {
                center: SCREEN_CENTER,
                radius: 212,
                width: 14,
                start_deg: t as f32 * 13.0,
                sweep_deg: 44.0,
                color: Rgb565::CYAN,
            },
            Shape::Disc {
                center: SCREEN_CENTER + Point::new((t * 23) % 300 - 150, (t * 17) % 280 - 140),
                radius: 9,
                color: Rgb565::YELLOW,
            },
        ];
        if t % 5 == 0 {
            shapes.push(Shape::Label {
                center: SCREEN_CENTER + Point::new(0, 80),
                text: "BLINK".into(),
                scale: 1,
                color: Rgb565::WHITE,
            });
        }
        shapes
    }

    #[test]
    fn panel_ram_always_matches_a_full_render_of_the_latest_frame() {
        let mut panel = RamPanel::new();
        let mut presenter = Presenter::new(vec![0u8; 466 * 2 * 24], Rgb565::BLACK);
        for t in 0..40 {
            let shapes = scene(t);
            presenter.present(&shapes, &mut panel);
            assert!(panel.ram == full_render(&shapes), "frame {t} diverged");
        }
    }

    #[test]
    fn an_unchanged_frame_sends_nothing() {
        let mut panel = RamPanel::new();
        let mut presenter = Presenter::new(vec![0u8; 466 * 2 * 24], Rgb565::BLACK);
        presenter.present(&scene(1), &mut panel);
        assert_eq!(presenter.present(&scene(1), &mut panel), 0);
    }

    #[test]
    fn a_moving_ball_sends_far_less_than_a_full_screen() {
        let mut panel = RamPanel::new();
        let mut presenter = Presenter::new(vec![0u8; 466 * 2 * 24], Rgb565::BLACK);
        let ball = |x| {
            vec![Shape::Disc {
                center: Point::new(x, 200),
                radius: 9,
                color: Rgb565::YELLOW,
            }]
        };
        presenter.present(&ball(100), &mut panel);
        let sent = presenter.present(&ball(104), &mut panel);
        assert!(sent > 0 && sent < 466 * 466 * 2 / 50, "sent {sent} bytes");
    }

    #[test]
    fn every_write_respects_the_panel_alignment_rules() {
        let mut panel = RamPanel::new();
        let mut presenter = Presenter::new(vec![0u8; 466 * 2 * 24], Rgb565::BLACK);
        for t in 0..20 {
            presenter.present(&scene(t), &mut panel);
        }
        for w in &panel.writes {
            assert_eq!(
                (
                    w.top_left.x % 2,
                    w.top_left.y % 2,
                    w.size.width % 2,
                    w.size.height % 2
                ),
                (0, 0, 0, 0),
                "{w:?}"
            );
        }
    }
}
