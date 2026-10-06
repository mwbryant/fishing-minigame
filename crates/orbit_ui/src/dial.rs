//! Rotation as the selector: turning the device like a steering wheel sweeps
//! a pointer around the round glass, and the option it points at is chosen.

use bevy::math::ops;
use orbit_gfx::{Point, Rgb565, SCREEN_CENTER, Shape};

/// The pointer turns this many times as far as the device, so a comfortable
/// +-60 degree turn of the wrist reaches all the way round.
pub const DIAL_GAIN: f32 = 3.0;

/// The pointer for a device turn: degrees clockwise from 12 o'clock, 0..360.
pub fn dial_angle(steer_deg: f32) -> f32 {
    ops::rem_euclid(steer_deg * DIAL_GAIN, 360.0)
}

/// Which of `count` options, spaced evenly clockwise from 12 o'clock, the
/// pointer is nearest.
pub fn dial_choice(steer_deg: f32, count: usize) -> usize {
    let step = 360.0 / count as f32;
    (ops::rem_euclid(dial_angle(steer_deg) + step / 2.0, 360.0) / step) as usize % count
}

/// The screen point `radius` from the center at dial angle `deg`.
pub fn dial_point(deg: f32, radius: f32) -> Point {
    let (sin, cos) = ops::sin_cos(deg.to_radians());
    SCREEN_CENTER
        + Point::new(
            ops::round(sin * radius) as i32,
            ops::round(-cos * radius) as i32,
        )
}

/// The pointer as a line from `from` to `to` pixels out from the center.
pub fn dial_pointer(deg: f32, from: f32, to: f32, width: u32, color: Rgb565) -> Shape {
    Shape::Line {
        from: dial_point(deg, from),
        to: dial_point(deg, to),
        width,
        color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holding_level_points_at_the_top_option() {
        assert_eq!(dial_choice(0.0, 4), 0);
        assert_eq!(dial_choice(5.0, 4), 0);
        assert_eq!(dial_choice(-5.0, 4), 0);
    }

    #[test]
    fn a_thirty_degree_turn_moves_a_quarter_of_the_way_round() {
        assert_eq!(dial_choice(30.0, 4), 1, "clockwise to 3 o'clock");
        assert_eq!(dial_choice(-30.0, 4), 3, "counterclockwise to 9 o'clock");
        assert_eq!(dial_choice(60.0, 4), 2);
        assert_eq!(dial_choice(-60.0, 4), 2);
    }

    #[test]
    fn many_options_split_the_circle_evenly_and_wrap() {
        assert_eq!(dial_choice(10.0, 12), 1);
        assert_eq!(dial_choice(-10.0, 12), 11);
        assert_eq!(dial_choice(120.0, 12), 0, "a full turn of the pointer");
    }

    #[test]
    fn dial_points_run_clockwise_from_twelve_oclock() {
        assert_eq!(dial_point(0.0, 100.0), SCREEN_CENTER - Point::new(0, 100));
        assert_eq!(dial_point(90.0, 100.0), SCREEN_CENTER + Point::new(100, 0));
    }
}
