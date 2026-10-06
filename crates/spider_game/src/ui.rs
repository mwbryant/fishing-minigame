//! Where the eggs sit and the HOME button.

use orbit_gfx::Point;
pub use orbit_ui::Button;
use orbit_ui::dial_point;

use crate::EGG_DEG;

/// The eggs ring the web this far out, clockwise from 12 o'clock.
pub const EGG_RING: f32 = 160.0;

pub const HOME: Button = Button::new(193, 40, 80, 30, "HOME");

pub fn egg_center(egg: usize) -> Point {
    dial_point(egg as f32 * EGG_DEG, EGG_RING)
}
