//! Where the eggs sit and the HOME button.

use orbit_gfx::Point;
pub use orbit_ui::Button;
use orbit_ui::dial_point;

use crate::EGG_DEG;

/// The eggs ring the web this far out, clockwise from 12 o'clock.
pub const EGG_RING: f32 = 160.0;

pub const HOME: Button = Button::new(193, 40, 80, 30, "OUT");
/// Between spins, with a bet down and a safe egg cracked.
pub const CASH_OUT: Button = Button::new(173, 300, 120, 32, "CASH OUT");
/// How far out the bet dial's stakes sit.
pub const STAKE_RING: f32 = 150.0;

/// Where stake `index` of `count` sits on the bet dial.
pub fn stake_center(index: usize, count: usize) -> bevy::math::Vec2 {
    let at = dial_point(index as f32 * 360.0 / count as f32, STAKE_RING);
    bevy::math::Vec2::new(at.x as f32, at.y as f32)
}

pub fn egg_center(egg: usize) -> Point {
    dial_point(egg as f32 * EGG_DEG, EGG_RING)
}
