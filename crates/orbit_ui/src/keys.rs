//! Typing by turning: the alphabet rings the glass, the device's turn points
//! at a key (the way the dials do), and PWR types it. SPACE, DEL and OK sit
//! just counterclockwise of A, a small turn left from level.

use alloc::{format, vec::Vec};

use bevy::math::{Vec2, ops};
use orbit_gfx::{Point, Rgb565, Shape};

use crate::{
    DEEP, DIAL_GAIN, GOLD, HINT, TEXT, dial::dial_angle, dial_point, disc, label, label_at,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Letter(char),
    Space,
    Delete,
    Done,
}

impl Key {
    pub fn label(self) -> &'static str {
        const LETTERS: [&str; 26] = [
            "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q",
            "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
        ];
        match self {
            Key::Letter(letter) => LETTERS[(letter as u8 - b'A') as usize % 26],
            Key::Space => "SP",
            Key::Delete => "DEL",
            Key::Done => "OK",
        }
    }
}

const fn letter(index: u8) -> Key {
    Key::Letter((b'A' + index) as char)
}

/// Clockwise from 12 o'clock.
pub const KEYS: [Key; 29] = {
    let mut keys = [Key::Done; 29];
    let mut i = 0;
    while i < 26 {
        keys[i] = letter(i as u8);
        i += 1;
    }
    keys[26] = Key::Space;
    keys[27] = Key::Delete;
    keys[28] = Key::Done;
    keys
};

/// How far out the keys sit.
pub const KEY_RING: f32 = 196.0;
/// A tap this close to a key's center types it.
const TAP_PX: f32 = 22.0;
const STEP_DEG: f32 = 360.0 / KEYS.len() as f32;
/// The pointer has to get this far (in keys) from the pointed key's middle
/// before the next one is pointed, so a hand held on a boundary doesn't
/// flicker between two letters.
const STICKY: f32 = 0.65;

/// The pointed key, with a little hysteresis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyRing {
    pointed: usize,
}

impl KeyRing {
    /// Follows the device's turn; returns the key now pointed at.
    pub fn point(&mut self, steer_deg: f32) -> Key {
        let angle = dial_angle(steer_deg);
        let center = self.pointed as f32 * STEP_DEG;
        let off = ops::rem_euclid(angle - center + 180.0, 360.0) - 180.0;
        if off.abs() > STICKY * STEP_DEG {
            self.pointed =
                (ops::rem_euclid(angle + STEP_DEG / 2.0, 360.0) / STEP_DEG) as usize % KEYS.len();
        }
        self.pointed()
    }

    pub fn pointed(&self) -> Key {
        KEYS[self.pointed]
    }

    /// The key a tap at `at` lands on.
    pub fn key_at(at: Vec2) -> Option<Key> {
        KEYS.iter().enumerate().find_map(|(i, key)| {
            let center = dial_point(i as f32 * STEP_DEG, KEY_RING);
            let center = Vec2::new(center.x as f32, center.y as f32);
            (center.distance(at) <= TAP_PX).then_some(*key)
        })
    }
}

/// The device turn that points straight at `key`, for scripted typing.
pub fn key_turn(key: Key) -> f32 {
    let index = KEYS.iter().position(|k| *k == key).unwrap_or(0);
    let deg = index as f32 * STEP_DEG;
    let deg = if deg > 180.0 { deg - 360.0 } else { deg };
    deg / DIAL_GAIN
}

/// Where `key` sits on the glass, for scripted taps.
pub fn key_center(key: Key) -> Vec2 {
    let index = KEYS.iter().position(|k| *k == key).unwrap_or(0);
    let center = dial_point(index as f32 * STEP_DEG, KEY_RING);
    Vec2::new(center.x as f32, center.y as f32)
}

/// The ring of keys with the pointed one lit, `title` and `typed` (with a
/// cursor) in the middle, and the controls below. OK shows dim until
/// `can_finish`.
pub fn key_ring(
    shapes: &mut Vec<Shape>,
    ring: &KeyRing,
    title: &str,
    typed: &str,
    can_finish: bool,
) {
    for (i, key) in KEYS.iter().enumerate() {
        let center = dial_point(i as f32 * STEP_DEG, KEY_RING);
        let pointed = i == ring.pointed;
        if pointed {
            shapes.push(disc(center.x, center.y, 20, GOLD));
        }
        let color: Rgb565 = match (pointed, key) {
            (true, _) => DEEP,
            (false, Key::Done) if !can_finish => HINT,
            (false, Key::Letter(_)) => TEXT,
            (false, _) => GOLD,
        };
        let scale = if matches!(key, Key::Letter(_)) { 2 } else { 1 };
        shapes.push(label_at(center, key.label(), scale, color));
    }
    shapes.push(label(170, title, 1, HINT));
    shapes.push(label(222, &format!("{typed}_"), 3, TEXT));
    shapes.push(label(282, "TURN TO PICK A LETTER", 1, HINT));
    shapes.push(label(302, "PWR TO TYPE IT", 1, HINT));
    shapes.push(label_at(Point::new(233, 330), "OR TAP ONE", 1, HINT));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn_to(index: usize) -> f32 {
        key_turn(KEYS[index])
    }

    #[test]
    fn level_points_at_a_and_turning_clockwise_walks_the_alphabet() {
        let mut ring = KeyRing::default();
        assert_eq!(ring.point(0.0), Key::Letter('A'));
        assert_eq!(ring.point(turn_to(1)), Key::Letter('B'));
        assert_eq!(ring.point(turn_to(25)), Key::Letter('Z'));
    }

    #[test]
    fn a_small_turn_left_reaches_ok_delete_and_space() {
        let mut ring = KeyRing::default();
        assert_eq!(ring.point(turn_to(28)), Key::Done);
        assert_eq!(ring.point(turn_to(27)), Key::Delete);
        assert_eq!(ring.point(turn_to(26)), Key::Space);
        assert!(turn_to(26).abs() < 13.0, "{}", turn_to(26));
    }

    #[test]
    fn a_hand_held_on_a_boundary_does_not_flicker() {
        let mut ring = KeyRing::default();
        ring.point(0.0);
        let boundary = turn_to(0) + STEP_DEG / 2.0 / DIAL_GAIN;
        for wobble in [0.3, -0.3, 0.4, -0.2, 0.1] {
            assert_eq!(ring.point(boundary + wobble), Key::Letter('A'));
        }
        assert_eq!(ring.point(turn_to(1)), Key::Letter('B'));
        for wobble in [0.3, -0.3, 0.4, -0.2] {
            assert_eq!(ring.point(boundary + wobble), Key::Letter('B'));
        }
    }

    #[test]
    fn tapping_a_key_on_the_rim_types_it_and_the_middle_types_nothing() {
        let k = dial_point(10.0 * STEP_DEG, KEY_RING);
        assert_eq!(
            KeyRing::key_at(Vec2::new(k.x as f32 + 5.0, k.y as f32)),
            Some(Key::Letter('K'))
        );
        assert_eq!(KeyRing::key_at(Vec2::splat(233.0)), None);
    }
}
