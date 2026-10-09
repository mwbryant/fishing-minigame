//! Spider Roulette's touch targets, and where the eggs and stakes sit.

use orbit_gfx::Point;
pub use orbit_ui::Button;
use orbit_ui::dial_point;

use crate::EGG_DEG;

/// Radius of the egg ring, px. Egg 0 is at 12 o'clock and the rest go
/// clockwise.
pub const EGG_RING: f32 = 160.0;

/// On the menus when the town launched the game: back out to the street.
pub const HOME: Button = Button::new(193, 40, 80, 30, "OUT");
/// Between spins, with a bet down and a safe egg cracked.
pub const CASH_OUT: Button = Button::new(173, 300, 120, 32, "CASH OUT");
/// Radius of the bet dial's ring of stakes, px.
pub const STAKE_RING: f32 = 150.0;
/// A tap this close to a stake picks it.
pub const STAKE_REACH: f32 = 48.0;

/// Where stake `index` of `count` sits on the bet dial.
pub fn stake_center(index: usize, count: usize) -> bevy::math::Vec2 {
    let at = dial_point(index as f32 * 360.0 / count as f32, STAKE_RING);
    bevy::math::Vec2::new(at.x as f32, at.y as f32)
}

/// Where egg `egg` sits on the glass.
pub fn egg_center(egg: usize) -> Point {
    dial_point(egg as f32 * EGG_DEG, EGG_RING)
}
