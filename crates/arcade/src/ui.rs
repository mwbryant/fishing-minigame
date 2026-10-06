//! The menu's touch targets, matching what the dial draws.

pub use orbit_ui::Button;

/// The rings sit 160 px out on the compass points, 72 px across.
pub const ANGLER: Button = Button::new(197, 37, 72, 72, "ANGLER");
pub const SPIDER: Button = Button::new(357, 197, 72, 72, "SPIDER");
pub const SKATE: Button = Button::new(197, 357, 72, 72, "SKATE");
pub const SETTINGS: Button = Button::new(37, 197, 72, 72, "SETTINGS");
/// In the same order as `CHOICES`.
pub const CHOICE_BUTTONS: [Button; 4] = [ANGLER, SPIDER, SKATE, SETTINGS];

pub const SOUND: Button = Button::new(133, 120, 200, 52, "SOUND");
pub const BACK: Button = Button::new(173, 320, 120, 44, "BACK");
