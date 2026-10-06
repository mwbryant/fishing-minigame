//! Where the eggs sit (shared by drawing and taps) and the HOME button.

use bevy::math::Vec2;
pub use orbit_ui::Button;
use orbit_ui::dial_point;

use crate::EGGS;

/// The eggs ring the web this far out, clockwise from 12 o'clock.
pub const EGG_RING: f32 = 160.0;
/// A tap this close to an egg's center picks it.
const EGG_REACH: f32 = 30.0;

pub const HOME: Button = Button::new(193, 40, 80, 30, "HOME");

pub fn egg_angle(egg: usize) -> f32 {
    egg as f32 * 360.0 / EGGS as f32
}

pub fn egg_center(egg: usize) -> Vec2 {
    let p = dial_point(egg_angle(egg), EGG_RING);
    Vec2::new(p.x as f32, p.y as f32)
}

pub fn egg_at(at: Vec2) -> Option<usize> {
    (0..EGGS).find(|&egg| egg_center(egg).distance(at) <= EGG_REACH)
}
