//! The touch pad beside the glass: a thumbstick that turns the board (left
//! and right, like the arrow keys) and leans it (up and down), around a
//! mini board that turns and leans as the game reads the real one. It's a
//! second 466 x 466 panel, drawn by `orbit_gfx` like the board's.

use bevy::math::Vec2;
use orbit_gfx::{Point, Presenter, Rectangle, Shape, Size};
use orbit_motion::LEAN_HELD_DEG;
use orbit_ui::{GOLD, HINT, TOAST, rgb};

use crate::panel::{RgbaPanel, SIZE};

const CENTER: Vec2 = Vec2::splat(SIZE as f32 / 2.0);
/// How far the knob travels for a full push, in panel pixels.
pub const TRAVEL: f32 = 150.0;
/// Pushes shorter than this (of a full one) are a resting thumb, not a turn.
pub const DEAD_ZONE: f32 = 0.12;
const KNOB: u32 = 46;
const BOARD: f32 = 105.0;
/// The lean meter's track runs this far above and below the middle.
const METER: f32 = 60.0;
const METER_X: i32 = 33;
/// The meter's ends, in degrees of lean.
const METER_DEG: f32 = LEAN_HELD_DEG * 4.0 / 3.0;

const BACKGROUND: orbit_gfx::Rgb565 = rgb(28, 30, 35);
const TRACK: orbit_gfx::Rgb565 = rgb(58, 62, 70);
const KNOB_FILL: orbit_gfx::Rgb565 = rgb(110, 116, 128);
const DIM: orbit_gfx::Rgb565 = rgb(80, 84, 92);
/// The mini board's glass.
const FACE: orbit_gfx::Rgb565 = rgb(10, 11, 14);

/// The board as the game reads it (`orbit_core::Tilt`), in degrees.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub steer_deg: f32,
    pub lean_deg: f32,
}

/// The stick's push (at most 1 long, y down as on screen)
/// for a finger at `at` on the pad.
pub fn stick(at: Vec2) -> Vec2 {
    let push = (at - CENTER) / TRAVEL;
    let length = push.length();
    if length < DEAD_ZONE {
        return Vec2::ZERO;
    }
    let strength = ((length - DEAD_ZONE) / (1.0 - DEAD_ZONE)).min(1.0);
    push / length * strength
}

/// The pad's shapes: the stick pushed by `stick`, and the board as the game
/// reads it (`None` for a game that doesn't turn, drawn level and dim).
pub fn draw(stick: Vec2, pose: Option<Pose>) -> Vec<Shape> {
    let (pose, rim, top, meter) = match pose {
        Some(pose) => (pose, HINT, GOLD, TOAST),
        None => (Pose::default(), DIM, DIM, DIM),
    };
    let turn = pose.steer_deg;
    let radial = |deg: f32, from: f32, to: f32, width: u32, color| Shape::Line {
        from: point(around(from, deg + turn)),
        to: point(around(to, deg + turn)),
        width,
        color,
    };
    let lean = (pose.lean_deg / METER_DEG).clamp(-1.0, 1.0) * METER;
    let center = point(CENTER);
    vec![
        Shape::Ring {
            center,
            radius: TRAVEL as u32,
            width: 2,
            color: TRACK,
        },
        Shape::Disc {
            center,
            radius: BOARD as u32,
            color: FACE,
        },
        Shape::Ring {
            center,
            radius: BOARD as u32,
            width: 8,
            color: rim,
        },
        // The side buttons, PWR at 2 o'clock and BOOT at 4, as on the board.
        radial(-30.0, BOARD + 8.0, BOARD + 24.0, 14, rim),
        radial(30.0, BOARD + 8.0, BOARD + 24.0, 14, rim),
        radial(-90.0, BOARD - 34.0, BOARD - 6.0, 14, top),
        Shape::Disc {
            center: point(CENTER + stick * TRAVEL),
            radius: KNOB,
            color: KNOB_FILL,
        },
        Shape::Ring {
            center: point(CENTER + stick * TRAVEL),
            radius: KNOB,
            width: 4,
            color: HINT,
        },
        Shape::Line {
            from: Point::new(METER_X, (CENTER.y - METER) as i32),
            to: Point::new(METER_X, (CENTER.y + METER) as i32),
            width: 6,
            color: TRACK,
        },
        Shape::Rect {
            area: Rectangle::new(
                Point::new(METER_X - 14, (CENTER.y - lean) as i32 - 5),
                Size::new(28, 10),
            ),
            color: meter,
        },
    ]
}

/// The point `radius` from the middle, `deg` clockwise from 3 o'clock.
fn around(radius: f32, deg: f32) -> Vec2 {
    let rad = deg.to_radians();
    CENTER + Vec2::new(rad.cos(), rad.sin()) * radius
}

fn point(at: Vec2) -> Point {
    Point::new(at.x.round() as i32, at.y.round() as i32)
}

