//! The town's touch buttons.

pub use orbit_ui::Button;

/// Settings, clockwise from 12 o'clock as on its five-spot dial.
pub const SOUND: Button = Button::new(158, 70, 150, 44, "SOUND");
pub const BRIGHTNESS: Button = Button::new(290, 160, 140, 44, "BRIGHT");
pub const MOCK_WALK: Button = Button::new(234, 316, 160, 44, "MOCK WALK");
pub const RESET: Button = Button::new(82, 316, 140, 44, "RESET");
pub const BACK: Button = Button::new(46, 160, 120, 44, "BACK");
pub const SETTINGS_BUTTONS: [Button; 5] = [SOUND, BRIGHTNESS, MOCK_WALK, RESET, BACK];
pub const KEEP: Button = Button::new(88, 300, 140, 44, "KEEP");
pub const ERASE: Button = Button::new(238, 300, 140, 44, "ERASE");
