//! The menu's touch targets, matching what the dial draws.

pub use orbit_ui::Button;

/// The rings sit 160 px out, 72 degrees apart clockwise from 12 o'clock,
/// 72 px across.
pub const ANGLER: Button = Button::new(197, 37, 72, 72, "ANGLER");
pub const SPIDER: Button = Button::new(349, 148, 72, 72, "SPIDER");
pub const SKATE: Button = Button::new(291, 326, 72, 72, "SKATE");
pub const BEES: Button = Button::new(103, 326, 72, 72, "BEES");
pub const SETTINGS: Button = Button::new(45, 148, 72, 72, "SETTINGS");
/// In the same order as `CHOICES`.
pub const CHOICE_BUTTONS: [Button; 5] = [ANGLER, SPIDER, SKATE, BEES, SETTINGS];

pub const SOUND: Button = Button::new(133, 120, 200, 52, "SOUND");
pub const BACK: Button = Button::new(173, 320, 120, 44, "BACK");