/// The pad's panel, redrawn only where it changed, as the board's is.
pub struct Pad {
    presenter: Presenter<Vec<u8>>,
    panel: RgbaPanel,
    drawn: bool,
}

impl Default for Pad {
    fn default() -> Self {
        Self {
            presenter: Presenter::new(vec![0u8; SIZE * 2 * 8], BACKGROUND),
            panel: RgbaPanel::default(),
            drawn: false,
        }
    }
}

impl Pad {
    pub fn show(&mut self, stick: Vec2, pose: Option<Pose>) {
        self.drawn |= self.presenter.present(&draw(stick, pose), &mut self.panel) > 0;
    }

    pub fn pixels(&self) -> &[u8] {
        self.panel.pixels()
    }

    /// True once after the pixels change.
    pub fn take_drawn(&mut self) -> bool {
        core::mem::take(&mut self.drawn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turned(steer_deg: f32, lean_deg: f32) -> Option<Pose> {
        Some(Pose {
            steer_deg,
            lean_deg,
        })
    }

    /// Pixels of `color` the pad shows within `radius` of `at`.
    fn ink_near(pad: &Pad, at: Vec2, radius: f32, color: orbit_gfx::Rgb565) -> usize {
        let rgba = crate::panel::rgba8(color);
        (0..SIZE * SIZE)
            .filter(|i| {
                let p = Vec2::new((i % SIZE) as f32, (i / SIZE) as f32);
                p.distance(at) <= radius && pad.pixels()[i * 4..i * 4 + 4] == rgba
            })
            .count()
    }

    fn shown(stick: Vec2, pose: Option<Pose>) -> Pad {
        let mut pad = Pad::default();
        pad.show(stick, pose);
        pad
    }

    #[test]
    fn a_finger_at_the_rim_of_its_travel_pushes_the_stick_all_the_way() {
        let right = stick(CENTER + Vec2::new(TRAVEL, 0.0));
        assert!(right.distance(Vec2::X) < 1e-4, "{right}");
        let up = stick(CENTER + Vec2::new(0.0, -TRAVEL / 2.0));
        assert!(up.y < -0.4 && up.x == 0.0, "{up}");
    }

    #[test]
    fn a_finger_past_the_rim_still_pushes_only_all_the_way() {
        let far = stick(CENTER + Vec2::new(3.0 * TRAVEL, 3.0 * TRAVEL));
        assert!((far.length() - 1.0).abs() < 1e-4, "{far}");
    }

    #[test]
    fn a_thumb_resting_near_the_middle_doesnt_turn_anything() {
        assert_eq!(stick(CENTER + Vec2::new(8.0, -6.0)), Vec2::ZERO);
    }

    #[test]
    fn the_mini_board_turns_its_top_where_the_board_is_turned() {
        let top = CENTER - Vec2::new(0.0, BOARD - 15.0);
        let right = CENTER + Vec2::new(BOARD - 15.0, 0.0);
        let level = shown(Vec2::ZERO, turned(0.0, 0.0));
        assert!(ink_near(&level, top, 20.0, GOLD) > 50);
        assert_eq!(ink_near(&level, right, 20.0, GOLD), 0);
        let quarter = shown(Vec2::ZERO, turned(90.0, 0.0));
        assert_eq!(ink_near(&quarter, top, 20.0, GOLD), 0);
        assert!(ink_near(&quarter, right, 20.0, GOLD) > 50);
    }

    #[test]
    fn the_lean_meter_rises_as_the_board_tips_forward() {
        let middle = Vec2::new(METER_X as f32, CENTER.y);
        let high = middle - Vec2::new(0.0, METER * 0.75);
        let level = shown(Vec2::ZERO, turned(0.0, 0.0));
        assert!(ink_near(&level, middle, 6.0, TOAST) > 10);
        let forward = shown(Vec2::ZERO, turned(0.0, LEAN_HELD_DEG));
        assert_eq!(ink_near(&forward, middle, 6.0, TOAST), 0);
        assert!(ink_near(&forward, high, 6.0, TOAST) > 10);
    }

    #[test]
    fn the_knob_follows_the_push() {
        let pushed = shown(Vec2::X, turned(0.0, 0.0));
        let knob = CENTER + Vec2::new(TRAVEL, 0.0);
        assert!(ink_near(&pushed, knob, 10.0, KNOB_FILL) > 100);
        assert_eq!(ink_near(&pushed, CENTER, 10.0, KNOB_FILL), 0);
    }

    #[test]
    fn a_game_that_doesnt_turn_shows_the_board_dim() {
        let still = shown(Vec2::ZERO, None);
        let top = CENTER - Vec2::new(0.0, BOARD - 15.0);
        assert_eq!(ink_near(&still, top, 20.0, GOLD), 0);
        assert!(ink_near(&still, top, 20.0, DIM) > 50);
    }
}
