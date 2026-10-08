//! The town's touch buttons.

pub use orbit_ui::Button;

/// At a door, below its name.
pub const ENTER: Button = Button::new(173, 404, 120, 32, "GO IN");
/// The sign at a door, GO IN and all: a tap anywhere on it goes in.
pub const SIGN: Button = Button::new(113, 366, 240, 76, "");
/// Settings, clockwise from 12 o'clock as on its dial.
pub const SOUND: Button = Button::new(148, 88, 170, 44, "SOUND");
pub const MOCK_WALK: Button = Button::new(286, 211, 160, 44, "MOCK WALK");
pub const RESET: Button = Button::new(163, 334, 140, 44, "RESET");
pub const BACK: Button = Button::new(40, 211, 110, 44, "BACK");
pub const SETTINGS_BUTTONS: [Button; 4] = [SOUND, MOCK_WALK, RESET, BACK];
pub const KEEP: Button = Button::new(88, 300, 140, 44, "KEEP");
pub const ERASE: Button = Button::new(238, 300, 140, 44, "ERASE");
